//! Sensor capabilities: what a sensor can measure, described without biological assumptions
//! (spec §10, §5).

use alloc::vec::Vec;

use crate::{Duration, Frequency, Modality};

/// A closed numeric range `[min, max]`.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Range {
    min: f64,
    max: f64,
}

impl Range {
    /// Creates a range. Returns `None` unless both ends are finite and `min <= max`.
    #[must_use]
    pub fn new(min: f64, max: f64) -> Option<Self> {
        (min.is_finite() && max.is_finite() && min <= max).then_some(Self { min, max })
    }

    /// Lower end of the range.
    #[must_use]
    pub const fn min(self) -> f64 {
        self.min
    }

    /// Upper end of the range.
    #[must_use]
    pub const fn max(self) -> f64 {
        self.max
    }

    /// Returns `true` if `value` lies within the range (inclusive).
    #[must_use]
    pub fn contains(self, value: f64) -> bool {
        value >= self.min && value <= self.max
    }
}

/// Spectral range in nanometres. Same shape as [`Range`].
pub type SpectralRange = Range;

/// The smallest distinguishable step a sensor can report, in its measurement unit.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Resolution {
    step: f64,
}

impl Resolution {
    /// Creates a resolution. Returns `None` unless `step` is finite and strictly positive.
    #[must_use]
    pub fn new(step: f64) -> Option<Self> {
        (step.is_finite() && step > 0.0).then_some(Self { step })
    }

    /// The step size in the measurement's unit.
    #[must_use]
    pub const fn step(self) -> f64 {
        self.step
    }
}

/// Descriptive capabilities of a sensor. Every field is optional except the modality list.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct SensorCapabilities {
    /// Modalities this sensor produces. A sensor may produce several.
    pub modalities: Vec<Modality>,
    /// Maximum or nominal sampling rate.
    pub sampling_rate: Option<Frequency>,
    /// Smallest spatial step the sensor resolves.
    pub spatial_resolution: Option<Resolution>,
    /// Smallest temporal step the sensor resolves.
    pub temporal_resolution: Option<Duration>,
    /// Range of values the sensor can report.
    pub measurement_range: Option<Range>,
    /// Spectral range, for optical sensors.
    pub spectral_range: Option<SpectralRange>,
}

impl SensorCapabilities {
    /// Creates capabilities for the given modalities with all other fields unset.
    #[must_use]
    pub fn new(modalities: Vec<Modality>) -> Self {
        Self {
            modalities,
            ..Self::default()
        }
    }

    /// Returns `true` if the sensor produces `modality`.
    #[must_use]
    pub fn supports(&self, modality: &Modality) -> bool {
        self.modalities.contains(modality)
    }
}

/// Capabilities specific to imaging sensors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct VisionCapability {
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
    /// Channels per pixel.
    pub channels: u8,
}

impl VisionCapability {
    /// Frame dimensions and channel count of an imaging sensor.
    #[must_use]
    pub const fn new(width: u32, height: u32, channels: u8) -> Self {
        Self {
            width,
            height,
            channels,
        }
    }
}

/// Capabilities specific to audio sensors.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct AuditoryCapability {
    /// Sample rate.
    pub sample_rate: Frequency,
    /// Number of channels.
    pub channels: u16,
    /// Audible (or sensed) frequency range in hertz.
    pub frequency_range: Range,
}

impl AuditoryCapability {
    /// Sample rate, channel count and frequency range of an audio sensor.
    #[must_use]
    pub const fn new(sample_rate: Frequency, channels: u16, frequency_range: Range) -> Self {
        Self {
            sample_rate,
            channels,
            frequency_range,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn range_validation_and_containment() {
        let r = Range::new(-1.0, 1.0).unwrap();
        assert!(r.contains(0.0));
        assert!(!r.contains(1.5));
        assert!(Range::new(1.0, -1.0).is_none());
        assert!(Range::new(0.0, f64::NAN).is_none());
    }

    #[test]
    fn resolution_must_be_positive() {
        assert!(Resolution::new(0.001).is_some());
        assert!(Resolution::new(0.0).is_none());
    }

    #[test]
    fn capabilities_report_modalities() {
        let caps = SensorCapabilities::new(alloc::vec![Modality::Thermal, Modality::Infrared]);
        assert!(caps.supports(&Modality::Infrared));
        assert!(!caps.supports(&Modality::Lidar));
        assert!(caps.sampling_rate.is_none());
    }
}
