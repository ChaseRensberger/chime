//! Passive recording feedback. Renderers observe the actor; they never drive capture.
use serde::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    sync::Mutex,
    time::{Duration, Instant},
};
use tauri::Manager;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Position {
    Top,
    #[default]
    Bottom,
    Hidden,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    position: Position,
    backend: Option<String>,
    limitation: Option<String>,
}

pub struct Overlay {
    status: Mutex<Status>,
    path: PathBuf,
    preview_until: Mutex<Option<Instant>>,
}

#[derive(Clone, Debug, Default)]
pub struct Input {
    pub phase: String,
    pub revision: u64,
    pub level: f32,
    pub paste_failed: bool,
    pub delivery_pending: bool,
    pub message: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Starting,
    Listening,
    Processing,
    Success,
    Notice,
    Error,
}

#[derive(Clone, Debug)]
pub struct Frame {
    pub label: &'static str,
    pub heights: [f64; 5],
}

impl Frame {
    fn new(kind: Kind, label: &'static str, level: f32, seconds: f64) -> Self {
        let strength = if level.is_finite() {
            (level.clamp(0.0, 1.0) as f64).sqrt()
        } else {
            0.0
        };
        let heights = std::array::from_fn(|i| {
            let base = [6.0, 13.0, 21.0, 13.0, 6.0][i];
            match kind {
                Kind::Listening => {
                    3.0 + strength
                        * (base + 6.0)
                        * (0.35
                            + 0.65
                                * (seconds * (3.8 + i as f64 * 0.16) + i as f64 * 1.3)
                                    .sin()
                                    .abs())
                }
                Kind::Processing => 4.0 + 8.0 * (seconds * 3.3 - i as f64 * 0.7).cos().max(0.0),
                _ => base,
            }
        });
        Self { label, heights }
    }
}

/// Transient results have a deadline, not a delayed hide callback that can hide a newer recording.
#[derive(Default)]
pub struct Presentation {
    previous: Option<(u64, String, bool, bool, Option<String>)>,
    notice_until: Option<Instant>,
    since: Option<Instant>,
}

impl Presentation {
    pub fn update(&mut self, input: &Input, now: Instant) -> Option<Frame> {
        let key = (
            input.revision,
            input.phase.clone(),
            input.paste_failed,
            input.delivery_pending,
            input.message.clone(),
        );
        if self.previous.as_ref() != Some(&key) {
            self.since = Some(now);
            // Do not display a leftover result on launch.
            self.notice_until = self.previous.as_ref().and_then(|_| {
                matches!(input.phase.as_str(), "idle" | "complete" | "error").then_some(
                    now + Duration::from_secs(if input.paste_failed || input.phase == "error" {
                        5
                    } else {
                        2
                    }),
                )
            });
            self.previous = Some(key);
        }
        let (kind, label) = match input.phase.as_str() {
            "starting" => (Kind::Starting, "Starting microphone"),
            "recording" => (Kind::Listening, "Listening"),
            "processing" => (Kind::Processing, "Transcribing"),
            "complete" if input.delivery_pending => (Kind::Processing, "Pasting"),
            _ if !self.notice_until.is_some_and(|until| now < until) => return None,
            _ if input.paste_failed => (Kind::Error, "Paste failed · open Chime"),
            "error" => (Kind::Error, "Open Chime for details"),
            "complete" => (Kind::Success, "Transcript ready"),
            "idle" if input.message.as_deref() == Some("No speech detected.") => {
                (Kind::Notice, "No speech detected")
            }
            _ => return None,
        };
        Some(Frame::new(
            kind,
            label,
            input.level,
            now.saturating_duration_since(self.since.unwrap_or(now))
                .as_secs_f64(),
        ))
    }
}

