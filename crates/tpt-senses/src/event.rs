//! Events (spec §21). An event is a notable change, not a fact: it carries confidence,
//! provenance and the measurements it was derived from.

use alloc::borrow::Cow;
use alloc::vec::Vec;

use crate::{
    Confidence, EntityId, Measurement, Modality, Observation, ObservationValue, Provenance,
    SensorId, Timestamp,
};

/// What happened.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum EventKind {
    /// An entity came into view or range.
    ObjectAppeared,
    /// An entity left view or range.
    ObjectDisappeared,
    /// An entity changed position.
    ObjectMoved,
    /// Contact or collision with something.
    CollisionDetected,
    /// A temperature crossed a limit.
    TemperatureExceeded,
    /// A sound was detected.
    SoundDetected,
    /// Pressure changed significantly.
    PressureChanged,
    /// A radiation level spiked.
    RadiationSpike,
    /// A sensor failed or degraded.
    SensorFailure,
    /// A sensor's calibration changed.
    CalibrationChanged,
    /// The environment changed (lighting, weather, ...).
    EnvironmentChanged,
    /// An event type not listed above.
    Custom(Cow<'static, str>),
}

/// A detected event.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct Event {
    /// What happened.
    pub kind: EventKind,
    /// When it happened, according to the detecting sensor's clock.
    pub timestamp: Timestamp,
    /// The sensor that detected it.
    pub source: SensorId,
    /// The entity involved, if any.
    pub entity: Option<EntityId>,
    /// The modality the event was detected in.
    pub modality: Modality,
    /// The measurements the event was derived from.
    pub measurements: Vec<Measurement>,
    /// How confident the detector is that the event occurred.
    pub confidence: Confidence,
    /// Where the event information came from.
    pub provenance: Provenance,
}

impl Event {
    /// Creates an event with no entity and no supporting measurements.
    #[must_use]
    pub fn new(
        kind: EventKind,
        timestamp: Timestamp,
        source: SensorId,
        modality: Modality,
        confidence: Confidence,
        provenance: Provenance,
    ) -> Self {
        Self {
            kind,
            timestamp,
            source,
            entity: None,
            modality,
            measurements: Vec::new(),
            confidence,
            provenance,
        }
    }

    /// Sets the entity involved (builder style).
    #[must_use]
    pub const fn with_entity(mut self, entity: EntityId) -> Self {
        self.entity = Some(entity);
        self
    }

    /// Adds a supporting measurement (builder style).
    #[must_use]
    pub fn with_measurement(mut self, measurement: Measurement) -> Self {
        self.measurements.push(measurement);
        self
    }
}

/// Detects a scalar observation crossing above `threshold` and reports it as an event of `kind`.
///
/// Returns `None` if the observation is not scalar or does not exceed the threshold. The event
/// inherits the observation's confidence, timestamp, subject and provenance.
#[must_use]
pub fn threshold_event(
    observation: &Observation,
    kind: EventKind,
    threshold: f64,
    source: SensorId,
) -> Option<Event> {
    let ObservationValue::Scalar(value) = observation.value() else {
        return None;
    };
    if *value <= threshold {
        return None;
    }
    let mut event = Event::new(
        kind,
        observation.timestamp(),
        source,
        observation.modality().clone(),
        observation.confidence(),
        observation.provenance().clone(),
    );
    event.entity = observation.subject();
    Some(event)
}

/// Detects movement of an entity between two located observations of it.
///
/// Returns `None` unless both observations have a position in the same frame, the later one is
/// on the same clock and not earlier, and the distance is at least `min_distance`. The event's
/// confidence is the lower of the two observations' confidences, because movement is only as
/// reliable as its weaker end.
#[must_use]
pub fn movement_event(
    previous: &Observation,
    current: &Observation,
    min_distance: f64,
    source: SensorId,
) -> Option<Event> {
    let (prev_loc, curr_loc) = (previous.location()?, current.location()?);
    if prev_loc.frame != curr_loc.frame {
        return None;
    }
    let (a, b) = (prev_loc.position?, curr_loc.position?);
    if current.timestamp() < previous.timestamp()
        || !current.timestamp().is_same_clock(previous.timestamp())
    {
        return None;
    }
    let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    let distance = crate::math::sqrt(d[0] * d[0] + d[1] * d[1] + d[2] * d[2]);
    if distance < min_distance {
        return None;
    }
    let confidence = if previous.confidence() <= current.confidence() {
        previous.confidence()
    } else {
        current.confidence()
    };
    let mut provenance = previous.provenance().clone();
    for step in current.provenance().chain() {
        provenance.push(step.clone());
    }
    let mut event = Event::new(
        EventKind::ObjectMoved,
        current.timestamp(),
        source,
        current.modality().clone(),
        confidence,
        provenance,
    );
    event.entity = current.subject().or(previous.subject());
    Some(event)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ClockSource, Origin};

    #[test]
    fn event_builder_collects_evidence() {
        let e = Event::new(
            EventKind::ObjectAppeared,
            Timestamp::new(5, ClockSource::Monotonic),
            SensorId::new(1),
            Modality::Visual,
            Confidence::new(0.8).unwrap(),
            Provenance::new(Origin::Real),
        )
        .with_entity(EntityId::new(27));
        assert_eq!(e.entity, Some(EntityId::new(27)));
        assert!(e.measurements.is_empty());
        assert_ne!(e.kind, EventKind::Custom("x".into()));
    }
}
