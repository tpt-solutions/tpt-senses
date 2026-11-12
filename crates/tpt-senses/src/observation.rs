//! Observations: semantically meaningful interpretations of measurements (spec §13, §14, §34).

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use crate::{
    Confidence, EntityId, Measurement, Modality, ObservationId, Provenance, ProvenanceStep,
    SpatialReference, Timestamp, TrustMetadata, Uncertainty,
};

/// The property an observation describes, e.g. `possible_disease` or `temperature_state`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Property(String);

impl Property {
    /// Creates a property name.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// The property name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Property {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// The value an observation asserts about its property.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ObservationValue {
    /// True/false.
    Boolean(bool),
    /// A number.
    Scalar(f64),
    /// A label such as `normal` or `fungal_stress`.
    Categorical(String),
    /// A vector of numbers.
    Vector(Vec<f64>),
}

/// Lifecycle stages an observation moves through (spec §34).
///
/// Stages are ordered. Not every observation must pass through every stage, but an observation
/// can only move forward.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ObservationState {
    /// Just produced; nothing checked.
    Raw,
    /// Passed validation.
    Validated,
    /// Calibration applied.
    Calibrated,
    /// Processed by a perception stage.
    Processed,
    /// Associated with other evidence or entities.
    Correlated,
    /// Interpreted as an observation of the world.
    Observed,
    /// Combined with other observations.
    Fused,
    /// Delivered to a consumer.
    Consumed,
    /// Stored for later reference.
    Archived,
}

impl ObservationState {
    /// Returns `true` if moving from `self` to `next` is a forward transition.
    #[must_use]
    pub fn can_advance_to(self, next: Self) -> bool {
        next > self
    }
}

/// Returned when an observation is asked to move backwards or to its current stage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct LifecycleError {
    /// The current state.
    pub from: ObservationState,
    /// The requested state.
    pub to: ObservationState,
}

impl fmt::Display for LifecycleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid observation transition from {:?} to {:?}",
            self.from, self.to
        )
    }
}

#[cfg(feature = "std")]
impl std::error::Error for LifecycleError {}

/// A semantically meaningful interpretation of one or more measurements.
///
/// An observation is evidence, not fact: it carries a [`Confidence`], its [`Provenance`] and a
/// lifecycle state. Nothing in this type marks it as true.
///
/// # Example
///
/// ```
/// use tpt_senses::prelude::*;
/// use tpt_senses::StandardDeviation;
///
/// let range = Observation::new(
///     ObservationId::new(1),
///     Modality::Depth,
///     Property::new("range_m"),
///     ObservationValue::Scalar(12.5),
///     Confidence::new(0.9).unwrap(),
///     Timestamp::new(1_000_000, ClockSource::Monotonic),
///     Provenance::new(Origin::Real).with_step(ProvenanceStep::Sensor(SensorId::new(7))),
/// )
/// .with_uncertainty(
///     Uncertainty::new().with_standard_deviation(StandardDeviation::new(0.05).unwrap()),
/// );
///
/// // New observations start as raw evidence, not as facts.
/// assert_eq!(range.state(), ObservationState::Raw);
/// assert_eq!(range.confidence().get(), 0.9);
/// ```
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Observation {
    id: ObservationId,
    subject: Option<EntityId>,
    modality: Modality,
    property: Property,
    value: ObservationValue,
    confidence: Confidence,
    uncertainty: Option<Uncertainty>,
    location: Option<SpatialReference>,
    timestamp: Timestamp,
    provenance: Provenance,
    trust: TrustMetadata,
    state: ObservationState,
}

impl Observation {
    /// Creates a new observation in the [`ObservationState::Raw`] state.
    #[must_use]
    pub fn new(
        id: ObservationId,
        modality: Modality,
        property: Property,
        value: ObservationValue,
        confidence: Confidence,
        timestamp: Timestamp,
        provenance: Provenance,
    ) -> Self {
        Self {
            id,
            subject: None,
            modality,
            property,
            value,
            confidence,
            uncertainty: None,
            location: None,
            timestamp,
            provenance,
            trust: TrustMetadata::unverified(),
            state: ObservationState::Raw,
        }
    }

    /// Builds an observation from a single measurement (simple sensor, no pipeline; spec §26).
    ///
    /// The measurement's timestamp, uncertainty and provenance are carried over, with the
    /// producing sensor appended to the provenance chain. The result is in the
    /// [`ObservationState::Processed`] state.
    #[must_use]
    pub fn from_measurement(
        measurement: &Measurement,
        id: ObservationId,
        property: Property,
        value: ObservationValue,
        confidence: Confidence,
    ) -> Self {
        let mut provenance = measurement.provenance.clone();
        provenance.push(ProvenanceStep::Sensor(measurement.sensor));
        Self {
            id,
            subject: None,
            modality: measurement.modality.clone(),
            property,
            value,
            confidence,
            uncertainty: measurement.uncertainty.clone(),
            location: None,
            timestamp: measurement.timestamp,
            provenance,
            trust: TrustMetadata::unverified(),
            state: ObservationState::Processed,
        }
    }

    /// Sets the entity this observation is about (builder style).
    #[must_use]
    pub const fn with_subject(mut self, subject: EntityId) -> Self {
        self.subject = Some(subject);
        self
    }

    /// Attaches uncertainty (builder style).
    #[must_use]
    pub fn with_uncertainty(mut self, uncertainty: Uncertainty) -> Self {
        self.uncertainty = Some(uncertainty);
        self
    }

    /// Sets where the observation was made (builder style).
    #[must_use]
    pub fn with_location(mut self, location: SpatialReference) -> Self {
        self.location = Some(location);
        self
    }

