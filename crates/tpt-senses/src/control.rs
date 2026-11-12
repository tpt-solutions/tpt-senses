//! Active perception and attention (spec §35–§37).
//!
//! These are *types and small pure rules* only. Actual device control stays in the device layer,
//! and deciding what to look at belongs to cognition. This module lets a system say "I am not
//! sure, please sample this differently" in a form every sensor can understand.

use alloc::string::String;
use alloc::vec::Vec;

use crate::{
    Confidence, EntityId, Frequency, ObservationId, Property, SensorCapabilities, SensorHealth,
    SensorId,
};

/// A controllable sensor parameter (spec §37).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ParameterKind {
    /// Optical zoom.
    Zoom,
    /// Focus distance.
    Focus,
    /// Exposure time.
    Exposure,
    /// Sampling frequency.
    SamplingFrequency,
    /// Pointing direction.
    Direction,
    /// Resolution setting.
    Resolution,
    /// Spectral band selection.
    SpectralBand,
    /// Gain.
    Gain,
    /// Measurement range.
    Range,
    /// A parameter not listed above.
    Custom(alloc::borrow::Cow<'static, str>),
}

/// A request to change one parameter of one sensor. The sensor decides whether to comply.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct ParameterChangeRequest {
    /// The sensor to change.
    pub sensor: SensorId,
    /// The parameter to change.
    pub parameter: ParameterKind,
    /// The requested value, in the parameter's natural unit.
    pub value: f64,
    /// Why, in plain words, for logs and audits.
    pub reason: Option<String>,
}

impl ParameterChangeRequest {
    /// A request to set `parameter` on `sensor` to `value`, with no reason recorded.
    #[must_use]
    pub const fn new(sensor: SensorId, parameter: ParameterKind, value: f64) -> Self {
        Self {
            sensor,
            parameter,
            value,
            reason: None,
        }
    }

    /// Records why the change is requested (builder style).
    #[must_use]
    pub fn with_reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }
}

/// What a sensor did with a [`ParameterChangeRequest`].
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum ParameterChangeOutcome {
    /// Applied; the value actually in effect is given.
    Applied {
        /// The value now in effect (it may differ from the request, e.g. after clamping).
        effective: f64,
    },
    /// Refused, with a reason.
    Rejected(String),
    /// Accepted but not yet in effect.
    Pending,
}

/// Returns the request's value clamped to the sensor's advertised range, if one is advertised.
///
/// This is a pre-check only. The sensor remains the authority on what it can do.
#[must_use]
pub fn clamp_to_capabilities(request: &ParameterChangeRequest, caps: &SensorCapabilities) -> f64 {
    match caps.measurement_range {
        Some(range) => request.value.clamp(range.min(), range.max()),
        None => request.value,
    }
}

/// What information is missing, when the system wants to look again (spec §36).
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum InformationNeed {
    /// The observation's confidence is too low to act on.
    LowConfidence,
    /// Sensors disagree about the same property.
    Disagreement,
    /// The data is too old to rely on.
    Stale,
}

/// A request for more information about an uncertain observation (active perception loop).
///
/// Loop: uncertainty → this request → sensor action → new observation → fusion.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct InformationRequest {
    /// The observation that triggered the request.
    pub trigger: ObservationId,
    /// The property that is uncertain.
    pub property: Property,
    /// Why more information is needed.
    pub need: InformationNeed,
    /// Suggested parameter changes that might help. The device layer chooses what to apply.
    pub suggestions: Vec<ParameterChangeRequest>,
}

/// Decides whether an observation confidence warrants an information request.
///
/// Returns `None` when `confidence` is at or above `threshold`.
#[must_use]
pub fn information_request_for(
    trigger: ObservationId,
    property: Property,
    confidence: Confidence,
    threshold: Confidence,
    suggestions: Vec<ParameterChangeRequest>,
) -> Option<InformationRequest> {
    if confidence < threshold {
        Some(InformationRequest {
            trigger,
            property,
            need: InformationNeed::LowConfidence,
            suggestions,
        })
    } else {
        None
    }
}

/// A hint from the cognitive layer about how much attention an entity deserves (spec §35).
///
/// Hints change sensing rates and resolution. They never change what counts as true.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct AttentionHint {
    /// The entity to attend to, or `None` for general attention.
    pub subject: Option<EntityId>,
    /// How much attention, from `0` (none) to `1` (maximum).
    pub weight: Confidence,
}

impl AttentionHint {
    /// A hint with the given subject and weight.
    #[must_use]
    pub const fn new(subject: Option<EntityId>, weight: Confidence) -> Self {
        Self { subject, weight }
    }
}

