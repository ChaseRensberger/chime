#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod feedback;
mod history;
mod inference;
mod microphones;
mod model;
mod models;
mod output;
mod overlay;
mod permissions;
mod session;
mod shortcuts;
mod tray;
mod updates;
use tauri::Manager;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

fn launch_commands(
    initial: Vec<String>,
) -> (UnboundedSender<Vec<String>>, UnboundedReceiver<Vec<String>>) {
    let (tx, rx) = unbounded_channel();
    tx.send(initial).expect("launch command receiver exists");
    (tx, rx)
}

fn show_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn dispatch(app: &tauri::AppHandle, args: &[String]) {
    let sessions = app.state::<session::Sessions>();
    if args.iter().any(|a| a == "--quit") {
        app.exit(0);
    } else if args.iter().any(|a| a == "--cancel") {
        sessions.cancel();
    } else if args.iter().any(|a| a == "--stop") {
        sessions.hold(false);
    } else if args.iter().any(|a| a == "--paste-last") {
        sessions.paste_last();
    } else if args
        .iter()
        .any(|a| a == "--start" || a == "--toggle" || a == "--toggle-terminal")
    {
        if sessions.shortcut_paused() {
            return;
        }
        if !sessions.needs_model_to_start() {
            if args.iter().any(|a| a == "--start") {
                sessions.hold(true);
            } else {
                sessions.toggle(args.iter().any(|a| a == "--toggle-terminal"));
            }
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
    let (commands, mut incoming) = launch_commands(std::env::args().collect());
    tauri::Builder::default()
        .manage(updates::Updates::default())
        .plugin(tauri_plugin_single_instance::init(move |_, args, _| {
            let _ = commands.send(args);
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(move |app| {
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
            shortcuts::setup(app)?;
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
            overlay::setup(app.handle())?;
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                while let Some(args) = incoming.recv().await {
                    dispatch(&handle, &args);
                }
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main"
                    && tray::available(window.app_handle())
                    && window.hide().is_ok()
                {
                    api.prevent_close();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            microphones::microphones_list,
            microphones::microphone_select,
            updates::update_check,
            updates::update_download,
            permissions::permissions_status,
            permissions::permissions_action,
            session::session_snapshot,
            shortcuts::shortcut_configure,
            shortcuts::shortcut_record,
            shortcuts::shortcut_bind,
            shortcuts::shortcut_mode,
            feedback::feedback_preferences,
            feedback::feedback_save,
            feedback::feedback_preview,
            overlay::overlay_status,
            overlay::overlay_save,
            overlay::overlay_preview,
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

#[cfg(test)]
mod launch_tests {
    #[test]
    fn cold_start_press_precedes_release_received_before_setup() {
        let args = |flag: &str| vec!["chime".to_owned(), flag.to_owned()];
        let (forward, mut incoming) = super::launch_commands(args("--start"));
        forward.send(args("--stop")).unwrap();
        assert_eq!(incoming.try_recv().unwrap(), args("--start"));
        forward.send(args("--toggle")).unwrap();
        assert_eq!(incoming.try_recv().unwrap(), args("--stop"));
        assert_eq!(incoming.try_recv().unwrap(), args("--toggle"));
        assert!(incoming.try_recv().is_err());
    }
}
