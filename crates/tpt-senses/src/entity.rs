//! Entity association (spec §20, §38): many observations from many sensors → one entity.
//!
//! Association works on observations that carry a position in a known frame. Observations are
//! grouped greedily: an observation joins a group when it is simultaneous with the group's first
//! member and within `max_distance` of that member's position. Observations in different frames
//! are never compared, because that would need a transform the caller has not supplied.

use alloc::vec::Vec;

use crate::{
    Algorithm, Confidence, CoordinateFrame, Duration, EntityId, Modality, Observation,
    ObservationId, Point3, Provenance, ProvenanceStep, simultaneous,
};

/// The kind of entity an association refers to.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum EntityKind {
    /// A physical object.
    Object,
    /// A person.
    Person,
    /// An animal.
    Animal,
    /// A plant.
    Plant,
    /// A vehicle.
    Vehicle,
    /// A robot (including other robots).
    Robot,
    /// Any other kind.
    Other(alloc::borrow::Cow<'static, str>),
}

/// The evidence that a set of observations belongs to one entity.
#[derive(Clone, Debug, PartialEq)]
pub struct EntityAssociation {
    /// The entity the evidence is associated with.
    pub entity: EntityId,
    /// What kind of entity it is believed to be.
    pub kind: EntityKind,
    /// Confidence in the association, combined from the members (noisy-OR, independence assumed).
    pub confidence: Confidence,
    /// The observations that support the association.
    pub members: Vec<ObservationId>,
    /// The modalities that contributed. More than one means multimodal evidence (§38).
    pub modalities: Vec<Modality>,
    /// Union of the members' provenance, plus the association step.
    pub provenance: Provenance,
}

impl EntityAssociation {
    /// The distinct sensors that contributed, in order of first appearance.
    #[must_use]
    pub fn sensors(&self) -> Vec<crate::SensorId> {
        let mut out = Vec::new();
        for s in self.provenance.sensors() {
            if !out.contains(&s) {
                out.push(s);
            }
        }
        out
    }
}

/// Settings for [`associate`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AssociationConfig {
    /// Maximum straight-line distance between a member and its group's first member.
    pub max_distance: f64,
    /// Maximum time gap between members of one group.
    pub time_tolerance: Duration,
}

fn position_of(observation: &Observation) -> Option<(&CoordinateFrame, Point3)> {
    let location = observation.location()?;
    Some((&location.frame, location.position?))
}

fn distance(a: Point3, b: Point3) -> f64 {
    let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    crate::math::sqrt(d[0] * d[0] + d[1] * d[1] + d[2] * d[2])
}

