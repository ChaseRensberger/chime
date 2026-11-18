use super::{presentation, Duration, TrayTask};
use crate::session::{Sessions, TrayState};
use gtk::prelude::GtkWindowExt;
use ksni::{menu::StandardItem, MenuItem, TrayMethods};
use std::sync::atomic::{AtomicU8, Ordering};
use tauri::Manager;

#[derive(Default)]
struct Availability(AtomicU8);
pub(super) fn available(app: &tauri::AppHandle) -> bool {
    app.try_state::<Availability>()
        .is_some_and(|s| s.0.load(Ordering::Acquire) == 1)
}

struct Tray {
    app: tauri::AppHandle,
    state: TrayState,
    icon: ksni::Icon,
}

impl Tray {
    fn show(&self) {
        let token = ksni::current_xdg_activation_token();
        let app = self.app.clone();
        let _ = self.app.run_on_main_thread(move || {
            if let Some(window) = app.get_webview_window("main") {
                if let Ok(native) = window.gtk_window() {
                    if let Some(token) = token {
                        native.set_startup_id(&token);
                    }
                    native.deiconify();
                    native.present();
                }
            }
        });
    }

    fn item(label: &str, enabled: bool, action: fn(&mut Self)) -> MenuItem<Self> {
        StandardItem {
            label: label.into(),
            enabled,
            activate: Box::new(action),
            ..Default::default()
        }
        .into()
    }
}

impl ksni::Tray for Tray {
    fn watcher_offline(&self, _: ksni::OfflineReason) -> bool {
        if self.app.state::<Availability>().0.swap(2, Ordering::AcqRel) == 1 {
            if let Some(window) = self.app.get_webview_window("main") {
                let _ = window.show();
            }
        }
        false
    }
    fn id(&self) -> String {
        "xyz.rensberger.chime".into()
    }
    fn title(&self) -> String {
        "Chime".into()
    }
    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        vec![self.icon.clone()]
    }
    fn tool_tip(&self) -> ksni::ToolTip {
        ksni::ToolTip {
            title: "Chime".into(),
            description: presentation(&self.state).0.into(),
            ..Default::default()
        }
    }
    fn activate(&mut self, _: i32, _: i32) {
        self.show();
    }
    fn menu(&self) -> Vec<MenuItem<Self>> {
        let (label, busy) = presentation(&self.state);
        let mut menu = vec![
            Self::item("Show Chime", true, |tray| tray.show()),
            Self::item(
                "Paste last dictation",
                self.state.can_copy
                    && !busy
                    && !self.state.delivery_pending
                    && std::env::var_os("WAYLAND_DISPLAY").is_some(),
                |tray| tray.app.state::<Sessions>().paste_last(),
            ),
        ];
        menu.push(Self::item(
            "Copy last dictation",
            self.state.can_copy,
            |tray| {
                if let Some(text) = tray.app.state::<Sessions>().last_text() {
                    tauri::async_runtime::spawn_blocking(move || {
                        if let Err(error) = crate::output::copy(&text) {
                            eprintln!("Could not copy dictation: {error}");
                        }
                    });
                }
            },
        ));
        if label != "Ready" {
            menu.push(MenuItem::Separator);
            menu.push(Self::item(label, false, |_| {}));
        }
        if busy {
            menu.push(Self::item("Cancel dictation", true, |tray| {
                tray.app.state::<Sessions>().cancel()
            }));
        }
        menu.push(MenuItem::Separator);
        menu.push(Self::item(
            super::pause_label(self.state.shortcut_paused),
            !busy,
            |tray| tray.app.state::<Sessions>().toggle_shortcut_pause(),
        ));
        menu.push(Self::item("Quit Chime", true, |tray| tray.app.exit(0)));
        menu
    }
}

pub fn setup(app: &tauri::AppHandle) -> tauri::Result<()> {
    let image = tauri::include_image!("icons/tray-linux.png");
    let icon = ksni::Icon {
        width: image.width() as i32,
        height: image.height() as i32,
        data: image
            .rgba()
            .chunks_exact(4)
            .flat_map(|p| [p[3], p[0], p[1], p[2]])
            .collect(),
    };
    let app_handle = app.clone();
    app.manage(Availability::default());
    let task = tauri::async_runtime::spawn(async move {
        loop {
            let availability = app_handle.state::<Availability>();
            availability.0.store(0, Ordering::Release);
            let mut previous = app_handle.state::<Sessions>().tray_state();
            let tray = Tray {
                app: app_handle.clone(),
                state: previous.clone(),
                icon: icon.clone(),
            };
            let handle = match tray.spawn().await {
                Ok(handle) => handle,
                Err(error) => {
                    eprintln!("Could not register Chime tray: {error}");
                    tokio::time::sleep(Duration::from_secs(15)).await;
                    continue;
                }
            };
            let _ = availability
                .0
                .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire);
            loop {
                tokio::time::sleep(Duration::from_millis(250)).await;
                if handle.is_closed() {
                    break;
                }
                let current = app_handle.state::<Sessions>().tray_state();
                if current != previous {
                    previous = current.clone();
                    if handle.update(|tray| tray.state = current).await.is_none() {
                        break;
                    }
                }
            }
            availability.0.store(0, Ordering::Release);
        }
    });
    app.manage(TrayTask(task));
    Ok(())
}
