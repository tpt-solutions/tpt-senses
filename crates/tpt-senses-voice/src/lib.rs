//! Adapter from `tpt-voice` output to `tpt-senses` auditory observations.
//!
//! Speech recognition is an optional component of hearing. This adapter represents what the
//! speech stack reports (words and speaker turns) as auditory observations. It does not define
//! hearing, and it does not produce observations for non-speech sounds: those come from the audio
//! adapter.
//!
//! `tpt-voice` is not published to crates.io, so this crate cannot depend on it. The input types
//! are **mirrors** of the source structs, each with its source named below.
//!
//! Source commit these mirrors were read from: `tpt-voice` `acd3307e558f9f7282dbb8f71375966af743eb5d`.
//!
//! * [`WordMirror`]: `tpt-av-voice-utils/src/transcript.rs`, `WordTimestamp`.
//! * [`SpeakerMirror`]: `tpt-av-voice-diarize/src/diarizer.rs`, `SpeakerSegment`. The display name
//!   is deliberately not mirrored: it is personal data, and the speaker identifier is enough.

use tpt_senses::prelude::*;
use tpt_senses::{Adapter, AdapterError, Algorithm, Duration};

/// Mirror of `WordTimestamp`.
#[derive(Clone, Debug, PartialEq)]
pub struct WordMirror {
    /// The recognised word.
    pub word: String,
    /// Start time, seconds from the start of the audio.
    pub start_time: f64,
    /// End time, seconds from the start of the audio.
    pub end_time: f64,
    /// Recogniser confidence, 0 to 1.
    pub confidence: f32,
}

/// Mirror of `SpeakerSegment`, without the display name.
#[derive(Clone, Debug, PartialEq)]
pub struct SpeakerMirror {
    /// Speaker identifier, such as `SPEAKER_00`.
    pub speaker_id: String,
    /// Start time, seconds from the start of the audio.
    pub start_time: f64,
    /// End time, seconds from the start of the audio.
    pub end_time: f64,
    /// Diarization confidence, 0 to 1.
    pub confidence: f32,
}

/// One batch of speech output. Times are relative to `base`, the time the audio started.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum VoiceInput {
    /// A transcript with word timings.
    Words {
        /// When the audio started.
        base: Timestamp,
        /// The words, in order.
        words: Vec<WordMirror>,
    },
    /// Speaker turns from diarization.
    Speakers {
        /// When the audio started.
        base: Timestamp,
        /// The turns.
        segments: Vec<SpeakerMirror>,
    },
}

/// Identity for a [`VoiceAdapter`].
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct VoiceConfig {
    /// The microphone or audio source the speech came from.
    pub sensor: SensorId,
}

/// Converts speech output into auditory observations.
#[derive(Clone, Debug)]
pub struct VoiceAdapter {
    config: VoiceConfig,
    next_id: u64,
}

impl VoiceAdapter {
    /// Creates an adapter whose observation ids start at `first_id`.
    #[must_use]
    pub const fn new(config: VoiceConfig, first_id: ObservationId) -> Self {
        Self {
            config,
            next_id: first_id.get(),
        }
    }

    fn id(&mut self) -> ObservationId {
        let id = ObservationId::new(self.next_id);
        self.next_id += 1;
        id
    }

    fn provenance(&self) -> Provenance {
        Provenance::new(Origin::Real)
            .with_step(ProvenanceStep::Sensor(self.config.sensor))
            .with_step(ProvenanceStep::Algorithm(Algorithm::new(
                "tpt-av-voice::asr-and-diarization",
                "acd3307",
            )))
    }
}

/// Converts seconds from `base` into an absolute timestamp, rejecting values that cannot be
/// represented.
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the range is checked before the cast, so truncation cannot wrap"
)]
fn at(base: Timestamp, seconds: f64) -> Result<Timestamp, AdapterError> {
    if !seconds.is_finite() || seconds < 0.0 {
        return Err(AdapterError::Invalid(
            "time offset must be finite and non-negative".into(),
        ));
    }
    let nanos = seconds * 1_000_000_000.0;
    if nanos >= u64::MAX as f64 {
        return Err(AdapterError::Invalid("time offset is out of range".into()));
    }
    Ok(base.saturating_add(Duration::from_nanos(nanos.round() as u64)))
}

fn confidence(value: f32) -> Result<Confidence, AdapterError> {
    Confidence::new(f64::from(value))
        .ok_or_else(|| AdapterError::Invalid("confidence must be finite and within [0, 1]".into()))
}

