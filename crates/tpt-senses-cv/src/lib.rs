//! Adapter from [`tpt-cv`](https://github.com/tpt-solutions/tpt-cv) to `tpt-senses`.
//!
//! `tpt-cv` produces geometry: keypoints, optical-flow tracks and camera poses. This crate turns
//! that output into [`Observation`]s with a confidence, provenance and a frame, and leaves
//! interpretation to the layers above.
//!
//! Confidence is a policy choice, not something `tpt-cv` reports. It comes from [`CvConfig`],
//! so the integrator decides how much each kind of measurement is worth.

use tpt_cv_feature::{KeyPoint, Point, TrackResult};
use tpt_cv_geo::pnp::Pose;
use tpt_senses::prelude::*;
use tpt_senses::{Adapter, AdapterError, Algorithm, CoordinateFrame, Point3, SpatialReference};

/// The `tpt-cv` release this adapter was written against.
pub const TPT_CV_VERSION: &str = "0.1.0";

/// Confidence policy and identity for a [`CvAdapter`].
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct CvConfig {
    /// The camera or sensor the input came from.
    pub sensor: SensorId,
    /// Confidence given to each keypoint. FAST responses are scores, not probabilities.
    pub keypoint_confidence: Confidence,
    /// Confidence given to each successful optical-flow track.
    pub flow_confidence: Confidence,
    /// Confidence given to a camera pose estimate.
    pub pose_confidence: Confidence,
}

impl CvConfig {
    /// A configuration with conservative defaults (0.5 for keypoints and flow, 0.7 for poses).
    #[must_use]
    pub fn new(sensor: SensorId) -> Self {
        Self {
            sensor,
            keypoint_confidence: Confidence::new(0.5).unwrap_or(Confidence::ZERO),
            flow_confidence: Confidence::new(0.5).unwrap_or(Confidence::ZERO),
            pose_confidence: Confidence::new(0.7).unwrap_or(Confidence::ZERO),
        }
    }
}

/// One batch of `tpt-cv` output, taken at a single time.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum CvInput {
    /// FAST/ORB keypoints detected in an image.
    Keypoints {
        /// Capture time of the image.
        timestamp: Timestamp,
        /// The keypoints.
        keypoints: Vec<KeyPoint>,
    },
    /// Optical-flow results: the starting point of each track and what happened to it.
    Flow {
        /// Capture time of the later image.
        timestamp: Timestamp,
        /// Each tracked point with its outcome.
        tracks: Vec<(Point, TrackResult)>,
    },
    /// A camera pose estimated from correspondences.
    Pose {
        /// Capture time of the image the pose was estimated from.
        timestamp: Timestamp,
        /// World-to-camera pose, as `tpt-cv` reports it.
        pose: Pose,
    },
}

/// Converts `tpt-cv` output into observations.
#[derive(Clone, Debug)]
pub struct CvAdapter {
    config: CvConfig,
    next_id: u64,
}

impl CvAdapter {
    /// Creates an adapter whose observation ids start at `first_id`.
    #[must_use]
    pub const fn new(config: CvConfig, first_id: ObservationId) -> Self {
        Self {
            config,
            next_id: first_id.get(),
        }
    }

    fn id(&mut self) -> ObservationId {
        let id = ObservationId::new(self.next_id);
        self.next_id += 1;
        id
    }

    fn provenance(&self, step: &str) -> Provenance {
        Provenance::new(Origin::Real)
            .with_step(ProvenanceStep::Sensor(self.config.sensor))
            .with_step(ProvenanceStep::Algorithm(Algorithm::new(
                step,
                TPT_CV_VERSION,
            )))
    }
}

fn finite(values: &[f32]) -> bool {
    values.iter().all(|v| v.is_finite())
}

impl Adapter for CvAdapter {
    type Input = CvInput;

