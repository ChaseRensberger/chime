use handy_recorder::{AudioChunk, Recorder, RecorderConfig, Sink};
use serde::Serialize;
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc, Arc, Mutex,
    },
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const MAX_SAMPLES: usize = 16_000 * 60;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Transcript {
    text: String,
    duration_ms: u64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    phase: String,
    started_at: Option<u64>,
    last_result: Option<Transcript>,
    message: Option<String>,
    level: f32,
    model_path: String,
    model_loaded: bool,
    shortcut_command: String,
}

impl Snapshot {
    fn phase(&mut self, phase: &str, message: Option<String>) {
        self.phase = phase.into();
        self.message = message;
        self.level = 0.0;
        if phase != "recording" {
            self.started_at = None;
        }
    }
}

enum Action {
    Start,
    Toggle(bool),
    Finish(Option<u64>),
    Cancel,
    Clear,
    Failed(u64, String),
    Complete(u64, u64, Result<String, String>),
    Delivered(u64, Result<(), String>),
    SelectModel(PathBuf, mpsc::SyncSender<Result<(), String>>),
    Shutdown,
}

pub struct Sessions {
    state: Arc<Mutex<Snapshot>>,
    tx: mpsc::Sender<Action>,
    generation: Arc<AtomicU64>,
    actor: Mutex<Option<thread::JoinHandle<()>>>,
}

struct Capture {
    samples: Vec<f32>,
    state: Arc<Mutex<Snapshot>>,
    tx: mpsc::Sender<Action>,
    limit_sent: bool,
    id: u64,
}

impl Sink for Capture {
    fn process_chunk(&mut self, chunk: AudioChunk<'_>) {
        let count = chunk.valid_frames.min(MAX_SAMPLES - self.samples.len());
        let audio = &chunk.samples[..count];
        self.samples.extend_from_slice(audio);
        if !audio.is_empty() {
            let rms = (audio.iter().map(|v| v * v).sum::<f32>() / audio.len() as f32).sqrt();
            let mut state = self.state.lock().unwrap();
            if state.phase == "starting" {
                state.phase("recording", None);
                state.started_at = Some(
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_millis() as u64,
                );
            }
            state.level = state.level * 0.65 + (rms * 10.0).min(1.0) * 0.35;
        }
        if self.samples.len() == MAX_SAMPLES && !self.limit_sent {
            self.limit_sent = true;
            let _ = self.tx.send(Action::Finish(Some(self.id)));
        }
    }
}

