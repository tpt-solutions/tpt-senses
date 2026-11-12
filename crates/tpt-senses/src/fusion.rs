//! Sensor fusion (spec §19, §48).
//!
//! Fusion combines several scalar observations of the same property into one observation.
//! Three rules hold throughout:
//!
//! 1. Inputs must be simultaneous within a stated tolerance, or fusion is refused. Samples from
//!    different clocks are never fused.
//! 2. The fused observation's provenance contains every input's full provenance chain, so the
//!    evidence is never erased.
//! 3. Disagreement between sensors is reported, not hidden.

use alloc::string::String;
use alloc::vec::Vec;

use crate::{
    Algorithm, ClockMismatch, Confidence, Duration, Modality, Observation, ObservationId,
    ObservationState, ObservationValue, Origin, Property, Provenance, ProvenanceStep, SensorId,
    StandardDeviation, Uncertainty, simultaneous,
};

/// One scalar input to a fusion strategy.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScalarInput {
    /// The measured value.
    pub value: f64,
    /// Confidence in the value, in `[0, 1]`.
    pub confidence: f64,
    /// Standard deviation of the value, if known.
    pub sigma: Option<f64>,
}

/// A way of combining scalar inputs into one value. Implement this for custom strategies.
pub trait FusionStrategy {
    /// Algorithm name recorded in provenance.
    fn name(&self) -> &'static str;

    /// Algorithm version recorded in provenance.
    fn version(&self) -> &'static str {
        "1"
    }

    /// Parameters recorded in provenance, so the strategy can be reproduced.
    fn parameters(&self) -> Vec<(String, String)> {
        Vec::new()
    }

    /// Combines the inputs into `(value, standard deviation if known)`.
    ///
    /// Returns `None` if the strategy cannot be applied to these inputs.
    fn combine(&self, inputs: &[ScalarInput]) -> Option<(f64, Option<f64>)>;
}

/// Confidence-weighted mean. Uses no uncertainty information.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ConfidenceWeighted;

impl FusionStrategy for ConfidenceWeighted {
    fn name(&self) -> &'static str {
        "confidence-weighted-mean"
    }

    fn combine(&self, inputs: &[ScalarInput]) -> Option<(f64, Option<f64>)> {
        let total: f64 = inputs.iter().map(|i| i.confidence).sum();
        if total <= 0.0 {
            return None;
        }
        let value = inputs.iter().map(|i| i.confidence * i.value).sum::<f64>() / total;
        Some((value, None))
    }
}

/// Gaussian (inverse-variance) fusion: the Bayesian product of independent Gaussian likelihoods.
///
/// Requires a positive standard deviation on every input. The result is at least as precise as
/// the best input.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct InverseVariance;

impl FusionStrategy for InverseVariance {
    fn name(&self) -> &'static str {
        "inverse-variance-gaussian"
    }

    fn parameters(&self) -> Vec<(String, String)> {
        alloc::vec![("assumes".into(), "independent gaussian errors".into())]
    }

    fn combine(&self, inputs: &[ScalarInput]) -> Option<(f64, Option<f64>)> {
        let mut weight_sum = 0.0;
        let mut weighted_sum = 0.0;
        for input in inputs {
            let sigma = input.sigma.filter(|s| *s > 0.0)?;
            let w = 1.0 / (sigma * sigma);
            weight_sum += w;
            weighted_sum += w * input.value;
        }
        if weight_sum <= 0.0 {
            return None;
        }
        Some((
            weighted_sum / weight_sum,
            Some(crate::math::sqrt(1.0 / weight_sum)),
        ))
    }
}

/// Settings for one fusion call.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FusionConfig {
    /// Inputs further apart in time than this are refused.
    pub time_tolerance: Duration,
    /// A spread (max − min) above this is reported as disagreement.
    pub disagreement_threshold: f64,
}

/// A sensor whose value disagreed with the fused result.
#[derive(Clone, Debug, PartialEq)]
pub struct Disagreement {
    /// Largest difference between any two inputs.
    pub spread: f64,
    /// Sensors whose value differs from the median of all inputs by more than the threshold.
    pub outlier_sensors: Vec<SensorId>,
}

/// A fused observation and any disagreement found among its inputs.
#[derive(Clone, Debug, PartialEq)]
pub struct FusedObservation {
    /// The fused observation, in the [`ObservationState::Fused`] state.
    pub observation: Observation,
    /// Present when the inputs disagree by more than the configured threshold.
    pub disagreement: Option<Disagreement>,
}

