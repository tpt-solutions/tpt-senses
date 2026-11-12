//! World state (spec §22). State is derived from observations and is never an independent source
//! of truth: the only constructor is [`WorldState::derive`], and its fields are read-only.

use alloc::vec::Vec;

use crate::internal::InternalMetric;
use crate::{
    Confidence, EntityAssociation, EntityId, EntityKind, Modality, Observation, ObservationId,
    Property, SpatialReference,
};

/// A view of one entity as currently believed.
#[derive(Clone, Debug, PartialEq)]
pub struct EntityView {
    entity: EntityId,
    kind: EntityKind,
    confidence: Confidence,
    members: Vec<ObservationId>,
    modalities: Vec<Modality>,
}

impl EntityView {
    /// The entity identifier.
    #[must_use]
    pub const fn entity(&self) -> EntityId {
        self.entity
    }

    /// The believed kind.
    #[must_use]
    pub const fn kind(&self) -> &EntityKind {
        &self.kind
    }

    /// Confidence in the association.
    #[must_use]
    pub const fn confidence(&self) -> Confidence {
        self.confidence
    }

    /// The supporting observations.
    #[must_use]
    pub fn members(&self) -> &[ObservationId] {
        &self.members
    }

    /// The modalities that contributed.
    #[must_use]
    pub fn modalities(&self) -> &[Modality] {
        &self.modalities
    }
}

/// The system's current picture of the world, its body and its surroundings.
#[derive(Clone, Debug, PartialEq)]
pub struct WorldState {
    robot_pose: Option<SpatialReference>,
    entities: Vec<EntityView>,
    environment: Vec<Observation>,
    body: Vec<Observation>,
}

impl WorldState {
    /// Derives the world state from observations and their entity associations.
    ///
    /// - Entities come from `associations`.
    /// - `robot_pose` is the located observation with property `robot_pose`, if any.
    /// - Environment is every observation of modality [`Modality::Environmental`].
    /// - Body is every observation whose property names an [`InternalMetric`].
    ///
    /// Meta-signals are not part of the world state. They are for cognition.
    #[must_use]
    pub fn derive(observations: &[Observation], associations: &[EntityAssociation]) -> Self {
        let entities = associations
            .iter()
            .map(|a| EntityView {
                entity: a.entity,
                kind: a.kind.clone(),
                confidence: a.confidence,
                members: a.members.clone(),
                modalities: a.modalities.clone(),
            })
            .collect();

        let pose_property = Property::new("robot_pose");
        let robot_pose = observations
            .iter()
            .filter(|o| o.property() == &pose_property)
            .find_map(|o| o.location().cloned());

        let environment = observations
            .iter()
            .filter(|o| o.modality() == &Modality::Environmental)
            .cloned()
            .collect();

        let body = observations
            .iter()
            .filter(|o| InternalMetric::from_property(o.property()).is_some())
            .cloned()
            .collect();

        Self {
            robot_pose,
            entities,
            environment,
            body,
        }
    }

    /// The robot's pose, if observed.
    #[must_use]
    pub const fn robot_pose(&self) -> Option<&SpatialReference> {
        self.robot_pose.as_ref()
    }

    /// Entities currently believed to exist.
    #[must_use]
    pub fn entities(&self) -> &[EntityView] {
        &self.entities
    }

    /// Environmental observations.
    #[must_use]
    pub fn environment(&self) -> &[Observation] {
        &self.environment
    }

    /// Internal (body) observations.
    #[must_use]
    pub fn body(&self) -> &[Observation] {
        &self.body
    }
}