impl Sessions {
    pub fn new(mut model: PathBuf, config: PathBuf, history: Arc<crate::history::History>) -> Self {
        let state = Arc::new(Mutex::new(Snapshot {
            phase: "idle".into(),
            started_at: None,
            last_result: None,
            message: if model.is_file() {
                None
            } else {
                Some("Choose a speech model in Settings.".into())
            },
            level: 0.0,
            model_path: model.to_string_lossy().into_owned(),
            model_loaded: false,
            shortcut_command: format!(
                "'{}' --toggle",
                // AppImage mounts are temporary; desktop bindings must launch the image itself.
                if cfg!(target_os = "linux") {
                    std::env::var_os("APPIMAGE")
                        .map(PathBuf::from)
                        .unwrap_or_else(|| std::env::current_exe().unwrap_or_default())
                } else {
                    std::env::current_exe().unwrap_or_default()
                }
                .to_string_lossy()
                .replace('\'', "'\\''")
            ),
        }));
        let generation = Arc::new(AtomicU64::new(0));
        let (tx, rx) = mpsc::channel();
        let actor = {
            let state = state.clone();
            let generation = generation.clone();
            let tx = tx.clone();
            thread::spawn(move || {
                let mut recorder: Option<Recorder<Capture>> = None;
                let mut workers: Vec<thread::JoinHandle<()>> = Vec::new();
                let mut warm = Arc::new(Mutex::new(None));
                let mut delivery: Option<bool> = None;
                loop {
                    let action = match rx.recv_timeout(Duration::from_secs(60)) {
                        Ok(action) => action,
                        Err(mpsc::RecvTimeoutError::Timeout) => {
                            history.prune();
                            continue;
                        }
                        Err(_) => break,
                    };
                    // Reap completed workers without retaining per-session resources.
                    let mut index = 0;
                    while index < workers.len() {
                        if workers[index].is_finished() {
                            let _ = workers.swap_remove(index).join();
                        } else {
                            index += 1;
                        }
                    }
                    let action = match action {
                        Action::Toggle(terminal) => {
                            let phase = state.lock().unwrap().phase.clone();
                            match phase.as_str() {
                                "recording" => Action::Finish(None),
                                "starting" | "processing" => Action::Cancel,
                                _ => {
                                    delivery = Some(terminal);
                                    Action::Start
                                }
                            }
                        }
                        Action::Start => {
                            delivery = None;
                            Action::Start
                        }
                        other => other,
                    };
                    match action {
                        Action::Start => {
                            if matches!(
                                state.lock().unwrap().phase.as_str(),
                                "starting" | "recording" | "processing"
                            ) {
                                continue;
                            }
                            if !model.is_file() {
                                state.lock().unwrap().phase(
                                    "error",
                                    Some(
                                        "Model not found. Choose a speech model in Settings."
                                            .into(),
                                    ),
                                );
                                continue;
                            }
                            let id = generation.fetch_add(1, Ordering::SeqCst) + 1;
                            state.lock().unwrap().phase("starting", None);
                            let errors = tx.clone();
                            let opened = Recorder::open_with_failure_handler(
                                RecorderConfig::speech(),
                                move |e| {
                                    let _ = errors.send(Action::Failed(id, e.to_string()));
                                },
                            );
                            match opened {
                                Ok(mic) => {
                                    let sink = Capture {
                                        samples: Vec::with_capacity(MAX_SAMPLES),
                                        state: state.clone(),
                                        tx: tx.clone(),
                                        limit_sent: false,
                                        id,
                                    };
                                    match mic.start(sink) {
                                        Ok(()) => recorder = Some(mic),
                                        Err(e) => state
                                            .lock()
                                            .unwrap()
                                            .phase("error", Some(e.to_string())),
                                    }
                                }
                                Err(e) => state.lock().unwrap().phase("error", Some(e.to_string())),
                            }
                        }
                        Action::Finish(id) => {
                            if id.is_some_and(|id| id != generation.load(Ordering::SeqCst)) {
                                continue;
                            }
                            let Some(mic) = recorder.take() else { continue };
                            let stopped = mic.stop();
                            drop(mic);
                            let samples = match stopped {
                                Ok(stopped) if stopped.is_complete() => stopped.sink.samples,
                                Ok(_) => {
                                    state.lock().unwrap().phase(
                                        "error",
                                        Some(
                                            "Microphone interrupted or audio lost. Try again."
                                                .into(),
                                        ),
                                    );
                                    continue;
                                }
                                Err(e) => {
                                    state.lock().unwrap().phase("error", Some(e.to_string()));
                                    continue;
                                }
                            };
                            if samples.len() < 1600 || samples.iter().all(|v| v.abs() < 0.0001) {
                                state
                                    .lock()
                                    .unwrap()
                                    .phase("idle", Some("No speech detected.".into()));
                                continue;
                            }
                            state.lock().unwrap().phase("processing", None);
                            let duration = samples.len() as u64 * 1000 / 16_000;
                            let id = generation.load(Ordering::SeqCst);
                            let generation = generation.clone();
                            let tx = tx.clone();
                            let model = model.clone();
                            let warm = warm.clone();
                            workers.push(thread::spawn(move || {
                                let result = crate::inference::infer(
                                    &mut warm.lock().unwrap(),
                                    &model,
                                    samples,
                                    &generation,
                                    id,
                                );
                                let _ = tx.send(Action::Complete(id, duration, result));
                            }));
                        }
                        Action::Cancel => {
                            generation.fetch_add(1, Ordering::SeqCst);
                            drop(recorder.take());
                            if state.lock().unwrap().phase == "processing" {
                                state.lock().unwrap().model_loaded = false;
                            }
                            state
                                .lock()
                                .unwrap()
                                .phase("idle", Some("Cancelled.".into()));
                        }
                        Action::Clear => state.lock().unwrap().last_result = None,
                        Action::Toggle(_) => unreachable!(),
                        Action::SelectModel(path, reply) => {
                            let busy = matches!(
                                state.lock().unwrap().phase.as_str(),
                                "starting" | "recording" | "processing"
                            );
                            let result = if busy {
                                Err("Finish or cancel dictation before changing models.".into())
                            } else {
                                crate::model::save(&config, &path)
                            };
                            if result.is_ok() {
                                generation.fetch_add(1, Ordering::SeqCst);
                                // A cancelled inference may still be unwinding. Do not block the actor.
                                let old = std::mem::replace(&mut warm, Arc::new(Mutex::new(None)));
                                workers.push(thread::spawn(move || {
                                    *old.lock().unwrap() = None;
                                }));
                                model = path;
                                let mut state = state.lock().unwrap();
                                state.model_path = model.to_string_lossy().into_owned();
                                state.model_loaded = false;
                                state.phase(
                                    "idle",
                                    Some(
                                        "Model selected. It will load on your next dictation."
                                            .into(),
                                    ),
                                );
                            }
                            let _ = reply.send(result);
                        }
                        Action::Failed(id, error) => {
                            if id == generation.load(Ordering::SeqCst) {
                                generation.fetch_add(1, Ordering::SeqCst);
                                drop(recorder.take());
                                state.lock().unwrap().phase("error", Some(error));
                            }
                        }
                        Action::Complete(id, duration, result) => {
                            if id != generation.load(Ordering::SeqCst) {
                                continue;
                            }
                            let saved = match &result {
                                Ok(text) => history.record(text.clone(), duration),
                                Err(_) => Ok(()),
                            };
                            let mut state = state.lock().unwrap();
                            state.model_loaded = result.is_ok();
                            match result {
                                Ok(text) if text.trim().is_empty() => {
                                    state.phase("idle", Some("No speech detected.".into()))
                                }
                                Ok(text) => {
                                    state.last_result = Some(Transcript {
                                        text: text.clone(),
                                        duration_ms: duration,
                                    });
                                    state.phase("complete", None);
                                    if let Err(error) = saved {
                                        state.message = Some(format!("Transcript ready, but history could not be saved: {error}"));
                                    }
                                    if let Some(terminal) = delivery {
                                        let tx = tx.clone();
                                        let generation = generation.clone();
                                        workers.push(thread::spawn(move || {
                                            let result = if generation.load(Ordering::SeqCst) != id
                                            {
                                                Err("Cancelled".into())
                                            } else {
                                                crate::output::copy(&text).and_then(|_| {
                                                    crate::output::paste(terminal, &generation, id)
                                                })
                                            };
                                            let _ = tx.send(Action::Delivered(id, result));
                                        }));
                                    }
                                }
                                Err(error) => state.phase("error", Some(error)),
                            }
                        }
                        Action::Delivered(id, result) => {
                            if id == generation.load(Ordering::SeqCst) {
                                let message = match result {
                                    Ok(()) => "Paste shortcut sent to the focused app.".into(),
                                    Err(error) => format!("Automatic paste unavailable: {error}. Use Copy, then paste manually."),
                                };
                                let mut state = state.lock().unwrap();
                                state.message = Some(match state.message.take() {
                                    Some(existing) => format!("{existing} {message}"),
                                    None => message,
                                });
                            }
                        }
                        Action::Shutdown => {
                            generation.fetch_add(1, Ordering::SeqCst);
                            drop(recorder.take());
                            for worker in workers {
                                let _ = worker.join();
                            }
                            *warm.lock().unwrap() = None;
                            break;
                        }
                    }
                }
            })
        };
        Self {
            state,
            tx,
            generation,
            actor: Mutex::new(Some(actor)),
        }
    }

