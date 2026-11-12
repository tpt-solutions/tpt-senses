//! Perception pipeline (spec §26, §47).
//!
//! A pipeline is an ordered list of optional stages. Each stage takes an [`Observation`],
//! may revise it, advances its lifecycle state, and records itself in the provenance chain. A
//! stage may also reject the observation, which stops the pipeline with a reason. Every run
//! returns a trace describing each stage's input and output, so any result can be explained.

use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use crate::{
    Algorithm, CalibrationKind, CalibrationState, Confidence, Observation, ObservationState,
    ProvenanceStep, Timestamp,
};

/// What a stage decided.
#[derive(Debug, PartialEq)]
#[allow(
    clippy::large_enum_variant,
    reason = "observations are passed by value through the pipeline; boxing would add an allocation per stage"
)]
pub enum StageOutcome {
    /// Continue with the (possibly revised) observation.
    Pass(Observation),
    /// Stop the pipeline. The string explains why.
    Reject(String),
}

/// One step of perception processing.
pub trait Stage {
    /// The algorithm this stage runs, with its version and parameters. Recorded in provenance.
    fn algorithm(&self) -> Algorithm;

    /// The lifecycle state the observation is in after this stage passes.
    fn target_state(&self) -> ObservationState;

    /// Processes one observation.
    fn apply(&self, observation: Observation) -> StageOutcome;
}

/// What one stage did during a pipeline run.
#[derive(Clone, Debug, PartialEq)]
pub struct StageRecord {
    /// The algorithm the stage ran.
    pub algorithm: Algorithm,
    /// Confidence before the stage ran.
    pub input_confidence: Confidence,
    /// Confidence after the stage ran.
    pub output_confidence: Confidence,
    /// Lifecycle state after the stage ran.
    pub output_state: ObservationState,
}

/// A successful pipeline run.
#[derive(Clone, Debug, PartialEq)]
pub struct PipelineOutput {
    /// The processed observation.
    pub observation: Observation,
    /// One record per stage, in order.
    pub trace: Vec<StageRecord>,
}

/// A pipeline run that was stopped by a stage.
#[derive(Clone, Debug, PartialEq)]
pub struct PipelineError {
    /// Name of the algorithm that rejected the observation.
    pub stage: String,
    /// Why it was rejected.
    pub reason: String,
    /// Records for the stages that ran before the rejection.
    pub trace: Vec<StageRecord>,
}

/// An ordered list of stages.
#[derive(Default)]
pub struct Pipeline {
    stages: Vec<Box<dyn Stage>>,
}

impl Pipeline {
    /// An empty pipeline. An empty pipeline returns its input unchanged.
    #[must_use]
    pub fn new() -> Self {
        Self { stages: Vec::new() }
    }

    /// Appends a stage (builder style).
    #[must_use]
    pub fn with_stage(mut self, stage: impl Stage + 'static) -> Self {
        self.stages.push(Box::new(stage));
        self
    }

    /// Number of stages.
    #[must_use]
    pub fn len(&self) -> usize {
        self.stages.len()
    }

    /// Returns `true` if the pipeline has no stages.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.stages.is_empty()
    }

    /// Runs `observation` through every stage.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError`] if any stage rejects the observation.
    pub fn run(&self, observation: Observation) -> Result<PipelineOutput, PipelineError> {
        let mut current = observation;
        let mut trace = Vec::with_capacity(self.stages.len());

        for stage in &self.stages {
            let algorithm = stage.algorithm();
            let input_confidence = current.confidence();

            let mut out = match stage.apply(current) {
                StageOutcome::Pass(out) => out,
                StageOutcome::Reject(reason) => {
                    return Err(PipelineError {
                        stage: algorithm.name,
                        reason,
                        trace,
                    });
                }
            };

            let target = stage.target_state();
            if target > out.state() {
                if let Err(e) = out.advance_to(target) {
                    return Err(PipelineError {
                        stage: algorithm.name,
                        reason: format!("{e}"),
                        trace,
                    });
                }
            }
            out.record_step(ProvenanceStep::Algorithm(algorithm.clone()));

            trace.push(StageRecord {
                algorithm,
                input_confidence,
                output_confidence: out.confidence(),
                output_state: out.state(),
            });
            current = out;
        }

        Ok(PipelineOutput {
            observation: current,
            trace,
        })
    }
}

/// Rejects observations that are too weak or known to be bad.
#[derive(Clone, Debug, PartialEq)]
pub struct ValidationStage {
    min_confidence: Confidence,
}

impl ValidationStage {
    /// Creates a validation stage that rejects confidence below `min_confidence`.
    #[must_use]
    pub const fn new(min_confidence: Confidence) -> Self {
        Self { min_confidence }
    }
}

impl Stage for ValidationStage {
    fn algorithm(&self) -> Algorithm {
        Algorithm::new("validation", "1")
            .with_parameter("min_confidence", format!("{}", self.min_confidence.get()))
    }

    fn target_state(&self) -> ObservationState {
        ObservationState::Validated
    }

    fn apply(&self, observation: Observation) -> StageOutcome {
        if observation.trust().is_known_bad() {
            return StageOutcome::Reject("source is known to be untrusted or corrupted".into());
        }
        if let Some(u) = observation.uncertainty() {
            if !u.is_valid() {
                return StageOutcome::Reject("uncertainty is not finite and non-negative".into());
            }
        }
        if observation.confidence() < self.min_confidence {
            return StageOutcome::Reject(format!(
                "confidence {} below minimum {}",
                observation.confidence().get(),
                self.min_confidence.get()
            ));
        }
        StageOutcome::Pass(observation)
    }
}

