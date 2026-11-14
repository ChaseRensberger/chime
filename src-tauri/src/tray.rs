//! Background controls share the recording actor's state.
use std::time::Duration;
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    Manager,
};

pub struct TrayTask(tauri::async_runtime::JoinHandle<()>);

impl TrayTask {
    pub fn stop(&self) {
        self.0.abort();
    }
}

pub(crate) fn presentation(state: &crate::session::TrayState) -> (&'static str, bool) {
    match state.phase.as_str() {
        "starting" => ("Starting microphone…", true),
        "recording" => ("Listening", true),
        "processing" => ("Transcribing…", true),
        "error" => ("Needs attention — open Chime", false),
        _ if state.paste_failed => ("Paste failed — copy transcript and paste manually", false),
        _ if !state.model_available => ("Set up speech model", false),
        _ => ("Ready", false),
    }
}

pub fn setup(app: &tauri::AppHandle) -> tauri::Result<()> {
    let initial = app.state::<crate::session::Sessions>().tray_state();
    let (label, busy) = presentation(&initial);
    let status = MenuItem::with_id(app, "status", label, false, None::<&str>)?;
    let show = MenuItem::with_id(app, "show", "Show Chime", true, None::<&str>)?;
    let copy = MenuItem::with_id(
        app,
        "copy-last",
        "Copy last transcript",
        initial.can_copy,
        None::<&str>,
    )?;
    let cancel = MenuItem::with_id(app, "cancel", "Cancel dictation", busy, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &status,
            &PredefinedMenuItem::separator(app)?,
            &show,
            &copy,
            &cancel,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )?;
    let copy_action = copy.clone();
    // macOS template images follow the menu bar's appearance automatically.
    // Linux uses a light mark with a dark outline for either panel background.
    #[cfg(target_os = "macos")]
    let icon = tauri::include_image!("icons/tray.png");
    #[cfg(not(target_os = "macos"))]
    let icon = tauri::include_image!("icons/tray-linux.png");
    let tray = TrayIconBuilder::with_id("chime")
        .icon(icon)
        .icon_as_template(cfg!(target_os = "macos"))
        .tooltip(format!("Chime — {label}"))
        .menu(&menu)
        .on_menu_event(move |app, event| match event.id.as_ref() {
            "show" => crate::show_window(app),
            "cancel" => app.state::<crate::session::Sessions>().cancel(),
            "copy-last" => {
                if let Some(text) = app.state::<crate::session::Sessions>().last_text() {
                    let item = copy_action.clone();
                    tauri::async_runtime::spawn_blocking(move || {
                        let label = match crate::output::copy(&text) {
                            Ok(()) => "Copied — copy again",
                            Err(error) => {
                                eprintln!("Could not copy last transcript: {error}");
                                "Copy failed — try again"
                            }
                        };
                        let _ = item.set_text(label);
                    });
                }
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;

    let handle = app.clone();
    let task = tauri::async_runtime::spawn(async move {
        let mut previous = Some(initial);
        loop {
            let current = handle.state::<crate::session::Sessions>().tray_state();
            if previous.as_ref() != Some(&current) {
                let (label, busy) = presentation(&current);
                let result = status
                    .set_text(label)
                    .and_then(|()| cancel.set_enabled(busy))
                    .and_then(|()| copy.set_enabled(current.can_copy))
                    .and_then(|()| copy.set_text("Copy last transcript"))
                    .and_then(|()| tray.set_tooltip(Some(format!("Chime — {label}"))));
                match result {
                    Ok(()) => previous = Some(current),
                    Err(error) => eprintln!("Could not update Chime tray: {error}"),
                }
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    });
    app.manage(TrayTask(task));
    Ok(())
}
