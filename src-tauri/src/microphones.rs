use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf, sync::Mutex};

#[derive(Clone, Deserialize, Serialize)]
struct Input {
    id: String,
    name: String,
    backend: String,
    occurrence: u32,
    stable: bool,
    default: bool,
}

fn list() -> Result<Vec<Input>, String> {
    handy_recorder::list_input_devices()
        .map_err(|e| format!("Couldn’t list microphones: {e}"))
        .map(|devices| {
            devices
                .into_iter()
                .filter(|d| !d.is_monitor)
                .map(|d| Input {
                    id: d.id,
                    name: d.name,
                    occurrence: d.occurrence,
                    backend: d.backend,
                    stable: d.id_is_stable,
                    default: d.is_default,
                })
                .collect()
        })
}

fn resolve<'a>(saved: &Input, inputs: &'a [Input]) -> Option<&'a Input> {
    inputs.iter().find(|d| {
        d.backend == saved.backend
            && if saved.stable {
                d.id == saved.id
            } else {
                d.name == saved.name && d.occurrence == saved.occurrence
            }
    })
}

pub struct Microphones {
    path: PathBuf,
    preferred: Mutex<Option<Input>>,
}

#[derive(Serialize)]
pub struct Listing {
    devices: Vec<Input>,
    selected: Option<String>,
    missing: Option<String>,
}

impl Microphones {
    pub fn load(path: PathBuf) -> Self {
        let preferred = fs::read(&path)
            .ok()
            .and_then(|v| serde_json::from_slice(&v).ok())
            .flatten();
        Self {
            path,
            preferred: Mutex::new(preferred),
        }
    }

    pub fn capture_device(&self) -> Result<Option<String>, String> {
        let saved = self.preferred.lock().unwrap().clone();
        let Some(saved) = saved else { return Ok(None) };
        Ok(resolve(&saved, &list()?).map(|d| d.id.clone()))
    }

    fn listing(&self) -> Result<Listing, String> {
        let devices = list()?;
        let saved = self.preferred.lock().unwrap();
        let selected = saved
            .as_ref()
            .and_then(|s| resolve(s, &devices))
            .map(|d| d.id.clone());
        let missing = saved
            .as_ref()
            .filter(|_| selected.is_none())
            .map(|s| s.name.clone());
        Ok(Listing {
            devices,
            selected,
            missing,
        })
    }

    fn select(&self, id: Option<String>) -> Result<(), String> {
        let next = match id {
            Some(id) => Some(
                list()?
                    .into_iter()
                    .find(|d| d.id == id)
                    .ok_or("That microphone is no longer connected.")?,
            ),
            None => None,
        };
        let mut saved = self.preferred.lock().unwrap();
        let parent = self
            .path
            .parent()
            .ok_or("Invalid microphone preference path")?;
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        let tmp = self.path.with_extension("tmp");
        fs::write(&tmp, serde_json::to_vec(&next).map_err(|e| e.to_string())?)
            .and_then(|_| fs::rename(tmp, &self.path))
            .map_err(|e| e.to_string())?;
        *saved = next;
        Ok(())
    }
}

#[tauri::command]
pub async fn microphones_list(
    sessions: tauri::State<'_, crate::session::Sessions>,
) -> Result<Listing, String> {
    let mics = sessions.microphones.clone();
    tauri::async_runtime::spawn_blocking(move || mics.listing())
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn microphone_select(
    sessions: tauri::State<'_, crate::session::Sessions>,
    id: Option<String>,
) -> Result<(), String> {
    let mics = sessions.microphones.clone();
    tauri::async_runtime::spawn_blocking(move || mics.select(id))
        .await
        .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires a connected microphone"]
    fn input_selection_survives_restart_and_opens_selected_device() {
        let path = std::env::temp_dir().join(format!(
            "chime-microphone-smoke-{}.json",
            std::process::id()
        ));
        let mics = Microphones::load(path.clone());
        let device = list()
            .unwrap()
            .into_iter()
            .next()
            .expect("a connected microphone");
        mics.select(Some(device.id.clone())).unwrap();
        let reloaded = Microphones::load(path.clone());
        let config = handy_recorder::RecorderConfig {
            device: reloaded.capture_device().unwrap(),
            ..handy_recorder::RecorderConfig::speech()
        };
        assert_eq!(config.device.as_deref(), Some(device.id.as_str()));
        let recorder =
            handy_recorder::Recorder::<handy_recorder::CollectingSink>::open(config).unwrap();
        assert_eq!(recorder.info().device.id, device.id);
        reloaded.select(None).unwrap();
        assert!(reloaded.capture_device().unwrap().is_none());
        fs::remove_file(path).unwrap();
    }
    #[test]
    fn reconnect_respects_device_identity() {
        let saved = Input {
            id: "a".into(),
            name: "USB".into(),
            backend: "test".into(),
            occurrence: 0,
            stable: true,
            default: false,
        };
        let inputs = [Input {
            id: "b".into(),
            ..saved.clone()
        }];
        assert!(resolve(&saved, &inputs).is_none());
        assert!(resolve(&saved, &[]).is_none());
        let unstable = Input {
            stable: false,
            ..saved.clone()
        };
        assert_eq!(resolve(&unstable, &inputs).unwrap().id, "b");
        assert!(resolve(
            &Input {
                occurrence: 1,
                ..unstable
            },
            &inputs
        )
        .is_none());
        assert_eq!(
            resolve(&saved, std::slice::from_ref(&saved)).unwrap().id,
            "a"
        );
    }
}
