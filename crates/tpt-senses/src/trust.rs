//! Security and trust metadata (spec §33).
//!
//! Sensor data may come from network devices, remote robots or third-party APIs. Every
//! observation carries trust metadata, and the default is *untrusted and unverified*. Nothing in
//! this crate promotes an observation to a fact; that decision belongs to consumers.

use alloc::string::String;

/// Identity of the source that produced the data (e.g. a device certificate fingerprint or a
/// service name). Free-form, so that different identity schemes can be represented.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SourceIdentity(String);

impl SourceIdentity {
    /// Creates a source identity.
    #[must_use]
    pub fn new(identity: impl Into<String>) -> Self {
        Self(identity.into())
    }

    /// The identity string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Whether the source proved who it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum AuthenticationStatus {
    /// Source identity verified cryptographically.
    Authenticated,
    /// No authentication was performed.
    #[default]
    Unauthenticated,
    /// Authentication was attempted and failed.
    Failed,
}

/// Whether the data is unchanged since the source produced it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum IntegrityStatus {
    /// Integrity verified (e.g. signature or checksum matched).
    Verified,
    /// No integrity check was performed.
    #[default]
    Unverified,
    /// A check was performed and the data does not match.
    Corrupted,
}

/// How much the consumer should rely on the data.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[non_exhaustive]
pub enum TrustLevel {
    /// Known to be unreliable or corrupted.
    Untrusted,
    /// Nothing is known about the source.
    #[default]
    Unknown,
    /// Source and data have been checked to a reasonable degree.
    Verified,
}

/// Trust metadata attached to an observation.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TrustMetadata {
    /// Who produced the data, if known.
    pub source: Option<SourceIdentity>,
    /// Authentication result.
    pub authentication: AuthenticationStatus,
    /// Integrity result.
    pub integrity: IntegrityStatus,
    /// Resulting trust level.
    pub level: TrustLevel,
}

impl TrustMetadata {
    /// Trust metadata for data with no checks performed: unknown source, unverified, unknown
    /// trust. This is the default for all newly created observations.
    #[must_use]
    pub const fn unverified() -> Self {
        Self {
            source: None,
            authentication: AuthenticationStatus::Unauthenticated,
            integrity: IntegrityStatus::Unverified,
            level: TrustLevel::Unknown,
        }
    }

    /// Returns `true` if the data is known to be bad: it failed authentication or integrity, or
    /// its trust level is [`TrustLevel::Untrusted`].
    #[must_use]
    pub fn is_known_bad(&self) -> bool {
        self.authentication == AuthenticationStatus::Failed
            || self.integrity == IntegrityStatus::Corrupted
            || self.level == TrustLevel::Untrusted
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_unverified_and_unknown() {
        let t = TrustMetadata::default();
        assert_eq!(t.level, TrustLevel::Unknown);
        assert_eq!(t, TrustMetadata::unverified());
        assert!(!t.is_known_bad());
    }

    #[test]
    fn failed_checks_mark_data_bad() {
        let t = TrustMetadata {
            authentication: AuthenticationStatus::Failed,
            ..TrustMetadata::unverified()
        };
        assert!(t.is_known_bad());
        let c = TrustMetadata {
            integrity: IntegrityStatus::Corrupted,
            ..Default::default()
        };
        assert!(c.is_known_bad());
    }

    #[test]
    fn trust_levels_are_ordered() {
        assert!(TrustLevel::Untrusted < TrustLevel::Unknown);
        assert!(TrustLevel::Unknown < TrustLevel::Verified);
    }
}
