use serde::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
};
use tauri::Manager;

#[derive(Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    #[default]
    Toggle,
    Hold,
}

#[derive(Default, Copy, Clone, Serialize, Deserialize)]
#[serde(default)]
struct Preferences {
    mode: Mode,
    enabled: bool,
}

#[derive(Clone, Serialize)]
pub struct Status {
    configuring: bool,
    backend: &'static str,
    label: Option<String>,
    error: Option<String>,
    paused: bool,
    mode: Mode,
}

pub struct Shortcuts {
    state: Mutex<Status>,
    prefs: Mutex<Preferences>,
    path: PathBuf,
    pressed: AtomicBool,
    #[cfg(target_os = "linux")]
    configure: tokio::sync::mpsc::Sender<()>,
}

impl Shortcuts {
    pub fn status(&self, paused: bool) -> Status {
        let mut status = self.state.lock().unwrap().clone();
        status.paused = paused;
        status
    }

    fn save(&self, change: impl FnOnce(&mut Preferences)) -> Result<(), String> {
        let mut current = self.prefs.lock().unwrap();
        let mut next = *current;
        change(&mut next);
        std::fs::create_dir_all(self.path.parent().ok_or("Invalid preferences path")?)
            .map_err(|e| e.to_string())?;
        let tmp = self.path.with_extension("tmp");
        std::fs::write(&tmp, serde_json::to_vec(&next).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        std::fs::rename(tmp, &self.path).map_err(|e| e.to_string())?;
        *current = next;
        self.state.lock().unwrap().mode = next.mode;
        Ok(())
    }
}

fn event(app: &tauri::AppHandle, down: bool) {
    let keys = app.state::<Shortcuts>();
    if keys.pressed.swap(down, Ordering::SeqCst) == down {
        return;
    }
    let sessions = app.state::<crate::session::Sessions>();
    if !down {
        sessions.hold(false);
        return;
    }
    if sessions.shortcut_paused() {
        return;
    }
    if sessions.needs_model_to_start() {
        crate::show_window(app);
        return;
    }
    match keys.state.lock().unwrap().mode {
        Mode::Hold => sessions.hold(true),
        Mode::Toggle => sessions.toggle(false),
    };
}

pub fn setup(app: &mut tauri::App) -> tauri::Result<()> {
    let path = app.path().app_config_dir()?.join("shortcuts.json");
    let prefs: Preferences = std::fs::read(&path)
        .ok()
        .and_then(|data| serde_json::from_slice(&data).ok())
        .unwrap_or_default();
    #[cfg(target_os = "linux")]
    let (configure, rx) = tokio::sync::mpsc::channel(1);
    app.manage(Shortcuts {
        path,
        prefs: Mutex::new(prefs),
        pressed: AtomicBool::new(false),
        state: Mutex::new(Status {
            configuring: false,
            backend: "manual",
            label: None,
            error: None,
            mode: prefs.mode,
            paused: false,
        }),
        #[cfg(target_os = "linux")]
        configure,
    });
    #[cfg(target_os = "linux")]
    {
        let app = app.handle().clone();
        tauri::async_runtime::spawn(linux::listen(app, rx));
    }
    #[cfg(target_os = "macos")]
    {
        app.handle()
            .plugin(tauri_plugin_global_shortcut::Builder::new().build())?;
        let keys = app.state::<Shortcuts>();
        keys.state.lock().unwrap().backend = "native";
        if prefs.enabled {
            if let Err(error) = register_mac(app.handle()) {
                keys.state.lock().unwrap().error = Some(error);
            }
        }
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn register_mac(app: &tauri::AppHandle) -> Result<(), String> {
    use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
    let keys = app.state::<Shortcuts>();
    if keys.state.lock().unwrap().label.is_some() {
        return Ok(());
    }
    app.global_shortcut()
        .on_shortcut("Control+Alt+Space", |app, _, e| {
            event(app, e.state == ShortcutState::Pressed)
        })
        .map_err(|e| format!("Could not register Control + Option + Space: {e}"))?;
    let mut state = keys.state.lock().unwrap();
    state.label = Some("Control + Option + Space".into());
    state.error = None;
    Ok(())
}

#[tauri::command]
pub async fn shortcut_configure(app: tauri::AppHandle) -> Result<(), String> {
    let keys = app.state::<Shortcuts>();
    #[cfg(target_os = "macos")]
    {
        register_mac(&app)?;
        keys.save(|p| p.enabled = true)
    }
    #[cfg(target_os = "linux")]
    {
        let mut status = keys.state.lock().unwrap();
        if status.backend != "portal" || status.configuring {
            return Err("Shortcut setup is unavailable.".into());
        }
        keys.configure
            .try_send(())
            .map_err(|_| "Shortcut setup is already running.".to_string())?;
        status.configuring = true;
        status.error = None;
        Ok(())
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        Err("Use a desktop shortcut on this system.".into())
    }
}

#[tauri::command]
pub fn shortcut_mode(keys: tauri::State<'_, Shortcuts>, mode: Mode) -> Result<(), String> {
    keys.save(|p| p.mode = mode)
}

#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    use ashpd::desktop::global_shortcuts::{
        Activated, Deactivated, GlobalShortcuts, NewShortcut, Shortcut, ShortcutsChanged,
    };
    use futures_util::StreamExt;
    use std::time::Duration;
    use tokio::sync::mpsc::Receiver;
    type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

    fn label(shortcuts: &[Shortcut]) -> Option<String> {
        shortcuts
            .iter()
            .find(|s| s.id() == "dictate")
            .map(|s| s.trigger_description().trim())
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    }

    fn identity(app: &tauri::AppHandle) -> Result<()> {
        use std::io::Write;
        let dir = app.path().data_dir()?.join("applications");
        let file = "xyz.rensberger.chime.desktop";
        let dirs = std::env::var_os("XDG_DATA_DIRS")
            .unwrap_or_else(|| "/usr/local/share:/usr/share".into());
        if std::env::split_paths(&dirs).any(|p| p.join("applications").join(file).is_file())
            || dir.join(file).is_file()
        {
            return Ok(());
        }
        std::fs::create_dir_all(&dir)?;
        let mut entry = std::fs::File::create_new(dir.join(file))?;
        entry.write_all(
            b"[Desktop Entry]\nType=Application\nName=Chime\nExec=chime-desktop\nNoDisplay=true\n",
        )?;
        Ok(())
    }

    pub async fn listen(app: tauri::AppHandle, mut configure: Receiver<()>) {
        let mut restore = app.state::<Shortcuts>().prefs.lock().unwrap().enabled;
        loop {
            let result = run(&app, &mut configure, restore).await;
            event(&app, false);
            let mut retry_probe = true;
            {
                let keys = app.state::<Shortcuts>();
                let mut state = keys.state.lock().unwrap();
                if state.backend == "portal" {
                    restore = state.label.is_some();
                    if let Err(error) = result {
                        state.error = Some(error.to_string());
                    }
                    retry_probe = false;
                }
                state.backend = "manual";
                state.label = None;
                state.configuring = false;
            }
            if retry_probe {
                tokio::time::sleep(Duration::from_secs(60)).await;
            }
        }
    }

    async fn run(
        app: &tauri::AppHandle,
        configure: &mut Receiver<()>,
        restore: bool,
    ) -> Result<()> {
        let portal = GlobalShortcuts::new().await?;
        let version = portal.get_property::<u32>("version").await?;
        let keys = app.state::<Shortcuts>();
        keys.state.lock().unwrap().backend = "portal";
        if !restore {
            configure.recv().await.ok_or("Shortcut setup stopped")?;
        }
        keys.state.lock().unwrap().configuring = true;
        identity(app)?;
        let _ = ashpd::register_host_app("xyz.rensberger.chime".parse()?).await;
        let session = portal.create_session().await?;
        let result = async {
            let path: String = serde_json::from_value(serde_json::to_value(&session)?)?;
            let mut signals = portal.receive_all_signals().await?;
            let mut closed = session.receive_closed().await?;
            let shortcut = NewShortcut::new("dictate", "Dictate with Chime").preferred_trigger("CTRL+ALT+space");
            let bound = portal.bind_shortcuts(&session, &[shortcut], None).await?.response()?;
            let trigger = label(bound.shortcuts());
            if trigger.is_none() { return Err("No shortcut assigned by the desktop".into()); }
            keys.save(|p| p.enabled = true)?;
            { let mut state = keys.state.lock().unwrap(); state.label = trigger; state.error = None; state.configuring = false; }
            loop {
                tokio::select! {
                    msg = signals.next() => {
                        let msg = msg.ok_or("Shortcut listener stopped")?;
                        match msg.header().member().map(|m| m.as_str()) {
                            Some("Activated") => { let e: Activated = msg.body().deserialize()?; if e.session_handle().as_str() == path && e.shortcut_id() == "dictate" { event(app,true); } }
                            Some("Deactivated") => { let e: Deactivated = msg.body().deserialize()?; if e.session_handle().as_str() == path && e.shortcut_id() == "dictate" { event(app,false); } }
                            Some("ShortcutsChanged") => { let e: ShortcutsChanged = msg.body().deserialize()?; if e.session_handle().as_str() == path { keys.state.lock().unwrap().label = label(e.shortcuts()); } }
                            _ => {}
                        };
                    }
                    _ = closed.next() => return Err("Shortcut session closed".into()),
                    _ = configure.recv() => {
                        let result = if version >= 2 { portal.configure_shortcuts(&session, None, None).await.map_err(|e| e.to_string()) } else { Err("Change the shortcut in your desktop’s keyboard settings.".into()) };
                        let mut status = keys.state.lock().unwrap();
                        status.error = result.err();
                        status.configuring = false;
                    }
                }
            }
        }.await;
        let _ = session.close().await;
        if let Err(ref e) = result {
            keys.state.lock().unwrap().error = Some(format!("Shortcut setup stopped: {e}"));
        }
        result
    }
}
