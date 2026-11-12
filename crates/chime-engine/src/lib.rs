//! Host-independent local transcription. Hosts own capture and worker lifecycle.
//! Hosts own microphone capture, resampling, shortcuts, persistence, and delivery.

use std::{borrow::Cow, fmt};

/// Borrowed mono PCM samples at 16 kHz. Finite peaks outside [-1.0, 1.0]
/// are clamped before inference; non-finite samples are rejected.
pub struct Audio<'a> {
    pub samples: &'a [f32],
}

pub struct Transcript {
    pub text: String,
}

#[derive(Debug)]
pub enum TranscriptionError {
    Unavailable(String),
    InvalidAudio(String),
    Interrupted,
}

impl fmt::Display for TranscriptionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable(reason) => write!(f, "Speech engine unavailable: {reason}"),
            Self::InvalidAudio(reason) => write!(f, "Invalid audio: {reason}"),
            Self::Interrupted => f.write_str("Transcription interrupted"),
        }
    }
}

impl std::error::Error for TranscriptionError {}

/// Implementations run on a host-owned worker, never the interface thread.
/// An empty transcript represents a clip with no recognized speech.
pub trait Transcriber: Send {
    fn transcribe(&mut self, audio: Audio<'_>) -> Result<Transcript, TranscriptionError>;
}

pub struct LocalTranscriber {
    session: transcribe_cpp::Session,
}

impl LocalTranscriber {
    pub fn load(path: &std::path::Path) -> Result<Self, TranscriptionError> {
        transcribe_cpp::disable_logging();
        let session = transcribe_cpp::Model::load(path)
            .and_then(|model| model.session())
            .map_err(|e| TranscriptionError::Unavailable(e.to_string()))?;
        Ok(Self { session })
    }
}

fn prepare_samples(samples: &[f32]) -> Result<Cow<'_, [f32]>, TranscriptionError> {
    if samples.is_empty() {
        return Err(TranscriptionError::InvalidAudio(
            "Recording contains no audio samples".into(),
        ));
    }
    if samples.len() > 16_000 * 60 {
        return Err(TranscriptionError::InvalidAudio(
            "Recording exceeds the 60-second limit at 16 kHz".into(),
        ));
    }
    let mut needs_clamping = false;
    for sample in samples {
        if !sample.is_finite() {
            return Err(TranscriptionError::InvalidAudio(
                "Recording contains non-finite samples (NaN or infinity)".into(),
            ));
        }
        needs_clamping |= sample.abs() > 1.0;
    }
    if needs_clamping {
        Ok(Cow::Owned(
            samples
                .iter()
                .map(|sample| sample.clamp(-1.0, 1.0))
                .collect(),
        ))
    } else {
        Ok(Cow::Borrowed(samples))
    }
}

impl Transcriber for LocalTranscriber {
    fn transcribe(&mut self, audio: Audio<'_>) -> Result<Transcript, TranscriptionError> {
        let samples = prepare_samples(audio.samples)?;
        let result = self
            .session
            .run(&samples, &transcribe_cpp::RunOptions::default())
            .map_err(|e| TranscriptionError::Unavailable(e.to_string()))?;
        Ok(Transcript {
            text: result.text.trim().to_owned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clips_finite_peaks_without_changing_input_or_quiet_samples() {
        let samples = [-1.1, -1.0, -0.25, 0.0, 0.25, 1.0, 1.01, f32::MAX];
        let prepared = prepare_samples(&samples).unwrap();
        assert_eq!(
            prepared.as_ref(),
            &[-1.0, -1.0, -0.25, 0.0, 0.25, 1.0, 1.0, 1.0]
        );
        assert_eq!(samples[0], -1.1);
        assert_eq!(samples[6], 1.01);
    }

    #[test]
    fn non_finite_samples_are_not_hidden_by_clamping() {
        for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let error = prepare_samples(&[1.1, invalid, -1.1]).unwrap_err();
            assert!(error.to_string().contains("non-finite"));
        }
    }

    #[test]
    fn valid_audio_is_borrowed_and_duration_errors_are_distinct() {
        let samples = vec![0.5; 16_000 * 60];
        assert!(matches!(
            prepare_samples(&samples).unwrap(),
            Cow::Borrowed(_)
        ));
        let empty = prepare_samples(&[]).unwrap_err().to_string();
        let oversized = prepare_samples(&vec![0.0; 16_000 * 60 + 1])
            .unwrap_err()
            .to_string();
        assert!(empty.contains("no audio samples"));
        assert!(oversized.contains("60-second limit"));
        assert_ne!(empty, oversized);
    }
}
