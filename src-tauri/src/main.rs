#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod history;
mod inference;
mod model;
mod output;
mod session;
use tauri::Manager;

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
        } else if let Some(window) = app.get_webview_window("main") {
            let _ = window.show();
            let _ = window.set_focus();
        }
    } else if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
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
            app.manage(history);
            dispatch(app.handle(), &std::env::args().collect::<Vec<_>>());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            session::session_snapshot,
            session::session_action,
            model::choose_model,
            output::copy_result,
            history::history_list,
            history::history_update,
            history::history_copy
        ])
        .build(tauri::generate_context!())
        .expect("could not run Chime")
        .run(|app, event| {
            if matches!(event, tauri::RunEvent::Exit) {
                app.state::<session::Sessions>().shutdown();
            }
        });
}
