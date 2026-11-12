//! CBOR encoding of the serde-enabled model (feature `cbor`), via `cbor4ii` (MIT).
//!
//! Decoding goes through the same validated `Deserialize` implementations as JSON, so a CBOR
//! document cannot create a value that the constructors would reject.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt;

use serde::Serialize;
use serde::de::DeserializeOwned;

/// A CBOR encoding or decoding failure. The message is for diagnostics only.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodecError(String);

impl CodecError {
    /// Human-readable description of the failure.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CBOR codec error: {}", self.0)
    }
}

#[cfg(feature = "std")]
impl std::error::Error for CodecError {}

/// # Examples
///
/// ```
/// use tpt_senses::{Confidence, from_cbor, to_cbor};
///
/// let c = Confidence::new(0.25).unwrap();
/// let bytes = to_cbor(&c).unwrap();
/// let back: Confidence = from_cbor(&bytes).unwrap();
/// assert_eq!(back, c);
/// ```
/// Encodes a value as CBOR.
///
/// # Errors
///
/// Returns [`CodecError`] if the value cannot be serialized, or if memory for the output cannot
/// be reserved.
pub fn to_cbor<T: Serialize>(value: &T) -> Result<Vec<u8>, CodecError> {
    cbor4ii::serde::to_vec(Vec::new(), value).map_err(|e| CodecError(e.to_string()))
}

/// Decodes a value from CBOR, applying the same validation as the in-memory constructors.
///
/// # Errors
///
/// Returns [`CodecError`] if the bytes are not valid CBOR for `T`, or if they describe a value
/// that fails validation.
pub fn from_cbor<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, CodecError> {
    cbor4ii::serde::from_slice(bytes).map_err(|e| CodecError(e.to_string()))
}
