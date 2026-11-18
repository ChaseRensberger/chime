//! Text-only history with Hex's retention defaults and storage bounds.
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Read, Write},
    path::PathBuf,
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::Manager;

const MAX_ENTRIES: usize = 10_000;
const MAX_TOTAL: usize = 16 * 1024 * 1024;
const MAX_FILE: u64 = 128 * 1024 * 1024;

#[derive(Clone, Copy, Default, Deserialize, Serialize, PartialEq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum Retention {
    Off,
    Day,
    #[default]
    Week,
    Month,
    Forever,
}
impl Retention {
    fn age(self) -> Option<u64> {
        match self {
            Self::Day => Some(86_400_000),
            Self::Week => Some(7 * 86_400_000),
            Self::Month => Some(30 * 86_400_000),
            _ => None,
        }
    }
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub id: u64,
    pub timestamp_ms: u64,
    pub text: String,
    pub duration_ms: u64,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Saved {
    version: u32,
    retention: Retention,
    last_id: u64,
    entries: Vec<Entry>,
}
impl Default for Saved {
    fn default() -> Self {
        Self {
            version: 1,
            retention: Retention::Week,
            last_id: 0,
            entries: Vec::new(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Listing {
    entries: Vec<Entry>,
    total: usize,
    matches: usize,
    retention: Retention,
    error: Option<String>,
}

struct Store {
    saved: Saved,
    error: Option<String>,
    writable: bool,
    dirty: bool,
}
pub struct History {
    path: Option<PathBuf>,
    store: Mutex<Store>,
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn prune(saved: &mut Saved, now: u64) -> bool {
    let before = saved.entries.len();
    if let Some(age) = saved.retention.age() {
        saved
            .entries
            .retain(|e| e.timestamp_ms >= now.saturating_sub(age));
    }
    let mut bytes = 0;
    let keep = saved
        .entries
        .iter()
        .rev()
        .take(MAX_ENTRIES)
        .take_while(|e| {
            bytes += e.text.len();
            bytes <= MAX_TOTAL
        })
        .count();
    saved.entries.drain(..saved.entries.len() - keep);
    saved.entries.len() != before
}

impl History {
    pub fn open(path: PathBuf) -> Self {
        let loaded = (|| -> Result<Saved, String> {
            let file = match fs::File::open(&path) {
                Ok(file) => file,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Saved::default()),
                Err(e) => return Err(e.to_string()),
            };
            if file.metadata().map_err(|e| e.to_string())?.len() > MAX_FILE {
                return Err("History file is too large".into());
            }
            let mut bytes = Vec::new();
            file.take(MAX_FILE + 1)
                .read_to_end(&mut bytes)
                .map_err(|e| e.to_string())?;
            if bytes.len() as u64 > MAX_FILE {
                return Err("History file is too large".into());
            }
            let mut saved: Saved = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
            if saved.version != 1 {
                return Err("Unsupported history version".into());
            }
            saved
                .entries
                .sort_by_key(|entry| (entry.timestamp_ms, entry.id));
            for entry in &saved.entries {
                saved.last_id = saved.last_id.max(entry.id);
            }
            Ok(saved)
        })();
        let (saved, error, writable) = match loaded {
            Ok(saved) => (saved, None, true),
            Err(error) => (
                Saved::default(),
                Some(format!(
                    "History could not be read: {error}. The existing file has been left intact."
                )),
                false,
            ),
        };
        let history = Self {
            path: Some(path),
            store: Mutex::new(Store {
                saved,
                error,
                writable,
                dirty: writable,
            }),
        };
        history.prune();
        history
    }

    #[cfg(test)]
    pub fn memory() -> Self {
        Self {
            path: None,
            store: Mutex::new(Store {
                saved: Saved::default(),
                error: None,
                writable: true,
                dirty: false,
            }),
        }
    }

    fn persist(&self, saved: &Saved) -> Result<(), String> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        let parent = path.parent().ok_or("Invalid history path")?;
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        let temp = path.with_extension("tmp");
        let mut options = fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temp).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            file.set_permissions(fs::Permissions::from_mode(0o600))
                .map_err(|e| e.to_string())?;
        }
        serde_json::to_writer(&mut file, saved).map_err(|e| e.to_string())?;
        file.flush()
            .and_then(|_| file.sync_all())
            .map_err(|e| e.to_string())?;
        fs::rename(&temp, path).map_err(|e| e.to_string())
    }

    fn change(&self, update: impl FnOnce(&mut Saved)) -> Result<(), String> {
        self.change_if(|next, _| {
            update(next);
            None
        })
        .map(|_| ())
    }

    fn retention(&self, retention: Retention, confirmed: usize) -> Result<usize, String> {
        self.change_if(|next, now| {
            let count = retention.age().map_or(0, |age| {
                next.entries
                    .iter()
                    .filter(|e| e.timestamp_ms < now.saturating_sub(age))
                    .count()
            });
            if count > confirmed {
                return Some(count);
            }
            next.retention = retention;
            None
        })
    }

    fn change_if(
        &self,
        update: impl FnOnce(&mut Saved, u64) -> Option<usize>,
    ) -> Result<usize, String> {
        let mut store = self.store.lock().unwrap();
        if !store.writable {
            return Err(store.error.clone().unwrap_or_default());
        }
        let mut next = store.saved.clone();
        let now = now_ms();
        if let Some(count) = update(&mut next, now) {
            return Ok(count);
        }
        prune(&mut next, now);
        match self.persist(&next) {
            Ok(()) => {
                store.saved = next;
                store.error = None;
                store.dirty = false;
                Ok(0)
            }
            Err(error) => {
                store.error = Some(format!("History could not be saved: {error}"));
                Err(error)
            }
        }
    }

    pub fn record(&self, text: String, duration_ms: u64) -> Result<(), String> {
        if text.trim().is_empty() || self.store.lock().unwrap().saved.retention == Retention::Off {
            return Ok(());
        }
        if text.len() > MAX_TOTAL {
            return Err("Transcript exceeds the history storage limit.".into());
        }
        self.change(|saved| {
            if saved.retention == Retention::Off {
                return;
            }
            saved.last_id = saved.last_id.saturating_add(1).max(now_ms());
            saved.entries.push(Entry {
                id: saved.last_id,
                timestamp_ms: now_ms(),
                text,
                duration_ms,
            });
        })
    }

    pub fn prune(&self) {
        let mut store = self.store.lock().unwrap();
        if !store.writable {
            return;
        }
        if prune(&mut store.saved, now_ms()) || store.dirty {
            match self.persist(&store.saved) {
                Err(error) => {
                    store.error = Some(error);
                    store.dirty = true;
                }
                Ok(()) => {
                    store.error = None;
                    store.dirty = false;
                }
            }
        }
    }

    pub fn list(&self, query: &str) -> Listing {
        self.prune();
        let store = self.store.lock().unwrap();
        let needle = query.trim().to_lowercase();
        let matches: Vec<_> = store
            .saved
            .entries
            .iter()
            .rev()
            .filter(|e| e.text.to_lowercase().contains(&needle))
            .collect();
        Listing {
            entries: matches.iter().take(200).map(|e| (*e).clone()).collect(),
            total: store.saved.entries.len(),
            matches: matches.len(),
            retention: store.saved.retention,
            error: store.error.clone(),
        }
    }
}

#[tauri::command]
pub async fn history_list(app: tauri::AppHandle, query: String) -> Result<Listing, String> {
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<std::sync::Arc<History>>().list(&query)
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn history_update(
    app: tauri::AppHandle,
    action: String,
    id: Option<u64>,
    retention: Option<Retention>,
    confirmed: Option<usize>,
) -> Result<usize, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let history = app.state::<std::sync::Arc<History>>();
        match action.as_str() {
            "delete" => {
                let id = id.ok_or("Missing entry")?;
                history
                    .change(|saved| saved.entries.retain(|e| e.id != id))
                    .map(|_| 0)
            }
            "clear" => history.change(|saved| saved.entries.clear()).map(|_| 0),
            "retention" => {
                let retention = retention.ok_or("Missing retention")?;
                history.retention(retention, confirmed.unwrap_or_default())
            }
            _ => Err("Unknown history action".into()),
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn history_copy(app: tauri::AppHandle, id: u64) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let history = app.state::<std::sync::Arc<History>>();
        history.prune();
        let text = history
            .store
            .lock()
            .unwrap()
            .saved
            .entries
            .iter()
            .find(|e| e.id == id)
            .map(|e| e.text.clone())
            .ok_or("Entry no longer available")?;
        crate::output::copy(&text)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retention_and_off_match_hex() {
        let mut saved = Saved::default();
        let day = 86_400_000;
        saved.entries = vec![
            Entry {
                id: 1,
                timestamp_ms: 0,
                text: "old".into(),
                duration_ms: 1,
            },
            Entry {
                id: 2,
                timestamp_ms: day * 7,
                text: "new".into(),
                duration_ms: 1,
            },
        ];
        prune(&mut saved, day * 8);
        assert_eq!(saved.entries.len(), 1);
        saved.retention = Retention::Off;
        prune(&mut saved, day * 100);
        assert_eq!(saved.entries.len(), 1);
        let history = History::memory();
        history.record("kept".into(), 1000).unwrap();
        history
            .change(|saved| saved.retention = Retention::Off)
            .unwrap();
        history.record("not kept".into(), 1000).unwrap();
        assert_eq!(history.list("").total, 1);
    }

    #[test]
    fn disk_roundtrip_delete_and_corruption() {
        let dir = std::env::temp_dir().join(format!("chime-history-test-{}", std::process::id()));
        let path = dir.join("history.json");
        let history = History::open(path.clone());
        history.record("Find these words".into(), 1234).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        let loaded = History::open(path.clone());
        assert_eq!(loaded.list("THESE").entries[0].duration_ms, 1234);
        loaded.change(|saved| saved.entries.clear()).unwrap();
        assert_eq!(History::open(path.clone()).list("").total, 0);
        fs::write(&path, "broken").unwrap();
        let broken = History::open(path.clone());
        assert!(broken.record("do not overwrite".into(), 1).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "broken");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn bounds_preserve_utf8_and_newest_entries() {
        let text = "é".repeat(20_000);
        let history = History::memory();
        history.record(text.clone(), 600_000).unwrap();
        assert_eq!(history.list("").entries[0].text, text);
        let mut saved = Saved::default();
        saved.entries = (0..MAX_ENTRIES + 2)
            .map(|id| Entry {
                id: id as u64,
                timestamp_ms: now_ms(),
                text: "a".into(),
                duration_ms: 1,
            })
            .collect();
        prune(&mut saved, now_ms());
        assert_eq!(saved.entries.len(), MAX_ENTRIES);
        assert_eq!(saved.entries[0].id, 2);
        saved.entries = (0..1100)
            .map(|id| Entry {
                id,
                timestamp_ms: now_ms(),
                text: "x".repeat(16 * 1024),
                duration_ms: 1,
            })
            .collect();
        prune(&mut saved, now_ms());
        assert_eq!(saved.entries.len(), MAX_TOTAL / (16 * 1024));
        assert_eq!(saved.entries.last().unwrap().id, 1099);
    }

    #[test]
    fn retention_changes_only_after_approval_of_the_current_count() {
        let history = History::memory();
        history.retention(Retention::Forever, 0).unwrap();
        history.record("old".into(), 100).unwrap();
        history.store.lock().unwrap().saved.entries[0].timestamp_ms = 0;
        assert_eq!(history.retention(Retention::Day, 0).unwrap(), 1);
        assert_eq!(history.list("").total, 1);
        assert_eq!(history.list("").retention, Retention::Forever);
        assert_eq!(history.retention(Retention::Day, 1).unwrap(), 0);
        assert_eq!(history.list("").total, 0);
    }
}