/// Why fusion was refused.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum FusionError {
    /// No inputs were given.
    Empty,
    /// Inputs describe different properties.
    PropertyMismatch,
    /// An input is not a scalar observation.
    NotScalar,
    /// Inputs come from different clocks, so their simultaneity cannot be decided.
    ClockMismatch(ClockMismatch),
    /// Inputs are further apart in time than the tolerance allows.
    NotSimultaneous,
    /// The strategy could not combine these inputs, or produced an invalid result.
    StrategyFailed,
}

/// Fuses scalar observations of one property.
///
/// # Errors
///
/// Returns [`FusionError`] if the inputs are empty, mismatched, not scalar, not simultaneous
/// within `config.time_tolerance`, or cannot be combined by `strategy`.
pub fn fuse(
    inputs: &[Observation],
    id: ObservationId,
    strategy: &dyn FusionStrategy,
    config: &FusionConfig,
) -> Result<FusedObservation, FusionError> {
    let first = inputs.first().ok_or(FusionError::Empty)?;

    let mut scalars = Vec::with_capacity(inputs.len());
    for input in inputs {
        if input.property() != first.property() {
            return Err(FusionError::PropertyMismatch);
        }
        match simultaneous(first.timestamp(), input.timestamp(), config.time_tolerance) {
            Some(true) => {}
            Some(false) => return Err(FusionError::NotSimultaneous),
            None => {
                return Err(FusionError::ClockMismatch(ClockMismatch {
                    expected: first.timestamp().clock(),
                    found: input.timestamp().clock(),
                }));
            }
        }
        let ObservationValue::Scalar(value) = input.value() else {
            return Err(FusionError::NotScalar);
        };
        scalars.push(ScalarInput {
            value: *value,
            confidence: input.confidence().get(),
            sigma: input
                .uncertainty()
                .and_then(|u| u.standard_deviation)
                .map(StandardDeviation::get),
        });
    }

    let (value, sigma) = strategy
        .combine(&scalars)
        .ok_or(FusionError::StrategyFailed)?;
    if !value.is_finite() {
        return Err(FusionError::StrategyFailed);
    }

    // Independent-evidence rule: P(at least one is right) = 1 - Π(1 - c_i).
    let disbelief: f64 = scalars.iter().map(|s| 1.0 - s.confidence).product();
    let confidence =
        Confidence::new((1.0 - disbelief).clamp(0.0, 1.0)).ok_or(FusionError::StrategyFailed)?;

    let earliest = inputs
        .iter()
        .min_by_key(|o| o.timestamp().nanos())
        .map_or(first.timestamp(), Observation::timestamp);

    let modality = if inputs.iter().all(|o| o.modality() == first.modality()) {
        first.modality().clone()
    } else {
        Modality::Custom("fused".into())
    };

    let origin = if inputs
        .iter()
        .all(|o| o.provenance().origin() == first.provenance().origin())
    {
        first.provenance().origin()
    } else {
        Origin::Unknown
    };

    // Keep every input's full chain, then record the fusion itself.
    let mut provenance = Provenance::new(origin);
    for input in inputs {
        for step in input.provenance().chain() {
            provenance.push(step.clone());
        }
    }
    let mut fusion_algorithm = Algorithm::new(strategy.name(), strategy.version());
    for (k, v) in strategy.parameters() {
        fusion_algorithm = fusion_algorithm.with_parameter(k, v);
    }
    fusion_algorithm = fusion_algorithm
        .with_parameter(
            "time_tolerance_ns",
            alloc::format!("{}", config.time_tolerance.as_nanos()),
        )
        .with_parameter("confidence_rule", "noisy-or (independence assumed)");
    provenance.push(ProvenanceStep::Algorithm(fusion_algorithm));

    let mut observation = Observation::new(
        id,
        modality,
        Property::new(first.property().as_str()),
        ObservationValue::Scalar(value),
        confidence,
        earliest,
        provenance,
    );
    if let Some(sd) = sigma {
        let sd = StandardDeviation::new(sd).ok_or(FusionError::StrategyFailed)?;
        observation = observation.with_uncertainty(Uncertainty {
            standard_deviation: Some(sd),
            ..Uncertainty::default()
        });
    }
    observation
        .advance_to(ObservationState::Fused)
        .map_err(|_| FusionError::StrategyFailed)?;

    let disagreement = disagreement(inputs, &scalars, config.disagreement_threshold);

    Ok(FusedObservation {
        observation,
        disagreement,
    })
}