/// Groups observations into entities. Each group receives the next identifier from `first_id`.
///
/// Observations without a position are ignored.
#[must_use]
pub fn associate(
    observations: &[Observation],
    kind: &EntityKind,
    first_id: EntityId,
    config: &AssociationConfig,
) -> Vec<EntityAssociation> {
    struct Group<'a> {
        anchor: &'a Observation,
        anchor_frame: &'a CoordinateFrame,
        anchor_pos: Point3,
        members: Vec<&'a Observation>,
    }

    let mut groups: Vec<Group<'_>> = Vec::new();
    for obs in observations {
        let Some((frame, pos)) = position_of(obs) else {
            continue;
        };
        let home = groups.iter_mut().find(|g| {
            g.anchor_frame == frame
                && distance(g.anchor_pos, pos) <= config.max_distance
                && simultaneous(g.anchor.timestamp(), obs.timestamp(), config.time_tolerance)
                    == Some(true)
        });
        match home {
            Some(group) => group.members.push(obs),
            None => groups.push(Group {
                anchor: obs,
                anchor_frame: frame,
                anchor_pos: pos,
                members: alloc::vec![obs],
            }),
        }
    }

    groups
        .into_iter()
        .enumerate()
        .map(|(i, group)| {
            let mut modalities: Vec<Modality> = Vec::new();
            let first_origin = group.members[0].provenance().origin();
            let origin = if group
                .members
                .iter()
                .all(|m| m.provenance().origin() == first_origin)
            {
                first_origin
            } else {
                crate::Origin::Unknown
            };
            let mut provenance = Provenance::new(origin);
            for m in &group.members {
                if !modalities.contains(m.modality()) {
                    modalities.push(m.modality().clone());
                }
                for step in m.provenance().chain() {
                    provenance.push(step.clone());
                }
            }
            provenance.push(ProvenanceStep::Algorithm(
                Algorithm::new("entity-association", "1")
                    .with_parameter("max_distance", alloc::format!("{}", config.max_distance))
                    .with_parameter(
                        "time_tolerance_ns",
                        alloc::format!("{}", config.time_tolerance.as_nanos()),
                    ),
            ));
            let disbelief: f64 = group
                .members
                .iter()
                .map(|m| 1.0 - m.confidence().get())
                .product();
            let confidence =
                Confidence::new((1.0 - disbelief).clamp(0.0, 1.0)).unwrap_or(Confidence::ZERO);
            EntityAssociation {
                entity: EntityId::new(first_id.get() + i as u64),
                kind: kind.clone(),
                confidence,
                members: group.members.iter().map(|m| m.id()).collect(),
                modalities,
                provenance,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ClockSource, ObservationValue, Origin, Property, SensorId, SpatialReference, Timestamp,
    };

    fn t(ns: u64) -> Timestamp {
        Timestamp::new(ns, ClockSource::Monotonic)
    }

    fn at(
        id: u64,
        sensor: u64,
        modality: Modality,
        frame: CoordinateFrame,
        pos: Point3,
        ts: Timestamp,
    ) -> Observation {
        Observation::new(
            ObservationId::new(id),
            modality,
            Property::new("position"),
            ObservationValue::Vector(alloc::vec![pos[0], pos[1], pos[2]]),
            Confidence::new(0.8).unwrap(),
            ts,
            Provenance::new(Origin::Real).with_step(ProvenanceStep::Sensor(SensorId::new(sensor))),
        )
        .with_location(SpatialReference::at(frame, pos))
    }

    fn config() -> AssociationConfig {
        AssociationConfig {
            max_distance: 1.0,
            time_tolerance: Duration::from_millis(10),
        }
    }

    #[test]
    fn nearby_observations_form_one_multimodal_entity() {
        let obs = [
            at(
                1,
                1,
                Modality::Visual,
                CoordinateFrame::World,
                [10.0, 0.0, 0.0],
                t(0),
            ),
            at(
                2,
                2,
                Modality::Lidar,
                CoordinateFrame::World,
                [10.2, 0.1, 0.0],
                t(1_000),
            ),
            at(
                3,
                3,
                Modality::Radar,
                CoordinateFrame::World,
                [9.9, -0.1, 0.0],
                t(2_000),
            ),
            at(
                4,
                4,
                Modality::Position,
                CoordinateFrame::World,
                [10.1, 0.0, 0.1],
                t(3_000),
            ),
        ];
        let out = associate(&obs, &EntityKind::Vehicle, EntityId::new(27), &config());
        assert_eq!(out.len(), 1);
        let v = &out[0];
        assert_eq!(v.entity, EntityId::new(27));
        assert_eq!(v.members.len(), 4);
        assert_eq!(v.modalities.len(), 4);
        assert_eq!(v.sensors().len(), 4);
        assert!(v.confidence.get() > 0.99);
    }

    #[test]
    fn distant_observations_form_separate_entities() {
        let obs = [
            at(
                1,
                1,
                Modality::Visual,
                CoordinateFrame::World,
                [0.0, 0.0, 0.0],
                t(0),
            ),
            at(
                2,
                2,
                Modality::Visual,
                CoordinateFrame::World,
                [50.0, 0.0, 0.0],
                t(0),
            ),
        ];
        let out = associate(&obs, &EntityKind::Object, EntityId::new(1), &config());
        assert_eq!(out.len(), 2);
        assert_eq!(out[1].entity, EntityId::new(2));
    }

    #[test]
    fn different_frames_are_never_associated() {
        let obs = [
            at(
                1,
                1,
                Modality::Visual,
                CoordinateFrame::Camera,
                [0.0, 0.0, 0.0],
                t(0),
            ),
            at(
                2,
                2,
                Modality::Lidar,
                CoordinateFrame::World,
                [0.0, 0.0, 0.0],
                t(0),
            ),
        ];
        assert_eq!(
            associate(&obs, &EntityKind::Object, EntityId::new(1), &config()).len(),
            2
        );
    }

    #[test]
    fn asynchronous_observations_are_not_grouped() {
        let obs = [
            at(
                1,
                1,
                Modality::Visual,
                CoordinateFrame::World,
                [0.0, 0.0, 0.0],
                t(0),
            ),
            at(
                2,
                2,
                Modality::Lidar,
                CoordinateFrame::World,
                [0.0, 0.0, 0.0],
                t(1_000_000_000),
            ),
        ];
        assert_eq!(
            associate(&obs, &EntityKind::Object, EntityId::new(1), &config()).len(),
            2
        );
    }

    #[test]
    fn observations_without_position_are_ignored() {
        let no_pos = Observation::new(
            ObservationId::new(9),
            Modality::Chemical,
            Property::new("gas"),
            ObservationValue::Scalar(1.0),
            Confidence::new(0.5).unwrap(),
            t(0),
            Provenance::new(Origin::Real),
        );
        assert!(associate(&[no_pos], &EntityKind::Object, EntityId::new(1), &config()).is_empty());
    }
}
