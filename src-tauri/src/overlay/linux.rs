//! GTK surfaces only: no second WebKit process, and no audio-callback UI work.
//! Like Hex, prefer layer-shell and use an input-transparent XWayland surface
//! when the compositor does not implement that protocol.
use super::{Frame, Kind, Position, Presentation};
use gtk::{cairo, gdk, glib, prelude::*};
use std::{cell::RefCell, rc::Rc, sync::OnceLock, time::Duration};

const WIDTH: i32 = 260;
const HEIGHT: i32 = 48;
const MARGIN: i32 = 32;

type WindowFn = unsafe extern "C" fn(*mut gtk::ffi::GtkWindow);
type ValueFn = unsafe extern "C" fn(*mut gtk::ffi::GtkWindow, i32);
type EdgeFn = unsafe extern "C" fn(*mut gtk::ffi::GtkWindow, i32, i32);

struct LayerShell {
    init: WindowFn,
    layer: ValueFn,
    keyboard: ValueFn,
    exclusive: ValueFn,
    anchor: EdgeFn,
    margin: EdgeFn,
}

impl LayerShell {
    fn load() -> Option<Self> {
        // Even an unsupported probe installs GTK hooks and Wayland listeners.
        // Statics are never dropped: retain the library before calling any of its functions,
        // including when the probe or a subsequent symbol lookup fails.
        static LIBRARY: OnceLock<Option<libloading::Library>> = OnceLock::new();
        Self::load_from(&LIBRARY, std::ffi::OsStr::new("libgtk-layer-shell.so.0"))
    }

    fn load_from(
        cache: &'static OnceLock<Option<libloading::Library>>,
        path: &std::ffi::OsStr,
    ) -> Option<Self> {
        // Optional at runtime: missing layer-shell must not prevent launching Chime on X11/GNOME.
        unsafe {
            let library = cache
                .get_or_init(|| libloading::Library::new(path).ok())
                .as_ref()?;
            let supported: libloading::Symbol<unsafe extern "C" fn() -> i32> =
                library.get(b"gtk_layer_is_supported\0").ok()?;
            if supported() == 0 {
                return None;
            }
            Some(Self {
                init: *library.get(b"gtk_layer_init_for_window\0").ok()?,
                layer: *library.get(b"gtk_layer_set_layer\0").ok()?,
                keyboard: *library.get(b"gtk_layer_set_keyboard_mode\0").ok()?,
                exclusive: *library.get(b"gtk_layer_set_exclusive_zone\0").ok()?,
                anchor: *library.get(b"gtk_layer_set_anchor\0").ok()?,
                margin: *library.get(b"gtk_layer_set_margin\0").ok()?,
            })
        }
    }

    fn configure(&self, window: &gtk::Window, position: Position, initialize: bool) {
        // Values are from gtk-layer-shell's stable C enum ABI: overlay=3, keyboard none=0,
        // top=2, bottom=3. Initialize before GTK realizes the surface.
        unsafe {
            let ptr = window.as_ptr();
            if initialize {
                (self.init)(ptr);
                (self.layer)(ptr, 3);
                (self.keyboard)(ptr, 0);
                (self.exclusive)(ptr, 0);
            }
            (self.anchor)(ptr, 2, i32::from(position == Position::Top));
            (self.anchor)(ptr, 3, i32::from(position == Position::Bottom));
            (self.margin)(ptr, 2, MARGIN);
            (self.margin)(ptr, 3, MARGIN);
        }
    }
}

struct Surface {
    window: gtk::Window,
    layer: Option<LayerShell>,
    display: gdk::Display,
    frame: Rc<RefCell<Option<Frame>>>,
    position: Position,
    backend: &'static str,
}