/// Chooses a sampling frequency between `base` (no attention) and `max` (full attention).
///
/// Returns `None` if `max` is below `base`.
#[must_use]
pub fn attention_sampling(
    base: Frequency,
    max: Frequency,
    hint: AttentionHint,
) -> Option<Frequency> {
    if max.hz() < base.hz() {
        return None;
    }
    Frequency::from_hz(base.hz() + (max.hz() - base.hz()) * hint.weight.get())
}

/// A sensor that could satisfy a need, as seen by the selector.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct SensorCandidate {
    /// The sensor.
    pub sensor: SensorId,
    /// Its advertised capabilities.
    pub capabilities: SensorCapabilities,
    /// Its current health.
    pub health: SensorHealth,
    /// Its current noise level (standard deviation), if known. Lower is better.
    pub noise: Option<f64>,
}

impl SensorCandidate {
    /// A candidate sensor with its capabilities, health and known noise level.
    #[must_use]
    pub const fn new(
        sensor: SensorId,
        capabilities: SensorCapabilities,
        health: SensorHealth,
        noise: Option<f64>,
    ) -> Self {
        Self {
            sensor,
            capabilities,
            health,
            noise,
        }
    }
}

/// Picks the best sensor for a modality: it must support the modality and be operational, and
/// among those the one with the lowest known noise wins. Sensors with unknown noise rank last.
#[must_use]
pub fn select_sensor(
    candidates: &[SensorCandidate],
    modality: &crate::Modality,
) -> Option<SensorId> {
    candidates
        .iter()
        .filter(|c| c.capabilities.supports(modality) && c.health.is_operational())
        .min_by(|a, b| {
            let na = a.noise.unwrap_or(f64::INFINITY);
            let nb = b.noise.unwrap_or(f64::INFINITY);
            na.partial_cmp(&nb).unwrap_or(core::cmp::Ordering::Equal)
        })
        .map(|c| c.sensor)
}

#[cfg(test)]
#[allow(
    clippy::float_cmp,
    reason = "clamping and scaling results are exact in these cases"
)]
mod tests {
    use super::*;
    use crate::{Modality, Range};

    fn req(value: f64) -> ParameterChangeRequest {
        ParameterChangeRequest {
            sensor: SensorId::new(1),
            parameter: ParameterKind::Gain,
            value,
            reason: None,
        }
    }

    #[test]
    fn requests_are_clamped_to_advertised_range() {
        let mut caps = SensorCapabilities::new(alloc::vec![Modality::Visual]);
        caps.measurement_range = Range::new(0.0, 10.0);
        assert_eq!(clamp_to_capabilities(&req(42.0), &caps), 10.0);
        assert_eq!(clamp_to_capabilities(&req(-1.0), &caps), 0.0);
        let open = SensorCapabilities::new(alloc::vec![Modality::Visual]);
        assert_eq!(clamp_to_capabilities(&req(42.0), &open), 42.0);
    }

    #[test]
    fn low_confidence_triggers_information_request() {
        let c = Confidence::new(0.3).unwrap();
        let t = Confidence::new(0.7).unwrap();
        let r = information_request_for(
            ObservationId::new(5),
            Property::new("weed"),
            c,
            t,
            alloc::vec![],
        );
        assert_eq!(r.unwrap().need, InformationNeed::LowConfidence);
        assert!(
            information_request_for(
                ObservationId::new(5),
                Property::new("weed"),
                t,
                c,
                alloc::vec![]
            )
            .is_none()
        );
    }

    #[test]
    fn attention_scales_sampling_rate() {
        let base = Frequency::from_hz(10.0).unwrap();
        let max = Frequency::from_hz(100.0).unwrap();
        let half = AttentionHint {
            subject: None,
            weight: Confidence::new(0.5).unwrap(),
        };
        assert_eq!(attention_sampling(base, max, half).unwrap().hz(), 55.0);
        assert!(attention_sampling(max, base, half).is_none());
    }

    #[test]
    fn selector_prefers_operational_low_noise_sensor() {
        let mk = |id, health, noise| SensorCandidate {
            sensor: SensorId::new(id),
            capabilities: SensorCapabilities::new(alloc::vec![Modality::Thermal]),
            health,
            noise,
        };
        let candidates = [
            mk(1, SensorHealth::Faulted, Some(0.01)),
            mk(2, SensorHealth::Online, Some(0.5)),
            mk(3, SensorHealth::Online, Some(0.2)),
            mk(4, SensorHealth::Online, None),
        ];
        assert_eq!(
            select_sensor(&candidates, &Modality::Thermal),
            Some(SensorId::new(3))
        );
        assert_eq!(select_sensor(&candidates, &Modality::Lidar), None);
    }
}
