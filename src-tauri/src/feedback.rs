//! Native audio feedback works while the main window is hidden.
use serde::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc, Arc, Mutex,
    },
    time::{Duration, Instant},
};
use tauri::Manager;

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct Preferences {
    pub enabled: bool,
    pub volume: f32,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            enabled: true,
            volume: 0.35,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Cue {
    Start,
    Stop,
    Cancel,
}

pub fn transition(previous: &str, next: &str) -> Option<Cue> {
    if next == "recording" && previous != "recording" {
        Some(Cue::Start)
    } else if previous == "recording" && next != "recording" {
        Some(if next == "processing" {
            Cue::Stop
        } else {
            Cue::Cancel
        })
    } else {
        None
    }
}

#[derive(Clone)]
pub struct Sender {
    tx: mpsc::SyncSender<Request>,
    generation: Arc<AtomicU64>,
}

#[derive(Debug)]
struct Request {
    cue: Cue,
    requested: Instant,
    generation: Option<u64>,
    reply: Option<mpsc::SyncSender<Result<(), String>>>,
}

impl Sender {
    pub fn invalidate(&self) {
        self.generation.fetch_add(1, Ordering::SeqCst);
    }

    pub fn transition(&self, previous: &str, next: &str) {
        let cue = transition(previous, next);
        if cue.is_none() && next != "starting" {
            return;
        }
        let generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        let Some(cue) = cue else { return };
        // Never wait on the audio capture callback.
        let _ = self.tx.try_send(Request {
            cue,
            requested: Instant::now(),
            generation: Some(generation),
            reply: None,
        });
    }
}

pub struct Feedback {
    preferences: Arc<Mutex<Preferences>>,
    path: PathBuf,
    pub sender: Sender,
}

impl Feedback {
    pub fn new(path: PathBuf) -> Self {
        let preferences = std::fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Preferences>(&bytes).ok())
            .filter(|p| p.volume.is_finite() && (0.0..=1.0).contains(&p.volume))
            .unwrap_or_default();
        let preferences = Arc::new(Mutex::new(preferences));
        let settings = preferences.clone();
        let (tx, rx) = mpsc::sync_channel::<Request>(4);
        let generation = Arc::new(AtomicU64::new(0));
        let current_generation = generation.clone();
        std::thread::spawn(move || {
            let mut device = None;
            loop {
                let request = match rx.recv_timeout(Duration::from_secs(30)) {
                    Ok(event) => event,
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        device = None;
                        continue;
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                };
                let result = deliver(
                    &mut device,
                    || {
                        let preferences = *settings.lock().unwrap();
                        if !preferences.enabled || preferences.volume == 0.0 {
                            return Err("Recording sounds are muted.".to_string());
                        }
                        validate_request(&request, &current_generation, Instant::now())?;
                        Ok(preferences.volume)
                    },
                    || {
                        rodio::DeviceSinkBuilder::open_default_sink()
                            .map_err(|error| format!("Sound output unavailable: {error}"))
                    },
                    |device, volume| {
                        let player = rodio::Player::connect_new(device.mixer());
                        player.append(rodio::buffer::SamplesBuffer::new(
                            rodio::nz!(1),
                            rodio::nz!(48_000),
                            samples(request.cue, volume),
                        ));
                        let deadline = Instant::now() + Duration::from_secs(2);
                        while !player.empty() {
                            if Instant::now() >= deadline {
                                return Err("Sound output stopped responding.".into());
                            }
                            std::thread::sleep(Duration::from_millis(10));
                        }
                        Ok(())
                    },
                );
                if let Some(reply) = request.reply {
                    let _ = reply.send(result);
                }
            }
        });
        Self {
            preferences,
            path,
            sender: Sender { tx, generation },
        }
    }
}

fn validate_request(request: &Request, generation: &AtomicU64, now: Instant) -> Result<(), String> {
    if request
        .generation
        .is_some_and(|id| id != generation.load(Ordering::SeqCst))
    {
        return Err("Recording state changed.".into());
    }
    if now.saturating_duration_since(request.requested) > Duration::from_millis(500) {
        return Err("Sound feedback was delayed. Try again.".into());
    }
    Ok(())
}

// Validation failures skip only the cue; only playback failures discard a healthy device.
fn deliver<D>(
    device: &mut Option<D>,
    validate: impl Fn() -> Result<f32, String>,
    open: impl FnOnce() -> Result<D, String>,
    play: impl FnOnce(&D, f32) -> Result<(), String>,
) -> Result<(), String> {
    validate()?;
    if device.is_none() {
        *device = Some(open()?);
    }
    let volume = validate()?;
    let result = play(device.as_ref().unwrap(), volume);
    if result.is_err() {
        *device = None;
    }
    result
}

fn samples(cue: Cue, volume: f32) -> Vec<f32> {
    let (from, to) = match cue {
        Cue::Start => (660.0, 880.0),
        Cue::Stop => (660.0, 440.0),
        Cue::Cancel => (330.0, 260.0),
    };
    let duration = 0.12;
    (0..5760)
        .map(|index| {
            let time = index as f32 / 48_000.0;
            let envelope = (std::f32::consts::PI * time / duration).sin().powi(2);
            let phase = std::f32::consts::TAU
                * (from * time + (to - from) * time * time / (2.0 * duration));
            phase.sin() * envelope * volume * 0.25
        })
        .collect()
}

#[tauri::command]
pub fn feedback_preferences(app: tauri::AppHandle) -> Preferences {
    *app.state::<Feedback>().preferences.lock().unwrap()
}

