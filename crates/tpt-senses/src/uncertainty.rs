//! Confidence and uncertainty (spec §15).
//!
//! `value`, `confidence`, `precision`, `accuracy` and `uncertainty` are deliberately separate
//! types. They are not interchangeable: a sensor can be precise (low spread) yet inaccurate
//! (biased), and a confidence score is a judgement, not a measured spread.

/// A confidence score in the closed range `[0.0, 1.0]`.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(try_from = "f64", into = "f64")
)]
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
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(try_from = "f64", into = "f64")
)]
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
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(try_from = "f64", into = "f64")
)]
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
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(try_from = "BoundsWire", into = "BoundsWire")
)]
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
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Distribution {
    /// Normal (Gaussian) error.
    Gaussian,
    /// Uniform error across the bounds.
    Uniform,
    /// Distribution not known.
    Unknown,
}

/// A standard deviation: finite and non-negative. Build with [`StandardDeviation::new`].
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(try_from = "f64", into = "f64")
)]
pub struct StandardDeviation(f64);

impl StandardDeviation {
    /// Creates a standard deviation. Returns `None` unless `value` is finite and non-negative.
    #[must_use]
    pub fn new(value: f64) -> Option<Self> {
        (value.is_finite() && value >= 0.0).then_some(Self(value))
    }

    /// The standard deviation, in the measurement's unit.
    #[must_use]
    pub const fn get(self) -> f64 {
        self.0
    }
}

/// Quantified uncertainty of a measurement or observation. All parts are optional.
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(from = "UncertaintyWire", into = "UncertaintyWire")
)]
#[non_exhaustive]
pub struct Uncertainty {
    /// Standard deviation of the value. Finite and non-negative by construction.
    pub standard_deviation: Option<StandardDeviation>,
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
    /// No uncertainty information. Same as `Default::default()`.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the standard deviation (builder style).
    #[must_use]
    pub const fn with_standard_deviation(mut self, value: StandardDeviation) -> Self {
        self.standard_deviation = Some(value);
        self
    }

    /// Sets the precision (builder style).
    #[must_use]
    pub const fn with_precision(mut self, value: Precision) -> Self {
        self.precision = Some(value);
        self
    }

    /// Sets the accuracy (builder style).
    #[must_use]
    pub const fn with_accuracy(mut self, value: Accuracy) -> Self {
        self.accuracy = Some(value);
        self
    }

    /// Sets the bounds (builder style).
    #[must_use]
    pub const fn with_bounds(mut self, value: Bounds) -> Self {
        self.bounds = Some(value);
        self
    }

    /// Sets the error distribution (builder style).
    #[must_use]
    pub const fn with_distribution(mut self, value: Distribution) -> Self {
        self.distribution = Some(value);
        self
    }
}

#[cfg(feature = "serde")]
#[derive(serde::Serialize, serde::Deserialize)]
struct BoundsWire {
    lower: f64,
    upper: f64,
}

#[cfg(feature = "serde")]
#[derive(serde::Serialize, serde::Deserialize)]
struct UncertaintyWire {
    standard_deviation: Option<StandardDeviation>,
    precision: Option<Precision>,
    accuracy: Option<Accuracy>,
    bounds: Option<Bounds>,
    distribution: Option<Distribution>,
}

// Serde support. Every type with an invariant deserializes through the same validation as the
// constructors above, so untrusted input cannot produce an out-of-range value.
#[cfg(feature = "serde")]
mod serde_support {
    use super::{
        Accuracy, Bounds, BoundsWire, Confidence, Precision, StandardDeviation, Uncertainty,
        UncertaintyWire,
    };

    impl TryFrom<f64> for Confidence {
        type Error = &'static str;
        fn try_from(value: f64) -> Result<Self, Self::Error> {
            Self::new(value).ok_or("confidence must be finite and within [0, 1]")
        }
    }
    impl From<Confidence> for f64 {
        fn from(value: Confidence) -> Self {
            value.get()
        }
    }

    impl TryFrom<f64> for Precision {
        type Error = &'static str;
        fn try_from(value: f64) -> Result<Self, Self::Error> {
            Self::new(value).ok_or("precision must be finite and non-negative")
        }
    }
    impl From<Precision> for f64 {
        fn from(value: Precision) -> Self {
            value.get()
        }
    }

    impl TryFrom<f64> for Accuracy {
        type Error = &'static str;
        fn try_from(value: f64) -> Result<Self, Self::Error> {
            Self::new(value).ok_or("accuracy must be finite and non-negative")
        }
    }
    impl TryFrom<f64> for StandardDeviation {
        type Error = &'static str;
        fn try_from(value: f64) -> Result<Self, Self::Error> {
            Self::new(value).ok_or("standard deviation must be finite and non-negative")
        }
    }
    impl From<StandardDeviation> for f64 {
        fn from(value: StandardDeviation) -> Self {
            value.get()
        }
    }

    impl From<Accuracy> for f64 {
        fn from(value: Accuracy) -> Self {
            value.get()
        }
    }

    impl TryFrom<BoundsWire> for Bounds {
        type Error = &'static str;
        fn try_from(wire: BoundsWire) -> Result<Self, Self::Error> {
            Self::new(wire.lower, wire.upper).ok_or("bounds must be finite with lower <= upper")
        }
    }
    impl From<Bounds> for BoundsWire {
        fn from(bounds: Bounds) -> Self {
            Self {
                lower: bounds.lower(),
                upper: bounds.upper(),
            }
        }
    }

    impl From<UncertaintyWire> for Uncertainty {
        fn from(wire: UncertaintyWire) -> Self {
            Uncertainty {
                standard_deviation: wire.standard_deviation,
                precision: wire.precision,
                accuracy: wire.accuracy,
                bounds: wire.bounds,
                distribution: wire.distribution,
            }
        }
    }
    impl From<Uncertainty> for UncertaintyWire {
        fn from(u: Uncertainty) -> Self {
            Self {
                standard_deviation: u.standard_deviation,
                precision: u.precision,
                accuracy: u.accuracy,
                bounds: u.bounds,
                distribution: u.distribution,
            }
        }
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
    fn standard_deviation_must_be_finite_and_non_negative() {
        assert!(StandardDeviation::new(0.0).is_some());
        assert!(StandardDeviation::new(-1.0).is_none());
        assert!(StandardDeviation::new(f64::NAN).is_none());
        assert!(StandardDeviation::new(f64::INFINITY).is_none());
        assert_eq!(Uncertainty::default().standard_deviation, None);
    }
}
