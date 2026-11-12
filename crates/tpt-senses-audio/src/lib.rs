//! Adapter from `tpt-audio` level readings to `tpt-senses` auditory observations.
//!
//! `tpt-audio` is not published to crates.io, so this crate cannot depend on it (a crates.io
//! package may not depend on a git source). [`LevelMirror`] mirrors what the audio engine's
//! `Meter` reports. Each mirror says where it comes from. Update the mirror and its tests together
//! when the source changes.
//!
//! Source commit these mirrors were read from: `tpt-audio` `7268ccbb350b0f5dc77f71906cbe6cc5d59e1af5`.
//!
//! * [`LevelMirror`]: `tpt-av-audio-core/src/dsp/meter.rs`, `Meter::peak()` and `Meter::rms()`.
//!   Both are linear amplitude; the meter does not report decibels.

use tpt_senses::prelude::*;
use tpt_senses::{Adapter, AdapterError, Algorithm};

/// A peak and RMS reading for one channel, in linear amplitude.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LevelMirror {
    /// Channel index the meter belongs to.
    pub channel: u16,
    /// Newest peak level, linear amplitude.
    pub peak: f32,
    /// Newest RMS level, linear amplitude.
    pub rms: f32,
}

/// One level reading, with the time it was taken.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct AudioInput {
    /// When the reading was taken.
    pub timestamp: Timestamp,
    /// The reading.
    pub level: LevelMirror,
}

/// Confidence policy and identity for an [`AudioAdapter`].
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct AudioConfig {
    /// The audio device or engine the meters belong to.
    pub sensor: SensorId,
    /// Confidence for a meter reading.
    pub level_confidence: Confidence,
}

impl AudioConfig {
    /// Defaults: a meter reading gets confidence 0.9.
    #[must_use]
    pub fn new(sensor: SensorId) -> Self {
        Self {
            sensor,
            level_confidence: Confidence::new(0.9).unwrap_or(Confidence::ZERO),
        }
    }
}

/// Converts meter readings into auditory observations.
#[derive(Clone, Debug)]
pub struct AudioAdapter {
    config: AudioConfig,
    next_id: u64,
}

impl AudioAdapter {
    /// Creates an adapter whose observation ids start at `first_id`.
    #[must_use]
    pub const fn new(config: AudioConfig, first_id: ObservationId) -> Self {
        Self {
            config,
            next_id: first_id.get(),
        }
    }
}

impl Adapter for AudioAdapter {
    type Input = AudioInput;

    fn name(&self) -> &'static str {
        "tpt-audio-adapter"
    }

    fn adapt(&mut self, input: AudioInput) -> Result<Vec<Observation>, AdapterError> {
        let LevelMirror { channel, peak, rms } = input.level;
        if !peak.is_finite() || !rms.is_finite() {
            return Err(AdapterError::Invalid("non-finite level".into()));
        }
        if peak < 0.0 || rms < 0.0 {
            return Err(AdapterError::Invalid(
                "levels are amplitudes and cannot be negative".into(),
            ));
        }
        let provenance = Provenance::new(Origin::Real)
            .with_step(ProvenanceStep::Sensor(self.config.sensor))
            .with_step(ProvenanceStep::Algorithm(Algorithm::new(
                "tpt-av-audio-core::meter",
                "7268ccb",
            )));
        let mut out = Vec::with_capacity(2);
        for (name, value) in [("peak_amplitude", peak), ("rms_amplitude", rms)] {
            let id = ObservationId::new(self.next_id);
            self.next_id += 1;
            out.push(Observation::new(
                id,
                Modality::Auditory,
                Property::new(format!("channel_{channel}_{name}")),
                ObservationValue::Scalar(f64::from(value)),
                self.config.level_confidence,
                input.timestamp,
                provenance.clone(),
            ));
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn adapter() -> AudioAdapter {
        AudioAdapter::new(AudioConfig::new(SensorId::new(3)), ObservationId::new(1))
    }

    fn input(peak: f32, rms: f32) -> AudioInput {
        AudioInput {
            timestamp: Timestamp::new(100, ClockSource::Hardware),
            level: LevelMirror {
                channel: 2,
                peak,
                rms,
            },
        }
    }

    #[test]
    fn a_reading_becomes_peak_and_rms_auditory_observations() {
        let out = adapter().adapt(input(0.8, 0.3)).unwrap();
        assert_eq!(out.len(), 2);
        assert!(out.iter().all(|o| o.modality() == &Modality::Auditory));
        assert_eq!(out[0].property().as_str(), "channel_2_peak_amplitude");
        assert_eq!(
            out[0].value(),
            &ObservationValue::Scalar(f64::from(0.8_f32))
        );
        assert_eq!(
            out[1].value(),
            &ObservationValue::Scalar(f64::from(0.3_f32))
        );
    }

    #[test]
    fn silence_is_a_valid_reading() {
        assert!(adapter().adapt(input(0.0, 0.0)).is_ok());
    }

    #[test]
    fn negative_or_non_finite_levels_are_rejected() {
        assert!(matches!(
            adapter().adapt(input(-0.1, 0.0)),
            Err(AdapterError::Invalid(_))
        ));
        assert!(matches!(
            adapter().adapt(input(f32::NAN, 0.0)),
            Err(AdapterError::Invalid(_))
        ));
    }

    #[test]
    fn observation_ids_advance_across_readings() {
        let mut a = adapter();
        let first = a.adapt(input(0.1, 0.1)).unwrap();
        let second = a.adapt(input(0.2, 0.2)).unwrap();
        let ids: Vec<u64> = first.iter().chain(&second).map(|o| o.id().get()).collect();
        assert_eq!(ids, vec![1, 2, 3, 4]);
    }
}
