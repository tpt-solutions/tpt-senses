//! Identifier types. Plain `u64` newtypes: no UUID dependency, cheap to copy and hash.

use core::fmt;

macro_rules! define_id {
    ($(#[$meta:meta])* $name:ident, $doc:literal) => {
        $(#[$meta])*
        #[doc = $doc]
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(u64);

        impl $name {
            /// Creates an identifier from a raw numeric value.
            #[must_use]
            pub const fn new(raw: u64) -> Self {
                Self(raw)
            }

            /// Returns the raw numeric value.
            #[must_use]
            pub const fn get(self) -> u64 {
                self.0
            }
        }

        impl From<u64> for $name {
            fn from(raw: u64) -> Self {
                Self(raw)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Display::fmt(&self.0, f)
            }
        }
    };
}

define_id!(
    /// Identifies a sensor (real, simulated or internal).
    SensorId,
    "Identifier of a sensor."
);

define_id!(
    /// Identifies a single measurement.
    MeasurementId,
    "Identifier of a measurement."
);

define_id!(
    /// Identifies an observation.
    ObservationId,
    "Identifier of an observation."
);

define_id!(
    /// Identifies a real-world or virtual entity (object, plant, vehicle, ...).
    EntityId,
    "Identifier of an entity."
);