    fn name(&self) -> &'static str {
        "tpt-cv-adapter"
    }

    fn adapt(&mut self, input: CvInput) -> Result<Vec<Observation>, AdapterError> {
        match input {
            CvInput::Keypoints {
                timestamp,
                keypoints,
            } => {
                let confidence = self.config.keypoint_confidence;
                let provenance = self.provenance("tpt-cv-feature::fast");
                let mut out = Vec::with_capacity(keypoints.len());
                for kp in keypoints {
                    if !kp.angle.is_finite() {
                        return Err(AdapterError::Invalid("non-finite keypoint angle".into()));
                    }
                    let id = self.id();
                    out.push(
                        Observation::new(
                            id,
                            Modality::Visual,
                            Property::new("keypoint_response"),
                            ObservationValue::Scalar(f64::from(kp.response)),
                            confidence,
                            timestamp,
                            provenance.clone(),
                        )
                        .with_location(pixel(kp.pt)),
                    );
                }
                Ok(out)
            }
            CvInput::Flow { timestamp, tracks } => {
                let confidence = self.config.flow_confidence;
                let provenance = self.provenance("tpt-cv-feature::lucas-kanade");
                let mut out = Vec::new();
                for (start, result) in tracks {
                    // Failed tracks produce no observation: there is no motion to report.
                    if let TrackResult::Tracked(dx, dy) = result {
                        if !finite(&[dx, dy]) {
                            return Err(AdapterError::Invalid("non-finite flow offset".into()));
                        }
                        let id = self.id();
                        out.push(
                            Observation::new(
                                id,
                                Modality::Visual,
                                Property::new("optical_flow"),
                                ObservationValue::Vector(vec![f64::from(dx), f64::from(dy)]),
                                confidence,
                                timestamp,
                                provenance.clone(),
                            )
                            .with_location(pixel(start)),
                        );
                    }
                }
                Ok(out)
            }
            CvInput::Pose { timestamp, pose } => {
                if !finite(&pose.rotation) || !finite(&pose.translation) {
                    return Err(AdapterError::Invalid("non-finite camera pose".into()));
                }
                let confidence = self.config.pose_confidence;
                let provenance = self.provenance("tpt-cv-geo::solve-pnp");
                let center = camera_center(&pose);
                let position = self.id();
                let orientation = self.id();
                let position_obs = Observation::new(
                    position,
                    Modality::Position,
                    Property::new("camera_position"),
                    ObservationValue::Vector(center.iter().map(|v| f64::from(*v)).collect()),
                    confidence,
                    timestamp,
                    provenance.clone(),
                )
                .with_location(SpatialReference::at(
                    CoordinateFrame::World,
                    center_point(center),
                ));
                let orientation_obs = Observation::new(
                    orientation,
                    Modality::Orientation,
                    Property::new("camera_orientation"),
                    ObservationValue::Vector(pose.rotation.iter().map(|v| f64::from(*v)).collect()),
                    confidence,
                    timestamp,
                    provenance,
                );
                Ok(vec![position_obs, orientation_obs])
            }
        }
    }
}

fn pixel(p: Point) -> SpatialReference {
    SpatialReference::at(
        CoordinateFrame::Pixel,
        [f64::from(p.x), f64::from(p.y), 0.0],
    )
}

/// The camera centre in world coordinates: `C = -Rᵀ t` for a world-to-camera pose `(R, t)`.
///
/// `R` is row-major, so column `i` of `R` dotted with `t` gives `(Rᵀ t)[i]`.
#[must_use]
pub fn camera_center(pose: &Pose) -> [f32; 3] {
    let r = &pose.rotation;
    let t = &pose.translation;
    let mut c = [0.0f32; 3];
    for (i, out) in c.iter_mut().enumerate() {
        let dot = r[i] * t[0] + r[3 + i] * t[1] + r[6 + i] * t[2];
        *out = -dot;
    }
    c
}

fn center_point(c: [f32; 3]) -> Point3 {
    [f64::from(c[0]), f64::from(c[1]), f64::from(c[2])]
}

#[cfg(test)]
#[allow(clippy::float_cmp, reason = "camera centres in these tests are exact")]
mod tests {
    use super::*;

