//! Extensible modality model (spec §11).

use alloc::borrow::Cow;
use core::fmt;

/// The kind of quantity a sensor observes.
///
/// The enum is intentionally open-ended: [`Modality::Custom`] allows modalities that do not
/// exist yet (for example a gravitational or novel chemical signature) without changing the core.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Modality {
    /// Light-based imaging.
    Visual,
    /// Sound.
    Auditory,
    /// Touch.
    Tactile,
    /// Smell.
    Olfactory,
    /// Taste.
    Gustatory,
    /// Body configuration (joint positions etc.).
    Proprioceptive,
    /// Balance and acceleration of the body.
    Vestibular,
    /// Temperature / heat.
    Thermal,
    /// Pressure.
    Pressure,
    /// Force.
    Force,
    /// Torque.
    Torque,
    /// Vibration.
    Vibration,
    /// Range / depth.
    Depth,
    /// Laser ranging.
    Lidar,
    /// Radio-frequency ranging.
    Radar,
    /// Infrared light.
    Infrared,
    /// Ultraviolet light.
    Ultraviolet,
    /// A small number of spectral bands.
    Multispectral,
    /// Many spectral bands.
    Hyperspectral,
    /// Chemical composition.
    Chemical,
    /// Gas concentration.
    Gas,
    /// Ionising or non-ionising radiation.
    Radiation,
    /// Magnetic field.
    Magnetic,
    /// Electric field.
    Electric,
    /// Position.
    Position,
    /// Orientation.
    Orientation,
    /// Acceleration.
    Acceleration,
    /// Velocity.
    Velocity,
    /// Environmental conditions (humidity, light level, ...).
    Environmental,
    /// Biological signals.
    Biological,
    /// Any modality not covered above.
    Custom(Cow<'static, str>),
}

impl Modality {
    /// A short, stable, lowercase name for this modality.
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            Self::Visual => "visual",
            Self::Auditory => "auditory",
            Self::Tactile => "tactile",
            Self::Olfactory => "olfactory",
            Self::Gustatory => "gustatory",
            Self::Proprioceptive => "proprioceptive",
            Self::Vestibular => "vestibular",
            Self::Thermal => "thermal",
            Self::Pressure => "pressure",
            Self::Force => "force",
            Self::Torque => "torque",
            Self::Vibration => "vibration",
            Self::Depth => "depth",
            Self::Lidar => "lidar",
            Self::Radar => "radar",
            Self::Infrared => "infrared",
            Self::Ultraviolet => "ultraviolet",
            Self::Multispectral => "multispectral",
            Self::Hyperspectral => "hyperspectral",
            Self::Chemical => "chemical",
            Self::Gas => "gas",
            Self::Radiation => "radiation",
            Self::Magnetic => "magnetic",
            Self::Electric => "electric",
            Self::Position => "position",
            Self::Orientation => "orientation",
            Self::Acceleration => "acceleration",
            Self::Velocity => "velocity",
            Self::Environmental => "environmental",
            Self::Biological => "biological",
            Self::Custom(name) => name,
        }
    }

    /// Returns `true` for [`Modality::Custom`].
    #[must_use]
    pub const fn is_custom(&self) -> bool {
        matches!(self, Self::Custom(_))
    }
}

impl fmt::Display for Modality {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;

    #[test]
    fn custom_modality_is_supported() {
        let m = Modality::Custom("gravitational".into());
        assert!(m.is_custom());
        assert_eq!(m.name(), "gravitational");
        assert_ne!(m, Modality::Thermal);
    }

    #[test]
    fn builtin_names_are_stable() {
        assert_eq!(Modality::Lidar.to_string(), "lidar");
        assert!(!Modality::Visual.is_custom());
    }
}
