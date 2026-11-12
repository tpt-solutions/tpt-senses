//! # tpt-senses
//!
//! A general-purpose sensory and perception foundation for TPT's cognitive,
//! robotic and autonomous systems.
//!
//! The central principle:
//!
//! > **Sensors produce measurements. Perception produces observations.
//! > Cognition reasons about observations.**
//!
//! This crate provides the core data model:
//!
//! * [`Modality`] — what kind of quantity was sensed (extensible, with custom modalities).
//! * [`Measurement`] / [`MeasurementValue`] — raw or minimally processed sensor output.
//! * [`Observation`] — a semantically meaningful interpretation of measurements.
//! * [`Uncertainty`], [`Confidence`] — explicit, non-interchangeable quality descriptions.
//! * [`Provenance`] — the traceable chain from hardware to observation.
//! * [`Timestamp`], [`Duration`], [`Frequency`] — time with an explicit clock source.
//! * [`SensorHealth`], [`Diagnostics`] — sensors are themselves monitored entities.
//! * [`Sensor`] — the trait every sensor (real, simulated or internal) implements.
//!
//! The crate is `no_std` compatible and requires `alloc`. Enable the default
//! `std` feature for the `std::error::Error` integration.
//!
//! ## Example
//!
//! ```
//! use tpt_senses::prelude::*;
//!
//! let sensor = SensorId::new(1);
//! let timestamp = Timestamp::new(1_000_000, ClockSource::Monotonic);
//!
//! let provenance = Provenance::new(Origin::Real).with_step(ProvenanceStep::Sensor(sensor));
//! let measurement = Measurement::new(
//!     sensor,
//!     Modality::Thermal,
//!     MeasurementValue::Scalar(37.4),
//!     timestamp,
//!     provenance,
//! );
//!
//! let observation = Observation::from_measurement(
//!     &measurement,
//!     ObservationId::new(1),
//!     Property::new("temperature_state"),
//!     ObservationValue::Categorical("normal".into()),
//!     Confidence::new(0.95).expect("valid confidence"),
//! );
//!
//! assert_eq!(observation.modality(), &Modality::Thermal);
//! assert_eq!(observation.state(), ObservationState::Processed);
//! ```

#![cfg_attr(not(feature = "std"), no_std)]
#![doc(html_root_url = "https://docs.rs/tpt-senses/0.1.0")]

extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

mod adapter;
mod calibration;
mod capability;
mod control;
mod entity;
mod event;
mod fusion;
mod health;
mod id;
mod internal;
mod math;
mod measurement;
mod modality;
mod observation;
mod pipeline;
mod profile;
mod provenance;
mod record;
mod sensor;
mod spatial;
mod stream;
mod temporal;
mod timestamp;
mod trust;
mod uncertainty;
mod world;

pub use adapter::{Adapter, AdapterError};
pub use calibration::{CalibrationKind, CalibrationRecord, CalibrationState};
pub use capability::{
    AuditoryCapability, Range, Resolution, SensorCapabilities, SpectralRange, VisionCapability,
};
pub use control::{
    AttentionHint, InformationNeed, InformationRequest, ParameterChangeOutcome,
    ParameterChangeRequest, ParameterKind, SensorCandidate, attention_sampling,
    clamp_to_capabilities, information_request_for, select_sensor,
};
pub use entity::{AssociationConfig, EntityAssociation, EntityKind, associate};
pub use event::{Event, EventKind, movement_event, threshold_event};
pub use fusion::{
    ConfidenceWeighted, Disagreement, FusedObservation, FusionConfig, FusionError, FusionStrategy,
    InverseVariance, ScalarInput, fuse,
};
pub use health::{CalibrationStatus, Diagnostics, SensorHealth};
pub use id::{EntityId, MeasurementId, ObservationId, SensorId};
pub use internal::{InternalMetric, MetaSignal, internal_observation, meta_observation};
pub use measurement::{Measurement, MeasurementValue, SharedBuffer};
pub use modality::Modality;
pub use observation::{LifecycleError, Observation, ObservationState, ObservationValue, Property};
pub use pipeline::{
    CalibrationStage, FnStage, Pipeline, PipelineError, PipelineOutput, Stage, StageOutcome,
    StageRecord, ValidationStage,
};
pub use profile::SensoryProfile;
pub use provenance::{Algorithm, Origin, Provenance, ProvenanceStep};
pub use record::{RecordError, RecordSink, Recording, replay_through};
pub use sensor::{Sensor, SensorError};
pub use spatial::{CoordinateFrame, Point3, SpatialReference, Transform, Translation};
pub use stream::{
    BackpressurePolicy, BoundedQueue, Ingest, PushOutcome, RateLimiter, ReorderBuffer, Sample,
    SensorStream,
};
pub use temporal::{
    Freshness, TimingInfo, align_linear, align_nearest, freshness, observations_between,
    simultaneous, window_mean,
};
pub use timestamp::{ClockMismatch, ClockSource, Duration, Frequency, Timestamp};
pub use trust::{AuthenticationStatus, IntegrityStatus, SourceIdentity, TrustLevel, TrustMetadata};
pub use uncertainty::{Accuracy, Bounds, Confidence, Distribution, Precision, Uncertainty};
pub use world::{EntityView, WorldState};

/// Convenience re-exports of the most commonly used types.
pub mod prelude {
    pub use crate::{
        ClockSource, Confidence, EntityId, Measurement, MeasurementValue, Modality, Observation,
        ObservationId, ObservationState, ObservationValue, Origin, Property, Provenance,
        ProvenanceStep, Sensor, SensorError, SensorHealth, SensorId, Timestamp, Uncertainty,
    };
}
