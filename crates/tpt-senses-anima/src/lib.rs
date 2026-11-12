//! Adapter from `tpt-senses` observations to `tpt-anima` memory nodes (spec §35, §46).
//!
//! `tpt-anima` stores memories as [`MemoryNode`]s: a vector embedding, a payload and a sensory
//! modality. This crate builds one node per observation:
//!
//! * **Payload:** the whole observation, CBOR-encoded (`application/cbor`). Decoding it with
//!   `tpt_senses::from_cbor` gives back the same observation, so nothing is lost.
//! * **Modality:** `Visual` and `Auditory` map to their `tpt-anima` equivalents. Every other
//!   sensing modality is stored as `Abstract`, since it has no direct text, image or audio form.
//! * **Embedding:** produced by an [`Embedder`] that the caller supplies. `tpt-anima`'s own
//!   pipelines are demonstration bag-of-words and byte embedders, so this crate does not pick one.
//!
//! The node is written at full strength with `tpt-anima`'s default decay. Whether to store it
//! is up to the caller.

use tpt_anima_sse::{
    AgentId, MemoryId, MemoryNode, SensoryError, SensoryModality, VectorEmbedding,
};
use tpt_senses::{CodecError, Modality, Observation, to_cbor};

/// Produces an embedding for a payload. Supplied by the caller.
pub trait Embedder {
    /// Embeds the payload bytes.
    ///
    /// # Errors
    ///
    /// Returns [`SensoryError`] if the payload cannot be embedded.
    fn embed(&self, payload: &[u8]) -> Result<VectorEmbedding, SensoryError>;
}

/// Why an observation could not become a memory node.
#[derive(Debug)]
#[non_exhaustive]
pub enum AnimaAdapterError {
    /// The observation could not be encoded as CBOR.
    Encode(CodecError),
    /// The embedder failed.
    Embed(SensoryError),
}

impl core::fmt::Display for AnimaAdapterError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Encode(e) => write!(f, "could not encode observation: {e}"),
            Self::Embed(e) => write!(f, "could not embed observation: {e}"),
        }
    }
}

impl std::error::Error for AnimaAdapterError {}

/// The `tpt-anima` modality for a sensing modality.
#[must_use]
pub const fn sensory_modality(modality: &Modality) -> SensoryModality {
    match modality {
        Modality::Visual => SensoryModality::Visual,
        Modality::Auditory => SensoryModality::Audio,
        _ => SensoryModality::Abstract,
    }
}

/// Builds a `tpt-anima` memory node for one observation.
///
/// `memory_id` and `creator` are supplied by the caller, because `tpt-anima` owns its identifier
/// scheme.
///
/// # Errors
///
/// Returns [`AnimaAdapterError`] if the observation cannot be encoded or embedded.
pub fn observation_to_memory_node(
    observation: &Observation,
    embedder: &dyn Embedder,
    memory_id: MemoryId,
    creator: AgentId,
) -> Result<MemoryNode, AnimaAdapterError> {
    let payload = to_cbor(observation).map_err(AnimaAdapterError::Encode)?;
    let embedding = embedder.embed(&payload).map_err(AnimaAdapterError::Embed)?;
    Ok(MemoryNode::new(
        memory_id,
        embedding,
        payload,
        "application/cbor".to_owned(),
        sensory_modality(observation.modality()),
        creator,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tpt_senses::prelude::*;
    use tpt_senses::{Property, from_cbor};

    /// A fixed-size embedder for tests: one dimension, the payload length.
    struct LengthEmbedder;

    impl Embedder for LengthEmbedder {
        fn embed(&self, payload: &[u8]) -> Result<VectorEmbedding, SensoryError> {
            #[allow(clippy::cast_precision_loss, reason = "test payloads are small")]
            Ok(VectorEmbedding::new(vec![payload.len() as f32]))
        }
    }

    struct FailingEmbedder;

    impl Embedder for FailingEmbedder {
        fn embed(&self, _payload: &[u8]) -> Result<VectorEmbedding, SensoryError> {
            Err(SensoryError::IngestionFailed {
                reason: "no model".into(),
            })
        }
    }

    fn observation(modality: Modality) -> Observation {
        Observation::new(
            ObservationId::new(3),
            modality,
            Property::new("presence"),
            ObservationValue::Boolean(true),
            Confidence::new(0.8).unwrap(),
            tpt_senses::Timestamp::new(10, ClockSource::Monotonic),
            Provenance::new(Origin::Real),
        )
    }

    #[test]
    fn node_carries_cbor_payload_and_modality() {
        let observation = observation(Modality::Visual);
        let node =
            observation_to_memory_node(&observation, &LengthEmbedder, [1; 32], [2; 32]).unwrap();
        assert_eq!(node.payload_format, "application/cbor");
        assert_eq!(node.sensory_modality, SensoryModality::Visual);
        assert_eq!(node.id, [1; 32]);
        assert_eq!(node.embedding.values.len(), 1);
    }

    #[test]
    fn payload_decodes_back_to_the_same_observation() {
        let observation = observation(Modality::Auditory);
        let node =
            observation_to_memory_node(&observation, &LengthEmbedder, [1; 32], [2; 32]).unwrap();
        let decoded: Observation = from_cbor(&node.payload).unwrap();
        assert_eq!(decoded, observation);
        assert_eq!(node.sensory_modality, SensoryModality::Audio);
    }

    #[test]
    fn other_modalities_are_abstract() {
        assert_eq!(
            sensory_modality(&Modality::Thermal),
            SensoryModality::Abstract
        );
        assert_eq!(
            sensory_modality(&Modality::Custom("gravimetric".into())),
            SensoryModality::Abstract
        );
    }

    #[test]
    fn embedder_failure_is_reported() {
        let error = observation_to_memory_node(
            &observation(Modality::Visual),
            &FailingEmbedder,
            [1; 32],
            [2; 32],
        )
        .unwrap_err();
        assert!(matches!(error, AnimaAdapterError::Embed(_)));
    }
}