pub fn setup(app: &tauri::AppHandle) -> tauri::Result<()> {
    let path = app.path().app_config_dir()?.join("overlay.json");
    let position = std::fs::read(&path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default();
    app.manage(Overlay {
        status: Mutex::new(Status {
            position,
            backend: None,
            limitation: None,
        }),
        path,
        preview_until: Mutex::new(None),
    });
    #[cfg(target_os = "linux")]
    let result = linux::install(app);
    #[cfg(target_os = "macos")]
    let result = macos::install(app);
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    let result: Result<&str, String> =
        Err("Recording overlay is not available on this platform yet.".into());
    let overlay = app.state::<Overlay>();
    let mut status = overlay.status.lock().unwrap();
    match result {
        Ok(backend) => status.backend = Some(backend.into()),
        Err(error) => {
            eprintln!("Recording overlay unavailable: {error}");
            status.limitation = Some(error);
        }
    }
    Ok(())
}

fn current_frame(
    app: &tauri::AppHandle,
    presentation: &mut Presentation,
) -> (Position, Option<Frame>) {
    let now = Instant::now();
    let overlay = app.state::<Overlay>();
    let position = overlay.status.lock().unwrap().position;
    let input = app.state::<crate::session::Sessions>().overlay_input();
    let changed = presentation
        .previous
        .as_ref()
        .is_some_and(|previous| previous.0 != input.revision);
    let mut frame = presentation.update(&input, now);
    // A real recording or pending paste always takes precedence over Settings' preview.
    let mut preview = overlay.preview_until.lock().unwrap();
    if changed
        || position == Position::Hidden
        || input.delivery_pending
        || matches!(
            input.phase.as_str(),
            "starting" | "recording" | "processing"
        )
    {
        *preview = None;
    }
    if let Some(until) = preview.filter(|until| now < *until) {
        let seconds = 3.0 - (until - now).as_secs_f64();
        let level = (0.18 + 0.55 * (seconds * 3.5).sin().abs()) as f32;
        frame = Some(Frame::new(
            Kind::Listening,
            "Listening · preview",
            level,
            seconds,
        ));
    }
    (position, frame)
}

#[tauri::command]
pub fn overlay_status(overlay: tauri::State<'_, Overlay>) -> Status {
    overlay.status.lock().unwrap().clone()
}

#[tauri::command]
pub fn overlay_save(overlay: tauri::State<'_, Overlay>, position: Position) -> Result<(), String> {
    let mut status = overlay.status.lock().unwrap();
    std::fs::create_dir_all(overlay.path.parent().ok_or("Invalid settings path")?)
        .map_err(|e| e.to_string())?;
    let temporary = overlay.path.with_extension("tmp");
    std::fs::write(
        &temporary,
        serde_json::to_vec(&position).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    std::fs::rename(temporary, &overlay.path).map_err(|e| e.to_string())?;
    status.position = position;
    Ok(())
}

#[tauri::command]
pub fn overlay_preview(overlay: tauri::State<'_, Overlay>) -> Result<(), String> {
    let status = overlay.status.lock().unwrap();
    if let Some(error) = &status.limitation {
        return Err(error.clone());
    }
    if status.position == Position::Hidden {
        return Err("Choose an overlay position first.".into());
    }
    *overlay.preview_until.lock().unwrap() = Some(Instant::now() + Duration::from_secs(3));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn waveform_moves_on_sustained_voice_but_rests_in_silence() {
        let heights = |level, t| Frame::new(Kind::Listening, "Listening", level, t).heights;
        assert_ne!(heights(0.4, 0.0), heights(0.4, 0.1));
        assert_eq!(heights(0.0, 0.0), heights(0.0, 0.1));
        assert_eq!(heights(f32::NAN, 1.0), [3.0; 5]);
    }
    #[test]
    fn a_new_recording_replaces_results_and_their_deadlines() {
        let now = Instant::now();
        let mut view = Presentation::default();
        let mut input = Input {
            phase: "idle".into(),
            ..Input::default()
        };
        assert!(view.update(&input, now).is_none());
        input.phase = "recording".into();
        assert_eq!(view.update(&input, now).unwrap().label, "Listening");
        input.phase = "complete".into();
        input.delivery_pending = true;
        assert_eq!(view.update(&input, now).unwrap().label, "Pasting");
        input.delivery_pending = false;
        input.paste_failed = true;
        assert_eq!(
            view.update(&input, now).unwrap().label,
            "Paste failed · open Chime"
        );
        input.phase = "recording".into();
        input.paste_failed = false;
        assert_eq!(
            view.update(&input, now + Duration::from_secs(6))
                .unwrap()
                .label,
            "Listening"
        );
        input.phase = "idle".into();
        input.message = Some("Cancelled.".into());
        assert!(view.update(&input, now).is_none());
    }

    #[test]
    fn errors_expire_and_old_results_do_not_reappear() {
        let now = Instant::now();
        let mut view = Presentation::default();
        let mut input = Input {
            phase: "complete".into(),
            ..Input::default()
        };
        assert!(view.update(&input, now).is_none());
        input.phase = "starting".into();
        view.update(&input, now);
        input.phase = "error".into();
        assert!(view.update(&input, now).is_some());
        assert!(view.update(&input, now + Duration::from_secs(5)).is_none());
        assert!(view.update(&input, now + Duration::from_secs(10)).is_none());
        // A second microphone failure can happen between two display ticks.
        input.revision += 2;
        assert!(view.update(&input, now + Duration::from_secs(11)).is_some());
    }
}
