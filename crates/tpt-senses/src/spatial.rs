//! Spatial references and coordinate frames (spec §18).
//!
//! `tpt-senses` does not implement a transform library. It defines the frame vocabulary and a
//! [`Transform`] trait, so that external systems (for example ROS TF) can plug in without a hard
//! dependency.

use alloc::borrow::Cow;

/// A point in 3D space. Two-dimensional frames (such as [`CoordinateFrame::Pixel`]) use `z = 0`.
pub type Point3 = [f64; 3];

/// The frame a position is expressed in.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum CoordinateFrame {
    /// Fixed, global frame.
    World,
    /// Frame attached to the robot base.
    Robot,
    /// Frame attached to the body of a robot or agent.
    Body,
    /// Frame of a camera or other imaging sensor.
    Camera,
    /// Image pixel coordinates (`x`, `y`, `0`).
    Pixel,
    /// Any other frame, named by the integrator.
    Custom(Cow<'static, str>),
}

/// A position together with the frame it is expressed in.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct SpatialReference {
    /// The frame of `position`.
    pub frame: CoordinateFrame,
    /// The position, if known. A reference may name a frame without a position.
    pub position: Option<Point3>,
}

impl SpatialReference {
    /// A reference to a known position in `frame`.
    #[must_use]
    pub const fn at(frame: CoordinateFrame, position: Point3) -> Self {
        Self {
            frame,
            position: Some(position),
        }
    }

    /// A reference naming only a frame.
    #[must_use]
    pub const fn in_frame(frame: CoordinateFrame) -> Self {
        Self {
            frame,
            position: None,
        }
    }
}

/// A conversion between two coordinate frames.
///
/// Implementations may fail (for example when the transform is not known at a given time), so
/// [`Transform::apply`] returns an `Option`.
pub trait Transform {
    /// The frame this transform takes points from.
    fn source(&self) -> &CoordinateFrame;

    /// The frame this transform takes points to.
    fn target(&self) -> &CoordinateFrame;

    /// Converts a point from [`Transform::source`] into [`Transform::target`].
    fn apply(&self, point: Point3) -> Option<Point3>;

    /// Converts a spatial reference. Returns `None` if the reference is not in the source frame
    /// or has no position.
    fn transform_reference(&self, reference: &SpatialReference) -> Option<SpatialReference> {
        if reference.frame != *self.source() {
            return None;
        }
        let position = self.apply(reference.position?)?;
        Some(SpatialReference::at(self.target().clone(), position))
    }
}

/// A pure translation between two frames.
#[derive(Clone, Debug, PartialEq)]
pub struct Translation {
    source: CoordinateFrame,
    target: CoordinateFrame,
    offset: Point3,
}

impl Translation {
    /// Creates a translation that adds `offset` to points in `source` to express them in `target`.
    #[must_use]
    pub const fn new(source: CoordinateFrame, target: CoordinateFrame, offset: Point3) -> Self {
        Self {
            source,
            target,
            offset,
        }
    }
}

impl Transform for Translation {
    fn source(&self) -> &CoordinateFrame {
        &self.source
    }

    fn target(&self) -> &CoordinateFrame {
        &self.target
    }

    fn apply(&self, p: Point3) -> Option<Point3> {
        let [x, y, z] = p;
        let [dx, dy, dz] = self.offset;
        let out = [x + dx, y + dy, z + dz];
        out.iter().all(|v| v.is_finite()).then_some(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translation_converts_reference() {
        let t = Translation::new(
            CoordinateFrame::Camera,
            CoordinateFrame::Robot,
            [0.1, 0.0, 0.5],
        );
        let r = SpatialReference::at(CoordinateFrame::Camera, [1.0, 2.0, 3.0]);
        let out = t.transform_reference(&r).unwrap();
        assert_eq!(out.frame, CoordinateFrame::Robot);
        assert_eq!(out.position, Some([1.1, 2.0, 3.5]));
    }

    #[test]
    fn wrong_source_frame_is_rejected() {
        let t = Translation::new(CoordinateFrame::Camera, CoordinateFrame::Robot, [0.0; 3]);
        let r = SpatialReference::at(CoordinateFrame::World, [0.0; 3]);
        assert!(t.transform_reference(&r).is_none());
    }

    #[test]
    fn reference_without_position_is_rejected() {
        let t = Translation::new(CoordinateFrame::Camera, CoordinateFrame::Robot, [0.0; 3]);
        let r = SpatialReference::in_frame(CoordinateFrame::Camera);
        assert!(t.transform_reference(&r).is_none());
    }

    #[test]
    fn non_finite_results_are_rejected() {
        let t = Translation::new(
            CoordinateFrame::Pixel,
            CoordinateFrame::World,
            [f64::MAX; 3],
        );
        assert!(t.apply([f64::MAX, 0.0, 0.0]).is_none());
    }
}