    /// Sets trust metadata (builder style). Observations are untrusted until this is called.
    #[must_use]
    pub fn with_trust(mut self, trust: TrustMetadata) -> Self {
        self.trust = trust;
        self
    }

    /// Appends a step to this observation's provenance chain. Used by pipeline stages so that
    /// every transformation is recorded.
    pub fn record_step(&mut self, step: ProvenanceStep) {
        self.provenance.push(step);
    }

    /// Replaces the confidence (used by stages that revise it).
    pub fn set_confidence(&mut self, confidence: Confidence) {
        self.confidence = confidence;
    }

    /// Moves the observation forward through its lifecycle.
    ///
    /// # Errors
    ///
    /// Returns [`LifecycleError`] if `next` is not strictly after the current state.
    pub fn advance_to(&mut self, next: ObservationState) -> Result<(), LifecycleError> {
        if self.state.can_advance_to(next) {
            self.state = next;
            Ok(())
        } else {
            Err(LifecycleError {
                from: self.state,
                to: next,
            })
        }
    }

    /// The observation identifier.
    #[must_use]
    pub const fn id(&self) -> ObservationId {
        self.id
    }

    /// The entity this observation is about, if known.
    #[must_use]
    pub const fn subject(&self) -> Option<EntityId> {
        self.subject
    }

    /// The modality the observation was derived from.
    #[must_use]
    pub const fn modality(&self) -> &Modality {
        &self.modality
    }

    /// The property being asserted.
    #[must_use]
    pub const fn property(&self) -> &Property {
        &self.property
    }

    /// The asserted value.
    #[must_use]
    pub const fn value(&self) -> &ObservationValue {
        &self.value
    }

    /// The confidence in the assertion.
    #[must_use]
    pub const fn confidence(&self) -> Confidence {
        self.confidence
    }

    /// Quantified uncertainty, if known.
    #[must_use]
    pub const fn uncertainty(&self) -> Option<&Uncertainty> {
        self.uncertainty.as_ref()
    }

    /// Where the observation was made, if known.
    #[must_use]
    pub const fn location(&self) -> Option<&SpatialReference> {
        self.location.as_ref()
    }

    /// Trust metadata. Defaults to unverified; an observation is never trusted by default.
    #[must_use]
    pub const fn trust(&self) -> &TrustMetadata {
        &self.trust
    }

    /// When the underlying data was taken.
    #[must_use]
    pub const fn timestamp(&self) -> Timestamp {
        self.timestamp
    }

    /// Where the observation came from.
    #[must_use]
    pub const fn provenance(&self) -> &Provenance {
        &self.provenance
    }

    /// The current lifecycle state.
    #[must_use]
    pub const fn state(&self) -> ObservationState {
        self.state
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ClockSource, MeasurementValue, Origin, SensorId};

    fn sample_measurement() -> Measurement {
        Measurement::new(
            SensorId::new(3),
            Modality::Thermal,
            MeasurementValue::Scalar(37.4),
            Timestamp::new(42, ClockSource::Monotonic),
            Provenance::new(Origin::Real),
        )
    }

    #[test]
    fn from_measurement_carries_provenance_and_sensor() {
        let m = sample_measurement();
        let o = Observation::from_measurement(
            &m,
            ObservationId::new(1),
            Property::new("temperature_state"),
            ObservationValue::Categorical("normal".into()),
            Confidence::new(0.9).unwrap(),
        );
        assert_eq!(o.state(), ObservationState::Processed);
        assert_eq!(o.timestamp(), m.timestamp);
        assert_eq!(
            o.provenance().sensors().collect::<Vec<_>>(),
            [SensorId::new(3)]
        );
        assert_eq!(o.provenance().origin(), Origin::Real);
    }

    #[test]
    fn lifecycle_only_moves_forward() {
        let mut o = Observation::new(
            ObservationId::new(9),
            Modality::Visual,
            Property::new("presence"),
            ObservationValue::Boolean(true),
            Confidence::new(0.5).unwrap(),
            Timestamp::new(0, ClockSource::Unknown),
            Provenance::new(Origin::Unknown),
        );
        assert_eq!(o.state(), ObservationState::Raw);
        // Skipping stages is allowed.
        o.advance_to(ObservationState::Fused).unwrap();
        assert_eq!(o.state(), ObservationState::Fused);
        // Going backwards is not.
        let err = o.advance_to(ObservationState::Validated).unwrap_err();
        assert_eq!(err.from, ObservationState::Fused);
        assert_eq!(o.state(), ObservationState::Fused);
    }

    #[test]
    fn new_observations_are_untrusted() {
        let o = Observation::new(
            ObservationId::new(7),
            Modality::Visual,
            Property::new("presence"),
            ObservationValue::Boolean(true),
            Confidence::new(0.9).unwrap(),
            Timestamp::new(0, ClockSource::Unknown),
            Provenance::new(Origin::Unknown),
        );
        assert_eq!(o.trust(), &TrustMetadata::unverified());
        assert!(o.location().is_none());
    }

    #[test]
    fn observation_is_not_marked_as_fact() {
        // A new observation is only evidence: it starts Raw, never Archived/Observed.
        let o = Observation::new(
            ObservationId::new(2),
            Modality::Chemical,
            Property::new("compound"),
            ObservationValue::Categorical("CO2".into()),
            Confidence::new(0.1).unwrap(),
            Timestamp::new(0, ClockSource::Unknown),
            Provenance::new(Origin::Unknown),
        );
        assert!(o.state() < ObservationState::Observed);
    }
}
