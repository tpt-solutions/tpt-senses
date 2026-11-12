//! Sensory profiles (spec §24, §25).
//!
//! A profile is a named set of expected capabilities for a kind of system. Profiles describe
//! *engineering capabilities*. They are not claims about biological organisms, and no profile
//! here claims to reproduce a living sense.

use alloc::vec::Vec;

use crate::{Modality, SensorCapabilities};

/// A named set of expected sensing capabilities.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SensoryProfile {
    /// Reference profile inspired by human senses. Engineering inspiration only (§25).
    Human,
    /// A general-purpose mobile robot.
    Robot,
    /// An aerial drone.
    Drone,
    /// An agricultural robot: crop imaging, positioning and soil/environment sensing.
    AgriculturalRobot,
    /// An industrial robot: force, position and process monitoring.
    IndustrialRobot,
    /// A spacecraft: radiation, thermal and attitude sensing.
    Spacecraft,
    /// Defined entirely by the integrator.
    Custom,
}

impl SensoryProfile {
    /// The modalities a system with this profile is expected to have.
    ///
    /// `Custom` returns an empty list, because the integrator defines it.
    #[must_use]
    pub fn expected_modalities(self) -> Vec<Modality> {
        use Modality as M;
        match self {
            Self::Human => alloc::vec![
                M::Visual,
                M::Auditory,
                M::Tactile,
                M::Vestibular,
                M::Proprioceptive
            ],
            Self::Robot => alloc::vec![
                M::Visual,
                M::Depth,
                M::Lidar,
                M::Position,
                M::Orientation,
                M::Acceleration,
                M::Force,
                M::Torque
            ],
            Self::Drone => alloc::vec![
                M::Visual,
                M::Lidar,
                M::Position,
                M::Orientation,
                M::Acceleration,
                M::Pressure,
                M::Environmental
            ],
            Self::AgriculturalRobot => alloc::vec![
                M::Visual,
                M::Multispectral,
                M::Thermal,
                M::Lidar,
                M::Position,
                M::Orientation,
                M::Environmental,
                M::Chemical,
            ],
            Self::IndustrialRobot => alloc::vec![
                M::Force,
                M::Torque,
                M::Position,
                M::Vibration,
                M::Thermal,
                M::Visual
            ],
            Self::Spacecraft => alloc::vec![
                M::Radiation,
                M::Thermal,
                M::Orientation,
                M::Position,
                M::Magnetic,
                M::Acceleration
            ],
            Self::Custom => Vec::new(),
        }
    }

    /// Capabilities for this profile, with only the expected modalities filled in.
    #[must_use]
    pub fn capabilities(self) -> SensorCapabilities {
        SensorCapabilities::new(self.expected_modalities())
    }

    /// A short statement of what this profile is and is not.
    #[must_use]
    pub const fn disclaimer(self) -> &'static str {
        match self {
            Self::Human => "Engineering reference only. Not a model of human perception.",
            Self::Custom => "Defined by the integrator.",
            _ => "A set of engineering capabilities for a system type, not a biological model.",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agricultural_profile_matches_spec_example() {
        let m = SensoryProfile::AgriculturalRobot.expected_modalities();
        for expected in [
            Modality::Multispectral,
            Modality::Thermal,
            Modality::Lidar,
            Modality::Environmental,
        ] {
            assert!(m.contains(&expected), "missing {expected}");
        }
    }

    #[test]
    fn custom_profile_is_empty_until_defined() {
        assert!(SensoryProfile::Custom.expected_modalities().is_empty());
        assert!(SensoryProfile::Human.disclaimer().contains("Not a model"));
    }

    #[test]
    fn profile_capabilities_list_their_modalities() {
        let caps = SensoryProfile::Spacecraft.capabilities();
        assert!(caps.supports(&Modality::Radiation));
        assert!(!caps.supports(&Modality::Auditory));
    }
}