    pub fn toggle(&self, terminal: bool) {
        let _ = self.tx.send(Action::Toggle(terminal));
    }
    pub fn model_available(&self) -> bool {
        std::path::Path::new(&self.state.lock().unwrap().model_path).is_file()
    }
    pub fn cancel(&self) {
        let _ = self.tx.send(Action::Cancel);
    }
    pub fn last_text(&self) -> Option<String> {
        self.state
            .lock()
            .unwrap()
            .last_result
            .as_ref()
            .map(|t| t.text.clone())
    }
    pub fn select_model(&self, path: PathBuf) -> Result<(), String> {
        let (tx, rx) = mpsc::sync_channel(1);
        self.tx
            .send(Action::SelectModel(path, tx))
            .map_err(|e| e.to_string())?;
        rx.recv().map_err(|e| e.to_string())?
    }

    pub fn shutdown(&self) {
        self.generation.fetch_add(1, Ordering::SeqCst);
        let _ = self.tx.send(Action::Shutdown);
        if let Some(actor) = self.actor.lock().unwrap().take() {
            let _ = actor.join();
        }
    }
}

#[tauri::command]
pub fn session_snapshot(sessions: tauri::State<'_, Sessions>) -> Snapshot {
    sessions.state.lock().unwrap().clone()
}