fn ordered(start: f64, end: f64) -> Result<(), AdapterError> {
    if end >= start {
        Ok(())
    } else {
        Err(AdapterError::Invalid(
            "segment ends before it starts".into(),
        ))
    }
}

impl Adapter for VoiceAdapter {
    type Input = VoiceInput;

    fn name(&self) -> &'static str {
        "tpt-voice-adapter"
    }

    fn adapt(&mut self, input: VoiceInput) -> Result<Vec<Observation>, AdapterError> {
        let provenance = self.provenance();
        match input {
            VoiceInput::Words { base, words } => {
                let mut out = Vec::with_capacity(words.len());
                for w in words {
                    ordered(w.start_time, w.end_time)?;
                    let when = at(base, w.start_time)?;
                    let c = confidence(w.confidence)?;
                    let id = self.id();
                    out.push(Observation::new(
                        id,
                        Modality::Auditory,
                        Property::new("word"),
                        ObservationValue::Categorical(w.word),
                        c,
                        when,
                        provenance.clone(),
                    ));
                }
                Ok(out)
            }
            VoiceInput::Speakers { base, segments } => {
                let mut out = Vec::with_capacity(segments.len());
                for s in segments {
                    ordered(s.start_time, s.end_time)?;
                    let when = at(base, s.start_time)?;
                    let c = confidence(s.confidence)?;
                    let id = self.id();
                    out.push(Observation::new(
                        id,
                        Modality::Auditory,
                        Property::new("speaker_turn"),
                        ObservationValue::Categorical(s.speaker_id),
                        c,
                        when,
                        provenance.clone(),
                    ));
                }
                Ok(out)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> Timestamp {
        Timestamp::new(1_000_000_000, ClockSource::Hardware)
    }

    fn adapter() -> VoiceAdapter {
        VoiceAdapter::new(
            VoiceConfig {
                sensor: SensorId::new(4),
            },
            ObservationId::new(1),
        )
    }

    #[test]
    fn words_become_timed_categorical_observations() {
        let words = vec![
            WordMirror {
                word: "pick".into(),
                start_time: 0.5,
                end_time: 0.8,
                confidence: 0.92,
            },
            WordMirror {
                word: "rows".into(),
                start_time: 0.9,
                end_time: 1.3,
                confidence: 0.88,
            },
        ];
        let out = adapter()
            .adapt(VoiceInput::Words {
                base: base(),
                words,
            })
            .unwrap();
        assert_eq!(out.len(), 2);
        assert_eq!(
            out[0].value(),
            &ObservationValue::Categorical("pick".into())
        );
        assert_eq!(out[0].timestamp().nanos(), 1_000_000_000 + 500_000_000);
        assert_eq!(
            out[1].confidence(),
            Confidence::new(f64::from(0.88_f32)).unwrap()
        );
    }

    #[test]
    fn speaker_turns_carry_only_the_identifier() {
        let segments = vec![SpeakerMirror {
            speaker_id: "SPEAKER_00".into(),
            start_time: 2.0,
            end_time: 4.5,
            confidence: 0.7,
        }];
        let out = adapter()
            .adapt(VoiceInput::Speakers {
                base: base(),
                segments,
            })
            .unwrap();
        assert_eq!(out[0].property().as_str(), "speaker_turn");
        assert_eq!(
            out[0].value(),
            &ObservationValue::Categorical("SPEAKER_00".into())
        );
    }

    #[test]
    fn invalid_timing_and_confidence_are_rejected() {
        let backwards = vec![WordMirror {
            word: "x".into(),
            start_time: 2.0,
            end_time: 1.0,
            confidence: 0.5,
        }];
        assert!(matches!(
            adapter().adapt(VoiceInput::Words {
                base: base(),
                words: backwards
            }),
            Err(AdapterError::Invalid(_))
        ));
        let overconfident = vec![WordMirror {
            word: "x".into(),
            start_time: 0.0,
            end_time: 1.0,
            confidence: 1.5,
        }];
        assert!(matches!(
            adapter().adapt(VoiceInput::Words {
                base: base(),
                words: overconfident
            }),
            Err(AdapterError::Invalid(_))
        ));
        let negative = vec![WordMirror {
            word: "x".into(),
            start_time: -1.0,
            end_time: 0.0,
            confidence: 0.5,
        }];
        assert!(matches!(
            adapter().adapt(VoiceInput::Words {
                base: base(),
                words: negative
            }),
            Err(AdapterError::Invalid(_))
        ));
    }
}
