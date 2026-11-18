//! A nonactivating NSPanel, visible across Spaces without becoming the key window.
use super::{Frame, Position, Presentation};
use objc2::{rc::Retained, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSBackingStoreType, NSBox, NSBoxType, NSColor, NSEvent, NSFont, NSPanel, NSScreen,
    NSStatusWindowLevel, NSTextField, NSTitlePosition, NSView, NSWindowCollectionBehavior,
    NSWindowStyleMask,
};
use objc2_foundation::{
    NSPoint, NSRect, NSRunLoop, NSRunLoopCommonModes, NSSize, NSString, NSTimer,
};
use std::cell::RefCell;

const WIDTH: f64 = 260.0;
const HEIGHT: f64 = 48.0;

struct Surface {
    panel: Retained<NSPanel>,
    label: Retained<NSTextField>,
    bars: Vec<Retained<NSBox>>,
    position: Position,
}

thread_local! {
    static HUD: RefCell<Option<(Surface, Presentation)>> = const { RefCell::new(None) };
}

fn rect(x: f64, y: f64, width: f64, height: f64) -> NSRect {
    NSRect::new(NSPoint::new(x, y), NSSize::new(width, height))
}

impl Surface {
    fn new(mtm: MainThreadMarker) -> Self {
        let panel = NSPanel::initWithContentRect_styleMask_backing_defer(
            NSPanel::alloc(mtm),
            rect(0.0, 0.0, WIDTH, HEIGHT),
            NSWindowStyleMask::Borderless | NSWindowStyleMask::NonactivatingPanel,
            NSBackingStoreType::Buffered,
            false,
        );
        // Retained owns this panel; AppKit must not release it a second time on close.
        unsafe {
            panel.setReleasedWhenClosed(false);
        }
        panel.setFloatingPanel(true);
        panel.setBecomesKeyOnlyIfNeeded(true);
        panel.setHidesOnDeactivate(false);
        panel.setIgnoresMouseEvents(true);
        panel.setOpaque(false);
        panel.setBackgroundColor(Some(&NSColor::clearColor()));
        panel.setHasShadow(false);
        panel.setLevel(NSStatusWindowLevel);
        panel.setCollectionBehavior(
            NSWindowCollectionBehavior::CanJoinAllSpaces
                | NSWindowCollectionBehavior::FullScreenAuxiliary,
        );
        let content = NSView::initWithFrame(NSView::alloc(mtm), rect(0.0, 0.0, WIDTH, HEIGHT));
        let background =
            NSBox::initWithFrame(NSBox::alloc(mtm), rect(1.0, 2.0, WIDTH - 2.0, HEIGHT - 4.0));
        background.setBoxType(NSBoxType::Custom);
        background.setTitlePosition(NSTitlePosition::NoTitle);
        background.setCornerRadius(22.0);
        background.setBorderWidth(1.0);
        background.setBorderColor(&NSColor::colorWithSRGBRed_green_blue_alpha(
            1.0, 1.0, 1.0, 0.18,
        ));
        background.setFillColor(&NSColor::colorWithSRGBRed_green_blue_alpha(
            0.055, 0.055, 0.055, 0.96,
        ));
        content.addSubview(&background);
        let label = NSTextField::labelWithString(&NSString::from_str(""), mtm);
        label.setFrame(rect(62.0, 14.0, 190.0, 20.0));
        label.setFont(Some(&NSFont::systemFontOfSize(13.0)));
        label.setTextColor(Some(&NSColor::colorWithSRGBRed_green_blue_alpha(
            0.97, 0.945, 0.89, 1.0,
        )));
        content.addSubview(&label);
        let bars = (0..5)
            .map(|_| {
                let bar = NSBox::initWithFrame(NSBox::alloc(mtm), rect(0.0, 0.0, 3.0, 5.0));
                bar.setBoxType(NSBoxType::Custom);
                bar.setTitlePosition(NSTitlePosition::NoTitle);
                bar.setBorderWidth(0.0);
                bar.setCornerRadius(1.5);
                bar.setFillColor(&NSColor::colorWithSRGBRed_green_blue_alpha(
                    1.0,
                    90.0 / 255.0,
                    31.0 / 255.0,
                    1.0,
                ));
                content.addSubview(&bar);
                bar
            })
            .collect();
        panel.setContentView(Some(&content));
        Self {
            panel,
            label,
            bars,
            position: Position::Hidden,
        }
    }

    fn update(&mut self, mtm: MainThreadMarker, position: Position, frame: Option<Frame>) {
        let frame = match frame.filter(|_| position != Position::Hidden) {
            Some(frame) => frame,
            None => {
                if self.panel.isVisible() {
                    self.panel.orderOut(None);
                }
                return;
            }
        };
        if !self.panel.isVisible() || self.position != position {
            let cursor = NSEvent::mouseLocation();
            let screens = NSScreen::screens(mtm);
            let screen = screens
                .iter()
                .find(|screen| {
                    let frame = screen.frame();
                    cursor.x >= frame.origin.x
                        && cursor.x < frame.origin.x + frame.size.width
                        && cursor.y >= frame.origin.y
                        && cursor.y < frame.origin.y + frame.size.height
                })
                .or_else(|| NSScreen::mainScreen(mtm));
            if let Some(screen) = screen {
                let area = screen.visibleFrame();
                let y = if position == Position::Top {
                    area.origin.y + area.size.height - HEIGHT - 16.0
                } else {
                    area.origin.y + 16.0
                };
                self.panel.setFrameOrigin(NSPoint::new(
                    area.origin.x + (area.size.width - WIDTH) / 2.0,
                    y,
                ));
            }
            self.position = position;
        }
        self.label.setStringValue(&NSString::from_str(frame.label));
        for (i, (bar, height)) in self.bars.iter().zip(frame.heights).enumerate() {
            bar.setFrame(rect(
                19.5 + i as f64 * 6.0,
                (HEIGHT - height) / 2.0,
                3.0,
                height,
            ));
        }
        if !self.panel.isVisible() {
            self.panel.orderFrontRegardless();
        }
    }
}

pub fn install(app: &tauri::AppHandle) -> Result<&'static str, String> {
    let mtm =
        MainThreadMarker::new().ok_or("The recording overlay must start on the main thread.")?;
    let mut presentation = Presentation::default();
    let mut surface = Surface::new(mtm);
    let (position, frame) = super::current_frame(app, &mut presentation);
    surface.update(mtm, position, frame);
    HUD.with(|state| *state.borrow_mut() = Some((surface, presentation)));
    let app = app.clone();
    let block = block2::RcBlock::new(move |_: std::ptr::NonNull<NSTimer>| {
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        HUD.with(|state| {
            let mut state = state.borrow_mut();
            if let Some((surface, presentation)) = state.as_mut() {
                let (position, frame) = super::current_frame(&app, presentation);
                surface.update(mtm, position, frame);
            }
        });
    });
    // A main-run-loop timer keeps all AppKit access on its required thread.
    unsafe {
        let timer = NSTimer::timerWithTimeInterval_repeats_block(0.05, true, &block);
        NSRunLoop::mainRunLoop().addTimer_forMode(&timer, NSRunLoopCommonModes);
    }
    Ok("macOS")
}