/// Applies a calibration to the observation, or rejects it if no usable calibration exists.
///
/// A sensor whose calibration failed or expired must not produce data that looks calibrated.
#[derive(Clone, Debug, PartialEq)]
pub struct CalibrationStage {
    state: CalibrationState,
    kind: CalibrationKind,
    at: Timestamp,
}

impl CalibrationStage {
    /// Creates a stage that requires a calibration of `kind` usable at time `at`.
    #[must_use]
    pub const fn new(state: CalibrationState, kind: CalibrationKind, at: Timestamp) -> Self {
        Self { state, kind, at }
    }
}

impl Stage for CalibrationStage {
    fn algorithm(&self) -> Algorithm {
        Algorithm::new("calibration-check", "1").with_parameter("kind", format!("{:?}", self.kind))
    }

    fn target_state(&self) -> ObservationState {
        ObservationState::Calibrated
    }

    fn apply(&self, mut observation: Observation) -> StageOutcome {
        match self.state.usable(self.kind, self.at) {
            Some(record) => {
                observation.record_step(ProvenanceStep::Calibration(record.clone()));
                StageOutcome::Pass(observation)
            }
            None => StageOutcome::Reject(format!("no usable {:?} calibration", self.kind)),
        }
    }
}

/// A stage defined by a closure, for project-specific processing.
pub struct FnStage<F> {
    algorithm: Algorithm,
    target: ObservationState,
    f: F,
}

impl<F> FnStage<F>
where
    F: Fn(Observation) -> StageOutcome,
{
    /// Creates a stage that runs `f` and advances to `target` on success.
    #[must_use]
    pub const fn new(algorithm: Algorithm, target: ObservationState, f: F) -> Self {
        Self {
            algorithm,
            target,
            f,
        }
    }
}

impl<F> Stage for FnStage<F>
where
    F: Fn(Observation) -> StageOutcome,
{
    fn algorithm(&self) -> Algorithm {
        self.algorithm.clone()
    }

    fn target_state(&self) -> ObservationState {
        self.target
    }

    fn apply(&self, observation: Observation) -> StageOutcome {
        (self.f)(observation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ClockSource, Modality, ObservationId, ObservationValue, Origin, Property, Provenance,
        TrustMetadata,
    };

    fn t(ns: u64) -> Timestamp {
        Timestamp::new(ns, ClockSource::Monotonic)
    }

    fn obs(conf: f64) -> Observation {
        Observation::new(
            ObservationId::new(1),
            Modality::Thermal,
            Property::new("temperature"),
            ObservationValue::Scalar(37.0),
            Confidence::new(conf).unwrap(),
            t(100),
            Provenance::new(Origin::Real),
        )
    }

    #[test]
    fn empty_pipeline_is_identity() {
        let out = Pipeline::new().run(obs(0.5)).unwrap();
        assert_eq!(out.observation.state(), ObservationState::Raw);
        assert!(out.trace.is_empty());
    }

    #[test]
    fn stages_advance_state_and_record_provenance() {
        let mut state = CalibrationState::new();
        state.record(crate::CalibrationRecord::new(
            CalibrationKind::Temperature,
            t(0),
        ));
        let pipeline = Pipeline::new()
            .with_stage(ValidationStage::new(Confidence::new(0.5).unwrap()))
            .with_stage(CalibrationStage::new(
                state,
                CalibrationKind::Temperature,
                t(50),
            ));
        let out = pipeline.run(obs(0.9)).unwrap();
        assert_eq!(out.observation.state(), ObservationState::Calibrated);
        assert_eq!(out.trace.len(), 2);
        assert_eq!(out.observation.provenance().algorithms().count(), 2);
        assert_eq!(out.observation.provenance().calibrations().count(), 1);
    }

    #[test]
    fn low_confidence_is_rejected_with_trace() {
        let pipeline = Pipeline::new()
            .with_stage(ValidationStage::new(Confidence::new(0.5).unwrap()))
            .with_stage(ValidationStage::new(Confidence::new(0.1).unwrap()));
        let err = pipeline.run(obs(0.2)).unwrap_err();
        assert_eq!(err.stage, "validation");
        assert!(err.reason.contains("below minimum"));
        assert!(err.trace.is_empty());
    }

    #[test]
    fn failed_calibration_stops_the_pipeline() {
        let mut state = CalibrationState::new();
        state.record(crate::CalibrationRecord::new(CalibrationKind::Temperature, t(0)).failed());
        let pipeline = Pipeline::new().with_stage(CalibrationStage::new(
            state,
            CalibrationKind::Temperature,
            t(10),
        ));
        let err = pipeline.run(obs(0.9)).unwrap_err();
        assert_eq!(err.stage, "calibration-check");
    }

    #[test]
    fn untrusted_source_is_rejected() {
        let bad = obs(0.9).with_trust(TrustMetadata {
            integrity: crate::IntegrityStatus::Corrupted,
            ..TrustMetadata::unverified()
        });
        let pipeline =
            Pipeline::new().with_stage(ValidationStage::new(Confidence::new(0.1).unwrap()));
        assert!(pipeline.run(bad).is_err());
    }

    #[test]
    fn custom_stage_runs_and_is_recorded() {
        let stage = FnStage::new(
            Algorithm::new("double-confidence", "1"),
            ObservationState::Processed,
            |mut o: Observation| {
                let c = (o.confidence().get() * 2.0).min(1.0);
                o.set_confidence(Confidence::new(c).unwrap());
                StageOutcome::Pass(o)
            },
        );
        let out = Pipeline::new().with_stage(stage).run(obs(0.3)).unwrap();
        assert!((out.trace[0].output_confidence.get() - 0.6).abs() < 1e-12);
        assert_eq!(out.observation.state(), ObservationState::Processed);
    }
}