#[tauri::command]
pub fn session_action(action: String, sessions: tauri::State<'_, Sessions>) -> Result<(), String> {
    let action = match action.as_str() {
        "start" => Action::Start,
        "finish" => Action::Finish(None),
        "cancel" => Action::Cancel,
        "clear" => Action::Clear,
        _ => return Err("Unknown session action".into()),
    };
    sessions.tx.send(action).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "opens the default microphone briefly; run explicitly on the target desktop"]
    fn default_microphone_smoke() {
        let mic = Recorder::open(RecorderConfig::speech()).expect("open default input");
        mic.start(handy_recorder::CollectingSink::new()).unwrap();
        thread::sleep(Duration::from_millis(500));
        let stopped = mic.stop().unwrap();
        assert!(stopped.is_complete(), "{:?}", stopped.end_reason);
        let samples = stopped.sink.into_samples();
        assert!(!samples.is_empty());
        assert!(samples.iter().all(|sample| sample.is_finite()));
        mic.close().unwrap();
    }

    #[test]
    fn cancelled_worker_cannot_replace_last_result() {
        let sessions = Sessions::new(
            PathBuf::from("missing-model"),
            PathBuf::new(),
            Arc::new(crate::history::History::memory()),
        );
        sessions.state.lock().unwrap().last_result = Some(Transcript {
            text: "Keep this".into(),
            duration_ms: 1000,
        });
        sessions.tx.send(Action::Cancel).unwrap();
        sessions
            .tx
            .send(Action::Complete(0, 1000, Ok("Late result".into())))
            .unwrap();
        // Shutdown is an ordered barrier after all queued actions.
        sessions.tx.send(Action::Shutdown).unwrap();
        sessions
            .actor
            .lock()
            .unwrap()
            .take()
            .unwrap()
            .join()
            .unwrap();
        assert_eq!(
            sessions
                .state
                .lock()
                .unwrap()
                .last_result
                .as_ref()
                .unwrap()
                .text,
            "Keep this"
        );
    }

    #[test]
    fn capture_is_bounded_and_ignores_padding() {
        let sessions = Sessions::new(
            PathBuf::from("missing-model"),
            PathBuf::new(),
            Arc::new(crate::history::History::memory()),
        );
        let (tx, rx) = mpsc::channel();
        let mut sink = Capture {
            samples: vec![0.0; MAX_SAMPLES - 2],
            state: sessions.state.clone(),
            tx,
            limit_sent: false,
            id: 7,
        };
        sink.process_chunk(AudioChunk {
            samples: &[0.25, 0.5, 0.75, 1.0],
            sample_rate: 16_000,
            channels: 1,
            valid_frames: 2,
        });
        sink.process_chunk(AudioChunk {
            samples: &[1.0; 4],
            sample_rate: 16_000,
            channels: 1,
            valid_frames: 4,
        });
        assert_eq!(sink.samples.len(), MAX_SAMPLES);
        assert_eq!(&sink.samples[MAX_SAMPLES - 2..], &[0.25, 0.5]);
        assert!(matches!(rx.try_recv(), Ok(Action::Finish(Some(7)))));
        assert!(rx.try_recv().is_err());
        sessions.shutdown();
    }
}
