//! Internal sensing (spec §23): the system observing its own body and health with the same
//! observation model used for the outside world.
//!
//! [`MetaSignal`] is an interface for higher-level internal observations (prediction error,
//! contradiction, attention demand, ...). This crate only *represents* them. It never decides
//! what they mean or what to do about them, because that is cognitive state.

use alloc::string::String;

use crate::{
    Confidence, Modality, Observation, ObservationId, ObservationValue, Property, Provenance,
    Timestamp,
};

/// Measurable internal state of the system.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum InternalMetric {
    /// Battery state of charge.
    BatteryLevel,
    /// CPU / GPU load.
    ProcessorLoad,
    /// Memory pressure.
    MemoryPressure,
    /// Temperature of the system or a board.
    Temperature,
    /// Temperature of a motor.
    MotorTemperature,
    /// Joint position.
    JointPosition,
    /// Joint torque.
    JointTorque,
    /// Network link quality.
    NetworkQuality,
    /// Health of a sensor.
    SensorHealth,
    /// Health of an actuator.
    ActuatorHealth,
}

impl InternalMetric {
    /// Every metric, for iteration and tests.
    pub const ALL: [Self; 10] = [
        Self::BatteryLevel,
        Self::ProcessorLoad,
        Self::MemoryPressure,
        Self::Temperature,
        Self::MotorTemperature,
        Self::JointPosition,
        Self::JointTorque,
        Self::NetworkQuality,
        Self::SensorHealth,
        Self::ActuatorHealth,
    ];

    /// The property name used in observations of this metric.
    #[must_use]
    pub const fn property_name(self) -> &'static str {
        match self {
            Self::BatteryLevel => "battery_level",
            Self::ProcessorLoad => "processor_load",
            Self::MemoryPressure => "memory_pressure",
            Self::Temperature => "temperature",
            Self::MotorTemperature => "motor_temperature",
            Self::JointPosition => "joint_position",
            Self::JointTorque => "joint_torque",
            Self::NetworkQuality => "network_quality",
            Self::SensorHealth => "sensor_health",
            Self::ActuatorHealth => "actuator_health",
        }
    }

    /// The modality internal observations of this metric are reported under.
    #[must_use]
    pub fn modality(self) -> Modality {
        match self {
            Self::Temperature | Self::MotorTemperature => Modality::Thermal,
            Self::JointPosition => Modality::Position,
            Self::JointTorque => Modality::Torque,
            _ => Modality::Custom("internal".into()),
        }
    }

    /// Recognises an internal metric from an observation's property.
    #[must_use]
    pub fn from_property(property: &Property) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|m| m.property_name() == property.as_str())
    }
}

/// Higher-level internal signals. Represented, not interpreted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum MetaSignal {
    /// The system's prediction differed from what was observed.
    PredictionError,
    /// Uncertainty about the current state is high.
    UncertaintyLevel,
    /// Two pieces of evidence contradict each other.
    Contradiction,
    /// Something requests more attention or sensing.
    AttentionDemand,
    /// Two goals cannot both be satisfied.
    GoalConflict,
}

impl MetaSignal {
    /// The property name used in observations of this signal.
    #[must_use]
    pub const fn property_name(self) -> &'static str {
        match self {
            Self::PredictionError => "meta.prediction_error",
            Self::UncertaintyLevel => "meta.uncertainty_level",
            Self::Contradiction => "meta.contradiction",
            Self::AttentionDemand => "meta.attention_demand",
            Self::GoalConflict => "meta.goal_conflict",
        }
    }
}

/// Builds an internal observation with the standard observation model.
#[must_use]
pub fn internal_observation(
    id: ObservationId,
    metric: InternalMetric,
    value: f64,
    confidence: Confidence,
    timestamp: Timestamp,
    provenance: Provenance,
) -> Observation {
    Observation::new(
        id,
        metric.modality(),
        Property::new(metric.property_name()),
        ObservationValue::Scalar(value),
        confidence,
        timestamp,
        provenance,
    )
}

/// Builds a meta-signal observation. The value is a score in whatever unit the producer uses.
#[must_use]
pub fn meta_observation(
    id: ObservationId,
    signal: MetaSignal,
    score: f64,
    confidence: Confidence,
    timestamp: Timestamp,
    provenance: Provenance,
) -> Observation {
    let name: String = signal.property_name().into();
    Observation::new(
        id,
        Modality::Custom("meta".into()),
        Property::new(name),
        ObservationValue::Scalar(score),
        confidence,
        timestamp,
        provenance,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ClockSource, Origin};

    #[test]
    fn metrics_round_trip_through_property_names() {
        for metric in InternalMetric::ALL {
            let p = Property::new(metric.property_name());
            assert_eq!(InternalMetric::from_property(&p), Some(metric));
        }
        assert_eq!(
            InternalMetric::from_property(&Property::new("presence")),
            None
        );
    }

    #[test]
    fn internal_observation_uses_standard_model() {
        let o = internal_observation(
            ObservationId::new(1),
            InternalMetric::BatteryLevel,
            0.42,
            Confidence::new(1.0).unwrap(),
            Timestamp::new(0, ClockSource::Hardware),
            Provenance::new(Origin::Real),
        );
        assert_eq!(o.property().as_str(), "battery_level");
        assert_eq!(o.value(), &ObservationValue::Scalar(0.42));
    }

    #[test]
    fn meta_signals_are_representable() {
        let o = meta_observation(
            ObservationId::new(2),
            MetaSignal::Contradiction,
            0.7,
            Confidence::new(0.6).unwrap(),
            Timestamp::new(0, ClockSource::Monotonic),
            Provenance::new(Origin::Unknown),
        );
        assert_eq!(o.property().as_str(), "meta.contradiction");
    }
}