impl Surface {
    fn new() -> Result<Self, String> {
        let default = gdk::Display::default().ok_or("Chime cannot reach your display.")?;
        let layer = LayerShell::load();
        let (display, backend) = if layer.is_some() {
            (default, "Wayland")
        } else if default.type_().name() == "GdkX11Display" {
            (default, "X11")
        } else {
            let name = std::env::var("DISPLAY")
                .ok()
                .filter(|s| !s.is_empty())
                .ok_or(
                    "This desktop needs XWayland to show the recording overlay. Sounds still work.",
                )?;
            // Open only the HUD on X11. Never change GDK_BACKEND for the main app.
            let display = gdk::Display::open(&name)
                .filter(|display| display.type_().name() == "GdkX11Display");
            (
                display.ok_or(
                    "Could not open the recording overlay on XWayland. Sounds still work.",
                )?,
                "XWayland",
            )
        };
        let window = gtk::Window::new(gtk::WindowType::Toplevel);
        window.set_screen(&display.default_screen());
        window.set_title("Chime Recording");
        window.set_decorated(false);
        window.set_resizable(false);
        window.set_accept_focus(false);
        window.set_focus_on_map(false);
        window.set_skip_taskbar_hint(true);
        window.set_skip_pager_hint(true);
        window.set_keep_above(true);
        window.set_type_hint(gdk::WindowTypeHint::Notification);
        window.set_app_paintable(true);
        window.set_size_request(WIDTH, HEIGHT);
        window.connect_delete_event(|window, _| {
            window.hide();
            glib::Propagation::Stop
        });
        if let Some(visual) = display.default_screen().rgba_visual() {
            window.set_visual(Some(&visual));
        }
        let x11 = layer.is_none();
        window.connect_realize(move |window| {
            if let Some(native) = window.window() {
                if x11 {
                    native.set_override_redirect(true);
                }
                native.input_shape_combine_region(&cairo::Region::create(), 0, 0);
                native.set_pass_through(true);
            }
        });
        let frame = Rc::new(RefCell::new(None::<Frame>));
        let drawing = frame.clone();
        window.connect_draw(move |_, context| {
            draw(context, drawing.borrow().as_ref());
            glib::Propagation::Stop
        });
        if let Some(layer) = &layer {
            layer.configure(&window, Position::Bottom, true);
        }
        Ok(Self {
            window,
            layer,
            display,
            frame,
            position: Position::Hidden,
            backend,
        })
    }

    fn update(&mut self, position: Position, frame: Option<Frame>) {
        if frame.is_none() || position == Position::Hidden {
            self.window.hide();
            *self.frame.borrow_mut() = None;
            return;
        }
        if !self.window.is_visible() || position != self.position {
            if let Some(layer) = &self.layer {
                layer.configure(&self.window, position, false);
            } else {
                // GDK coordinates are logical, including negative monitor origins.
                let monitor = self
                    .display
                    .default_seat()
                    .and_then(|seat| seat.pointer())
                    .map(|pointer| pointer.position())
                    .and_then(|(_, x, y)| self.display.monitor_at_point(x, y))
                    .or_else(|| self.display.primary_monitor())
                    .or_else(|| self.display.monitor(0));
                if let Some(monitor) = monitor {
                    let area = monitor.workarea();
                    let (x, y) =
                        placement(area.x(), area.y(), area.width(), area.height(), position);
                    self.window.move_(x, y);
                }
            }
            self.position = position;
        }
        *self.frame.borrow_mut() = frame;
        self.window.show(); // Not present(): never requests keyboard focus.
        self.window.queue_draw();
    }
}

fn placement(x: i32, y: i32, width: i32, height: i32, position: Position) -> (i32, i32) {
    let left = x + (width - WIDTH).max(0) / 2;
    let top = if position == Position::Top {
        y + MARGIN
    } else {
        y + height - HEIGHT - MARGIN
    };
    (left, top.max(y))
}

fn draw(context: &cairo::Context, frame: Option<&Frame>) {
    context.set_operator(cairo::Operator::Source);
    context.set_source_rgba(0.0, 0.0, 0.0, 0.0);
    let _ = context.paint();
    let Some(frame) = frame else { return };
    context.set_operator(cairo::Operator::Over);
    let r = 22.0;
    context.new_sub_path();
    context.arc(
        WIDTH as f64 - r - 1.0,
        24.0,
        r,
        -std::f64::consts::FRAC_PI_2,
        std::f64::consts::FRAC_PI_2,
    );
    context.arc(
        r + 1.0,
        24.0,
        r,
        std::f64::consts::FRAC_PI_2,
        3.0 * std::f64::consts::FRAC_PI_2,
    );
    context.close_path();
    context.set_source_rgba(0.055, 0.055, 0.055, 0.96);
    let _ = context.fill_preserve();
    context.set_source_rgba(1.0, 1.0, 1.0, 0.18);
    context.set_line_width(1.0);
    let _ = context.stroke();
    context.set_source_rgb(1.0, 90.0 / 255.0, 31.0 / 255.0);
    context.set_line_width(3.0);
    context.set_line_cap(cairo::LineCap::Round);
    for (i, base) in [5.0, 11.0, 19.0, 11.0, 5.0].iter().enumerate() {
        let height = if frame.kind == Kind::Listening {
            base * (0.4 + frame.level as f64 * 0.9)
        } else {
            *base
        };
        let x = 21.0 + i as f64 * 6.0;
        context.move_to(x, 24.0 - height / 2.0);
        context.line_to(x, 24.0 + height / 2.0);
    }
    let _ = context.stroke();
    context.select_font_face(
        "sans-serif",
        cairo::FontSlant::Normal,
        cairo::FontWeight::Normal,
    );
    context.set_font_size(13.0);
    context.set_source_rgb(0.97, 0.945, 0.89);
    context.move_to(62.0, 28.5);
    let _ = context.show_text(frame.label);
}

