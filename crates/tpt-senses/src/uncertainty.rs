//! Confidence and uncertainty (spec §15).
//!
//! `value`, `confidence`, `precision`, `accuracy` and `uncertainty` are deliberately separate
//! types. They are not interchangeable: a sensor can be precise (low spread) yet inaccurate
//! (biased), and a confidence score is a judgement, not a measured spread.

/// A confidence score in the closed range `[0.0, 1.0]`.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Confidence(f64);

impl Confidence {
    /// No confidence at all.
    pub const ZERO: Self = Self(0.0);

    /// Full confidence.
    pub const ONE: Self = Self(1.0);

    /// Creates a confidence. Returns `None` unless `value` is finite and within `[0.0, 1.0]`.
    #[must_use]
    pub fn new(value: f64) -> Option<Self> {
        (value.is_finite() && (0.0..=1.0).contains(&value)).then_some(Self(value))
    }

    /// The confidence value.
    #[must_use]
    pub const fn get(self) -> f64 {
        self.0
    }
}

/// Repeatability of a measurement (spread under identical conditions). Non-negative, finite.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Precision(f64);

impl Precision {
    /// Creates a precision. Returns `None` unless `value` is finite and non-negative.
    #[must_use]
    pub fn new(value: f64) -> Option<Self> {
        (value.is_finite() && value >= 0.0).then_some(Self(value))
    }

    /// The precision value, in the measurement's unit.
    #[must_use]
    pub const fn get(self) -> f64 {
        self.0
    }
}

/// Closeness to the true value (absolute bound on error or bias). Non-negative, finite.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Accuracy(f64);

impl Accuracy {
    /// Creates an accuracy. Returns `None` unless `value` is finite and non-negative.
    #[must_use]
    pub fn new(value: f64) -> Option<Self> {
        (value.is_finite() && value >= 0.0).then_some(Self(value))
    }

    /// The accuracy value, in the measurement's unit.
    #[must_use]
    pub const fn get(self) -> f64 {
        self.0
    }
}

/// A closed interval `[lower, upper]`.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Bounds {
    lower: f64,
    upper: f64,
}

impl Bounds {
    /// Creates bounds. Returns `None` unless both ends are finite and `lower <= upper`.
    #[must_use]
    pub fn new(lower: f64, upper: f64) -> Option<Self> {
        (lower.is_finite() && upper.is_finite() && lower <= upper).then_some(Self { lower, upper })
    }

    /// Lower bound.
    #[must_use]
    pub const fn lower(self) -> f64 {
        self.lower
    }

    /// Upper bound.
    #[must_use]
    pub const fn upper(self) -> f64 {
        self.upper
    }

    /// Returns `true` if `value` lies within the bounds (inclusive).
    #[must_use]
    pub fn contains(self, value: f64) -> bool {
        value >= self.lower && value <= self.upper
    }
}

/// The assumed shape of an error distribution.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Distribution {
    /// Normal (Gaussian) error.
    Gaussian,
    /// Uniform error across the bounds.
    Uniform,
    /// Distribution not known.
    Unknown,
}

/// Quantified uncertainty of a measurement or observation. All parts are optional.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Uncertainty {
    /// Standard deviation of the value. Should be finite and non-negative; see [`Self::is_valid`].
    pub standard_deviation: Option<f64>,
    /// Repeatability.
    pub precision: Option<Precision>,
    /// Closeness to truth.
    pub accuracy: Option<Accuracy>,
    /// Interval the true value is believed to lie in.
    pub bounds: Option<Bounds>,
    /// Assumed error distribution.
    pub distribution: Option<Distribution>,
}

impl Uncertainty {
    /// Returns `true` if every present numeric field is finite and non-negative.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.standard_deviation
            .is_none_or(|sd| sd.is_finite() && sd >= 0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confidence_is_validated() {
        assert!(Confidence::new(0.0).is_some());
        assert!(Confidence::new(1.0).is_some());
        assert!(Confidence::new(1.01).is_none());
        assert!(Confidence::new(-0.1).is_none());
        assert!(Confidence::new(f64::NAN).is_none());
    }

    #[test]
    fn bounds_are_ordered_and_finite() {
        let b = Bounds::new(1.0, 2.0).unwrap();
        assert!(b.contains(1.5));
        assert!(!b.contains(2.5));
        assert!(Bounds::new(2.0, 1.0).is_none());
        assert!(Bounds::new(f64::NEG_INFINITY, 0.0).is_none());
    }

    #[test]
    fn precision_and_accuracy_are_non_negative() {
        assert!(Precision::new(0.0).is_some());
        assert!(Precision::new(-0.5).is_none());
        assert!(Accuracy::new(f64::INFINITY).is_none());
    }

    #[test]
    fn uncertainty_validity() {
        let ok = Uncertainty {
            standard_deviation: Some(0.2),
            ..Default::default()
        };
        let bad = Uncertainty {
            standard_deviation: Some(-1.0),
            ..Default::default()
        };
        assert!(ok.is_valid());
        assert!(!bad.is_valid());
        assert!(Uncertainty::default().is_valid());
    }
}