/// Probabilities are kept inside `[EPS, 1 - EPS]` so that odds stay finite.
const EPS: f64 = 1e-6;

/// # Examples
///
/// ```
/// use tpt_senses::prelude::*;
/// use tpt_senses::{Duration, FusionConfig, fuse_presence};
///
/// let at = Timestamp::new(0, ClockSource::Monotonic);
/// let detect = |id, modality, c| {
///     Observation::new(
///         ObservationId::new(id),
///         modality,
///         Property::new("person_present"),
///         ObservationValue::Boolean(true),
///         Confidence::new(c).unwrap(),
///         at,
///         Provenance::new(Origin::Real).with_step(ProvenanceStep::Sensor(SensorId::new(id))),
///     )
/// };
/// let config = FusionConfig {
///     time_tolerance: Duration::from_millis(5),
///     disagreement_threshold: 0.5,
/// };
/// let fused = fuse_presence(
///     &[detect(1, Modality::Visual, 0.7), detect(2, Modality::Thermal, 0.6)],
///     ObservationId::new(9),
///     &config,
/// )
/// .unwrap();
/// assert_eq!(fused.observation.value(), &ObservationValue::Boolean(true));
/// assert!(fused.observation.confidence().get() > 0.7);
/// ```
/// Fuses boolean detections of one property from any number of modalities (spec §38).
///
/// Each input says "true" or "false" with a confidence. The evidence is pooled by multiplying
/// odds, which assumes the inputs are independent and that the prior is even (50/50). That
/// assumption is recorded in the fused provenance. The result is the more probable value, with
/// the probability of that value as its confidence.
///
/// Inputs must describe the same property, be boolean, and be simultaneous within
/// `config.time_tolerance`. Their modalities may differ.
///
/// # Errors
///
/// Returns [`FusionError`] for empty input, mismatched properties, non-boolean values, inputs
/// that are not simultaneous, or mixed clocks.
pub fn fuse_presence(
    inputs: &[Observation],
    id: ObservationId,
    config: &FusionConfig,
) -> Result<FusedObservation, FusionError> {
    let first = inputs.first().ok_or(FusionError::Empty)?;

    let mut scalars = Vec::with_capacity(inputs.len());
    let mut odds = 1.0_f64;
    for input in inputs {
        if input.property() != first.property() {
            return Err(FusionError::PropertyMismatch);
        }
        match simultaneous(first.timestamp(), input.timestamp(), config.time_tolerance) {
            Some(true) => {}
            Some(false) => return Err(FusionError::NotSimultaneous),
            None => {
                return Err(FusionError::ClockMismatch(ClockMismatch {
                    expected: first.timestamp().clock(),
                    found: input.timestamp().clock(),
                }));
            }
        }
        let ObservationValue::Boolean(claim) = input.value() else {
            return Err(FusionError::NotScalar);
        };
        let c = input.confidence().get();
        // Probability that the input's claim is the true state.
        let p_true = if *claim { c } else { 1.0 - c };
        let p = p_true.clamp(EPS, 1.0 - EPS);
        odds *= p / (1.0 - p);
        scalars.push(ScalarInput {
            value: if *claim { 1.0 } else { 0.0 },
            confidence: c,
            sigma: None,
        });
    }

    // Posterior probability that the property is true, from the pooled odds.
    let p_true = if odds.is_finite() {
        odds / (1.0 + odds)
    } else {
        1.0
    };
    let claim = p_true >= 0.5;
    let reported = if claim { p_true } else { 1.0 - p_true };
    let confidence =
        Confidence::new(reported.clamp(0.0, 1.0)).ok_or(FusionError::StrategyFailed)?;

    let earliest = inputs
        .iter()
        .min_by_key(|o| o.timestamp().nanos())
        .map_or(first.timestamp(), Observation::timestamp);

    let modality = if inputs.iter().all(|o| o.modality() == first.modality()) {
        first.modality().clone()
    } else {
        Modality::Custom("multimodal".into())
    };

    let mut provenance = Provenance::new(Origin::Unknown);
    for input in inputs {
        for step in input.provenance().chain() {
            provenance.push(step.clone());
        }
    }
    provenance.push(ProvenanceStep::Algorithm(
        Algorithm::new("presence-odds-pooling", "1")
            .with_parameter("assumes", "independent evidence, even prior")
            .with_parameter(
                "time_tolerance_ns",
                alloc::format!("{}", config.time_tolerance.as_nanos()),
            ),
    ));

    let mut observation = Observation::new(
        id,
        modality,
        Property::new(first.property().as_str()),
        ObservationValue::Boolean(claim),
        confidence,
        earliest,
        provenance,
    );
    observation
        .advance_to(ObservationState::Fused)
        .map_err(|_| FusionError::StrategyFailed)?;

    let disagreement = disagreement(inputs, &scalars, config.disagreement_threshold);
    Ok(FusedObservation {
        observation,
        disagreement,
    })
}

