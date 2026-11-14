#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod feedback;
mod history;
mod inference;
mod model;
mod models;
mod output;
mod session;
mod tray;
use tauri::Manager;

fn show_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn dispatch(app: &tauri::AppHandle, args: &[String]) {
    let Some(sessions) = app.try_state::<session::Sessions>() else {
        return;
    };
    if args.iter().any(|a| a == "--quit") {
        app.exit(0);
    } else if args.iter().any(|a| a == "--cancel") {
        sessions.cancel();
    } else if args
        .iter()
        .any(|a| a == "--toggle" || a == "--toggle-terminal")
    {
        if sessions.model_available() {
            sessions.toggle(args.iter().any(|a| a == "--toggle-terminal"));
        } else {
            show_window(app);
        }
    } else {
        show_window(app);
    }
}

fn main() {
    let mut args = std::env::args_os().skip(1);
    if args.next().as_deref() == Some(std::ffi::OsStr::new("--transcribe-worker")) {
        if let Some(model) = args.next() {
            inference::worker(model.into());
        }
        return;
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, _| {
            dispatch(app, &args)
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let config = app.path().app_config_dir()?.join("model.json");
            let fallback = std::env::var_os("CHIME_MODEL_PATH")
                .map(std::path::PathBuf::from)
                .unwrap_or(
                    app.path()
                        .app_data_dir()?
                        .join("models/parakeet-unified-en-0.6b-Q8_0.gguf"),
                );
            let model = model::load(&config, fallback);
            if !config.exists() && std::env::var_os("CHIME_MODEL_PATH").is_some() && model.is_file()
            {
                if let Err(error) = model::save(&config, &model) {
                    eprintln!("Could not remember model: {error}");
                }
            }
            let history = std::sync::Arc::new(history::History::open(
                app.path().app_data_dir()?.join("history.json"),
            ));
            app.manage(session::Sessions::new(model, config, history.clone()));
            let feedback =
                feedback::Feedback::new(app.path().app_config_dir()?.join("feedback.json"));
            app.state::<session::Sessions>()
                .set_feedback(feedback.sender.clone());
            app.manage(feedback);
            app.manage(history);
            app.manage(std::sync::Arc::new(models::Models::new(
                app.path().app_data_dir()?.join("models"),
            )));
            tray::setup(app.handle())?;
            dispatch(app.handle(), &std::env::args().collect::<Vec<_>>());
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" && window.hide().is_ok() {
                    api.prevent_close();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            session::session_snapshot,
            feedback::feedback_preferences,
            feedback::feedback_save,
            feedback::feedback_preview,
            session::session_action,
            models::models_snapshot,
            models::models_start,
            models::models_cancel,
            models::models_open_info,
            output::copy_result,
            history::history_list,
            history::history_update,
            history::history_copy
        ])
        .build(tauri::generate_context!())
        .expect("could not run Chime")
        .run(|app, event| {
            #[cfg(target_os = "macos")]
            if matches!(event, tauri::RunEvent::Reopen { .. }) {
                show_window(app);
            }
            if matches!(event, tauri::RunEvent::Exit) {
                if let Some(task) = app.try_state::<tray::TrayTask>() {
                    task.stop();
                }
                app.state::<session::Sessions>().shutdown();
            }
        });
}