pub fn install(app: &tauri::AppHandle) -> Result<&'static str, String> {
    let mut surface = Surface::new()?;
    let backend = surface.backend;
    let app = app.clone();
    let mut presentation = Presentation::default();
    let (position, frame) = super::current_frame(&app, &mut presentation);
    surface.update(position, frame);
    glib::timeout_add_local(Duration::from_millis(50), move || {
        let (position, frame) = super::current_frame(&app, &mut presentation);
        surface.update(position, frame);
        glib::ControlFlow::Continue
    });
    Ok(backend)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;
    #[test]
    fn library_survives_unsupported_probe_and_missing_symbols() {
        let dir = std::env::temp_dir().join(format!(
            "chime-overlay-library-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let source = dir.join("probe.c");
        let path = dir.join("probe.so");
        // First probe is unsupported. The second succeeds but required symbols are missing.
        // The callback represents code retained by GTK after either probe.
        std::fs::write(
            &source,
            r#"
            static int probes = 0;
            int gtk_layer_is_supported(void) { return probes++ > 0; }
            int retained_callback(void) { return probes; }
        "#,
        )
        .unwrap();
        let build = std::process::Command::new("cc")
            .args(["-shared", "-fPIC"])
            .arg(&source)
            .arg("-o")
            .arg(&path)
            .output()
            .unwrap();
        assert!(
            build.status.success(),
            "{}",
            String::from_utf8_lossy(&build.stderr)
        );
        let cache = Box::leak(Box::new(OnceLock::new()));
        assert!(LayerShell::load_from(cache, path.as_os_str()).is_none());
        let library = cache.get().unwrap().as_ref().unwrap();
        let callback = unsafe {
            *library
                .get::<unsafe extern "C" fn() -> i32>(b"retained_callback\0")
                .unwrap()
        };
        assert_eq!(unsafe { callback() }, 1);
        assert!(LayerShell::load_from(cache, path.as_os_str()).is_none());
        assert_eq!(
            unsafe { callback() },
            2,
            "The same library must remain loaded after both failures"
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn placement_respects_negative_monitor_origins_and_workareas() {
        assert_eq!(
            placement(-1920, -200, 1920, 1080, Position::Bottom),
            (-1090, 800)
        );
        assert_eq!(placement(0, 30, 1920, 1010, Position::Top), (830, 62));
    }

    #[test]
    #[ignore = "shows a passive native overlay for three seconds"]
    fn native_overlay_smoke() {
        gtk::init().unwrap();
        let mut surface = Surface::new().unwrap();
        eprintln!("Overlay backend: {}", surface.backend);
        if let Ok(path) = std::env::var("CHIME_OVERLAY_CAPTURE") {
            let image = cairo::ImageSurface::create(cairo::Format::ARgb32, WIDTH, HEIGHT).unwrap();
            let context = cairo::Context::new(&image).unwrap();
            draw(
                &context,
                Some(&Frame {
                    kind: Kind::Listening,
                    label: "Listening",
                    level: 0.6,
                }),
            );
            gdk::pixbuf_get_from_surface(&image, 0, 0, WIDTH, HEIGHT)
                .unwrap()
                .savev(path, "png", &[])
                .unwrap();
        }
        let loop_ = glib::MainLoop::new(None, false);
        let done = loop_.clone();
        let start = Instant::now();
        glib::timeout_add_local(Duration::from_millis(50), move || {
            if start.elapsed() >= Duration::from_secs(3) {
                surface.window.hide();
                done.quit();
                return glib::ControlFlow::Break;
            }
            surface.update(
                Position::Bottom,
                Some(Frame {
                    kind: Kind::Listening,
                    label: "Listening · preview",
                    level: (start.elapsed().as_secs_f32() * 5.0).sin().abs(),
                }),
            );
            assert!(
                !surface.window.is_active(),
                "The overlay must never become active"
            );
            assert!(surface.window.window().unwrap().is_pass_through());
            glib::ControlFlow::Continue
        });
        loop_.run();
    }
}