fn disagreement(
    inputs: &[Observation],
    scalars: &[ScalarInput],
    threshold: f64,
) -> Option<Disagreement> {
    let max = scalars
        .iter()
        .map(|s| s.value)
        .fold(f64::NEG_INFINITY, f64::max);
    let min = scalars
        .iter()
        .map(|s| s.value)
        .fold(f64::INFINITY, f64::min);
    let spread = max - min;
    if spread <= threshold {
        return None;
    }
    // The median is the reference for outliers, so a single faulty sensor cannot drag the
    // reference towards itself the way it would with the mean.
    let mut sorted: Vec<f64> = scalars.iter().map(|s| s.value).collect();
    sorted.sort_by(|x, y| x.partial_cmp(y).unwrap_or(core::cmp::Ordering::Equal));
    let mid = sorted.len() / 2;
    let median = if sorted.len() % 2 == 0 {
        f64::midpoint(sorted[mid - 1], sorted[mid])
    } else {
        sorted[mid]
    };
    let outlier_sensors = inputs
        .iter()
        .zip(scalars)
        .filter(|(_, s)| (s.value - median).abs() > threshold)
        .filter_map(|(o, _)| o.provenance().sensors().next())
        .collect();
    Some(Disagreement {
        spread,
        outlier_sensors,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ClockSource, Timestamp};
    use alloc::vec;

    fn t(ns: u64) -> Timestamp {
        Timestamp::new(ns, ClockSource::Monotonic)
    }

    fn reading(
        sensor: u64,
        value: f64,
        conf: f64,
        sigma: Option<f64>,
        ts: Timestamp,
    ) -> Observation {
        let mut o = Observation::new(
            ObservationId::new(sensor),
            Modality::Depth,
            Property::new("range_m"),
            ObservationValue::Scalar(value),
            Confidence::new(conf).unwrap(),
            ts,
            Provenance::new(Origin::Real).with_step(ProvenanceStep::Sensor(SensorId::new(sensor))),
        );
        if let Some(sd) = sigma.and_then(StandardDeviation::new) {
            o = o.with_uncertainty(Uncertainty::new().with_standard_deviation(sd));
        }
        o
    }

    fn config(threshold: f64) -> FusionConfig {
        FusionConfig {
            time_tolerance: Duration::from_millis(1),
            disagreement_threshold: threshold,
        }
    }

    #[test]
    fn inverse_variance_favours_precise_sensor() {
        let a = reading(1, 10.0, 0.9, Some(0.1), t(0));
        let b = reading(2, 12.0, 0.9, Some(10.0), t(0));
        let fused = fuse(
            &[a, b],
            ObservationId::new(100),
            &InverseVariance,
            &config(5.0),
        )
        .unwrap();
        let ObservationValue::Scalar(v) = fused.observation.value() else {
            panic!()
        };
        assert!((v - 10.0).abs() < 0.01);
        let sd = fused
            .observation
            .uncertainty()
            .unwrap()
            .standard_deviation
            .unwrap()
            .get();
        assert!(sd <= 0.1 + 1e-9);
    }

    #[test]
    fn fused_provenance_keeps_every_input_sensor() {
        let a = reading(1, 10.0, 0.8, None, t(0));
        let b = reading(2, 10.2, 0.7, None, t(0));
        let fused = fuse(
            &[a, b],
            ObservationId::new(101),
            &ConfidenceWeighted,
            &config(5.0),
        )
        .unwrap();
        let sensors: Vec<_> = fused.observation.provenance().sensors().collect();
        assert_eq!(sensors, vec![SensorId::new(1), SensorId::new(2)]);
        assert_eq!(fused.observation.provenance().algorithms().count(), 1);
        assert_eq!(fused.observation.state(), ObservationState::Fused);
    }

    #[test]
    fn combined_confidence_is_noisy_or() {
        let a = reading(1, 5.0, 0.5, None, t(0));
        let b = reading(2, 5.0, 0.5, None, t(0));
        let fused = fuse(
            &[a, b],
            ObservationId::new(102),
            &ConfidenceWeighted,
            &config(5.0),
        )
        .unwrap();
        assert!((fused.observation.confidence().get() - 0.75).abs() < 1e-12);
    }

    #[test]
    fn asynchronous_inputs_are_refused() {
        let a = reading(1, 5.0, 0.5, None, t(0));
        let b = reading(2, 5.0, 0.5, None, t(10_000_000)); // 10 ms apart, tolerance 1 ms
        assert_eq!(
            fuse(
                &[a, b],
                ObservationId::new(103),
                &ConfidenceWeighted,
                &config(5.0)
            )
            .unwrap_err(),
            FusionError::NotSimultaneous
        );
    }

    #[test]
    fn mixed_clocks_are_refused_not_assumed_simultaneous() {
        let a = reading(1, 5.0, 0.5, None, t(0));
        let b = reading(2, 5.0, 0.5, None, Timestamp::new(0, ClockSource::Wall));
        assert!(matches!(
            fuse(
                &[a, b],
                ObservationId::new(104),
                &ConfidenceWeighted,
                &config(5.0)
            ),
            Err(FusionError::ClockMismatch(_))
        ));
    }

    #[test]
    fn disagreement_is_reported_with_outliers() {
        let a = reading(1, 10.0, 0.9, None, t(0));
        let b = reading(2, 10.1, 0.9, None, t(0));
        let c = reading(3, 20.0, 0.9, None, t(0));
        let fused = fuse(
            &[a, b, c],
            ObservationId::new(105),
            &ConfidenceWeighted,
            &config(2.0),
        )
        .unwrap();
        let d = fused.disagreement.expect("should disagree");
        assert!((d.spread - 10.0).abs() < 1e-9);
        assert_eq!(d.outlier_sensors, vec![SensorId::new(3)]);
    }

    #[test]
    fn no_disagreement_when_inputs_agree() {
        let a = reading(1, 10.0, 0.9, None, t(0));
        let b = reading(2, 10.05, 0.9, None, t(0));
        let fused = fuse(
            &[a, b],
            ObservationId::new(106),
            &ConfidenceWeighted,
            &config(2.0),
        )
        .unwrap();
        assert!(fused.disagreement.is_none());
    }

    #[test]
    fn inverse_variance_needs_uncertainty() {
        let a = reading(1, 10.0, 0.9, None, t(0));
        assert_eq!(
            fuse(
                &[a],
                ObservationId::new(107),
                &InverseVariance,
                &config(5.0)
            )
            .unwrap_err(),
            FusionError::StrategyFailed
        );
    }

    struct Median;
    impl FusionStrategy for Median {
        fn name(&self) -> &'static str {
            "median"
        }
        fn combine(&self, inputs: &[ScalarInput]) -> Option<(f64, Option<f64>)> {
            let mut v: Vec<f64> = inputs.iter().map(|i| i.value).collect();
            v.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let mid = v.len() / 2;
            let m = if v.len() % 2 == 0 {
                f64::midpoint(v[mid - 1], v[mid])
            } else {
                v[mid]
            };
            Some((m, None))
        }
    }

    #[test]
    fn custom_strategy_is_pluggable_and_recorded() {
        let inputs = [
            reading(1, 1.0, 0.5, None, t(0)),
            reading(2, 2.0, 0.5, None, t(0)),
            reading(3, 100.0, 0.5, None, t(0)),
        ];
        let fused = fuse(&inputs, ObservationId::new(108), &Median, &config(1000.0)).unwrap();
        assert_eq!(fused.observation.value(), &ObservationValue::Scalar(2.0));
        let algo = fused.observation.provenance().algorithms().last().unwrap();
        assert_eq!(algo.name, "median");
    }

    #[test]
    fn empty_and_mismatched_inputs_are_refused() {
        assert_eq!(
            fuse(
                &[],
                ObservationId::new(109),
                &ConfidenceWeighted,
                &config(1.0)
            )
            .unwrap_err(),
            FusionError::Empty
        );
        let a = reading(1, 1.0, 0.5, None, t(0));
        let mut b = reading(2, 1.0, 0.5, None, t(0));
        b = Observation::new(
            b.id(),
            Modality::Depth,
            Property::new("other"),
            ObservationValue::Scalar(1.0),
            b.confidence(),
            t(0),
            Provenance::new(Origin::Real),
        );
        assert_eq!(
            fuse(
                &[a, b],
                ObservationId::new(110),
                &ConfidenceWeighted,
                &config(1.0)
            )
            .unwrap_err(),
            FusionError::PropertyMismatch
        );
    }
}