    fn ts() -> Timestamp {
        Timestamp::new(42, ClockSource::Hardware)
    }

    fn adapter() -> CvAdapter {
        CvAdapter::new(CvConfig::new(SensorId::new(5)), ObservationId::new(100))
    }

    fn identity_pose(t: [f32; 3]) -> Pose {
        Pose {
            rotation: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
            translation: t,
        }
    }

    #[test]
    fn camera_center_is_minus_rt() {
        // Identity rotation, camera 2 units in front of the world origin along +z.
        let c = camera_center(&identity_pose([0.0, 0.0, -2.0]));
        assert_eq!(c, [0.0, 0.0, 2.0]);
    }

    #[test]
    fn camera_center_accounts_for_rotation() {
        // Camera rotated 90 degrees about z; with t = (0, -1, 0) the centre is at world (1, 0, 0).
        let rot = [0.0, -1.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0];
        let pose = Pose {
            rotation: rot,
            translation: [0.0, -1.0, 0.0],
        };
        let c = camera_center(&pose);
        assert!(
            (c[0] - 1.0).abs() < 1e-6 && c[1].abs() < 1e-6 && c[2].abs() < 1e-6,
            "{c:?}"
        );
    }

    #[test]
    fn keypoints_become_pixel_frame_visual_observations() {
        let kps = vec![KeyPoint {
            pt: Point::new(10, 20),
            angle: 0.5,
            response: 87,
            descriptor: [0; 4],
        }];
        let out = adapter()
            .adapt(CvInput::Keypoints {
                timestamp: ts(),
                keypoints: kps,
            })
            .unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].modality(), &Modality::Visual);
        assert_eq!(out[0].value(), &ObservationValue::Scalar(87.0));
        let loc = out[0].location().unwrap();
        assert_eq!(loc.frame, CoordinateFrame::Pixel);
        assert_eq!(loc.position, Some([10.0, 20.0, 0.0]));
        assert_eq!(out[0].provenance().sensors().next(), Some(SensorId::new(5)));
    }

    #[test]
    fn failed_tracks_produce_no_observation() {
        let tracks = vec![
            (Point::new(1, 1), TrackResult::Tracked(2.0, -1.0)),
            (Point::new(3, 3), TrackResult::Failed),
        ];
        let out = adapter()
            .adapt(CvInput::Flow {
                timestamp: ts(),
                tracks,
            })
            .unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].value(), &ObservationValue::Vector(vec![2.0, -1.0]));
    }

    #[test]
    fn pose_becomes_world_frame_position_and_orientation() {
        let out = adapter()
            .adapt(CvInput::Pose {
                timestamp: ts(),
                pose: identity_pose([0.0, 0.0, -2.0]),
            })
            .unwrap();
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].modality(), &Modality::Position);
        assert_eq!(out[0].location().unwrap().frame, CoordinateFrame::World);
        assert_eq!(out[0].location().unwrap().position, Some([0.0, 0.0, 2.0]));
        assert_eq!(out[1].modality(), &Modality::Orientation);
        assert_eq!(out[0].confidence(), Confidence::new(0.7).unwrap());
    }

    #[test]
    fn non_finite_pose_is_rejected_not_passed_on() {
        let mut pose = identity_pose([0.0, 0.0, 0.0]);
        pose.translation[1] = f32::NAN;
        assert!(matches!(
            adapter().adapt(CvInput::Pose {
                timestamp: ts(),
                pose
            }),
            Err(AdapterError::Invalid(_))
        ));
    }

    #[test]
    fn observation_ids_are_unique_across_batches() {
        let mut a = adapter();
        let first = a
            .adapt(CvInput::Pose {
                timestamp: ts(),
                pose: identity_pose([0.0; 3]),
            })
            .unwrap();
        let second = a
            .adapt(CvInput::Pose {
                timestamp: ts(),
                pose: identity_pose([0.0; 3]),
            })
            .unwrap();
        let mut ids: Vec<u64> = first
            .iter()
            .chain(second.iter())
            .map(|o| o.id().get())
            .collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 4);
    }
}