#[tauri::command]
pub fn feedback_save(app: tauri::AppHandle, preferences: Preferences) -> Result<(), String> {
    if !preferences.volume.is_finite() || !(0.0..=1.0).contains(&preferences.volume) {
        return Err("Sound volume must be between 0 and 1.".into());
    }
    let feedback = app.state::<Feedback>();
    let mut current = feedback.preferences.lock().unwrap();
    let parent = feedback.path.parent().ok_or("Invalid preferences path")?;
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let temporary = feedback.path.with_extension("tmp");
    std::fs::write(
        &temporary,
        serde_json::to_vec(&preferences).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    std::fs::rename(temporary, &feedback.path).map_err(|e| e.to_string())?;
    *current = preferences;
    Ok(())
}

#[tauri::command]
pub async fn feedback_preview(app: tauri::AppHandle) -> Result<(), String> {
    let (tx, rx) = mpsc::sync_channel(1);
    app.state::<Feedback>()
        .sender
        .tx
        .try_send(Request {
            cue: Cue::Start,
            requested: Instant::now(),
            generation: None,
            reply: Some(tx),
        })
        .map_err(|_| "Sound feedback is busy. Try again.".to_string())?;
    tauri::async_runtime::spawn_blocking(move || {
        rx.recv_timeout(Duration::from_secs(4))
            .map_err(|_| "Sound preview timed out.".to_string())?
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancellation_during_device_open_suppresses_start_cue() {
        let (tx, rx) = mpsc::sync_channel(4);
        let sender = Sender {
            tx,
            generation: Arc::new(AtomicU64::new(0)),
        };
        sender.transition("starting", "recording");
        let request = rx.recv().unwrap();
        let now = request.requested;
        let mut device = None;
        let result = deliver(
            &mut device,
            || validate_request(&request, &sender.generation, now).map(|()| 0.35),
            || {
                sender.transition("recording", "idle");
                Ok(())
            },
            |_, _| panic!("A cancelled recording must not play its start cue"),
        );
        assert!(result.is_err());
        assert!(
            device.is_some(),
            "Cancellation must preserve the healthy output"
        );
        let cancel = rx.recv().unwrap();
        assert!(validate_request(&cancel, &sender.generation, now).is_ok());
        sender.transition("idle", "starting");
        assert!(validate_request(&cancel, &sender.generation, now).is_err());
    }

    #[test]
    fn slow_initialization_keeps_device_for_the_next_cue() {
        let start = Instant::now();
        let now = std::cell::Cell::new(start);
        let generation = AtomicU64::new(0);
        let mut request = Request {
            cue: Cue::Start,
            requested: start,
            generation: None,
            reply: None,
        };
        let mut device = None;
        let result = deliver(
            &mut device,
            || validate_request(&request, &generation, now.get()).map(|()| 0.35),
            || {
                now.set(start + Duration::from_millis(600));
                Ok(())
            },
            |_, _| panic!("The expired cue must not play"),
        );
        assert!(result.is_err());
        assert!(device.is_some());
        request.requested = now.get();
        let played = std::cell::Cell::new(false);
        deliver(
            &mut device,
            || validate_request(&request, &generation, now.get()).map(|()| 0.35),
            || panic!("A healthy device must be reused"),
            |_, _| {
                played.set(true);
                Ok(())
            },
        )
        .unwrap();
        assert!(played.get());
    }

    #[test]
    fn playback_failure_discards_device_for_retry() {
        let mut device = Some(());
        assert!(deliver(
            &mut device,
            || Ok(0.35),
            || panic!("Already open"),
            |_, _| Err("Disconnected".into())
        )
        .is_err());
        assert!(device.is_none());
        deliver(&mut device, || Ok(0.35), || Ok(()), |_, _| Ok(())).unwrap();
        assert!(device.is_some());
    }

    #[test]
    #[ignore = "plays three brief sounds on the default output device"]
    fn native_sound_smoke() {
        let feedback = Feedback::new(PathBuf::new());
        for cue in [Cue::Start, Cue::Stop, Cue::Cancel] {
            let (tx, rx) = mpsc::sync_channel(1);
            feedback
                .sender
                .tx
                .try_send(Request {
                    cue,
                    requested: Instant::now(),
                    generation: None,
                    reply: Some(tx),
                })
                .unwrap();
            rx.recv_timeout(Duration::from_secs(4)).unwrap().unwrap();
        }
    }

    #[test]
    fn cues_follow_capture_readiness_and_stop_only_once() {
        assert_eq!(transition("idle", "starting"), None);
        assert_eq!(transition("starting", "recording"), Some(Cue::Start));
        assert_eq!(transition("recording", "recording"), None);
        assert_eq!(transition("recording", "processing"), Some(Cue::Stop));
        assert_eq!(transition("processing", "complete"), None);
        assert_eq!(transition("recording", "idle"), Some(Cue::Cancel));
        assert_eq!(transition("recording", "error"), Some(Cue::Cancel));
        assert_eq!(transition("starting", "error"), None);
    }

    #[test]
    fn tones_are_short_bounded_and_fade_at_both_ends() {
        for cue in [Cue::Start, Cue::Stop, Cue::Cancel] {
            let audio = samples(cue, 1.0);
            assert_eq!(audio.len(), 5760);
            assert!(audio.iter().all(|s| s.is_finite() && s.abs() <= 0.25));
            assert_eq!(audio[0], 0.0);
            assert!(audio.last().unwrap().abs() < 0.00001);
        }
        assert!(samples(Cue::Start, 0.0).iter().all(|s| *s == 0.0));
    }
}
