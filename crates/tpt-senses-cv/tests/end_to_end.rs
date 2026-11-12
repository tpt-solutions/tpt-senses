#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    reason = "test coordinates are small constants"
)]

//! End to end: run real `tpt-cv` detection and tracking on synthetic images, and check the
//! observations that come out.
//!
//! The scene is small bright 3x3 blobs on a dark background. FAST-9 fires on a blob's centre,
//! because the sixteen ring pixels around it are all darker. Larger shapes such as a square's
//! convex corners do not satisfy FAST-9, so they are not used here.

use tpt_cv_core::image::Image;
use tpt_cv_feature::{Point, orb_detect_and_compute, track_points_pyramidal};
use tpt_senses::prelude::*;
use tpt_senses::{Adapter, CoordinateFrame};
use tpt_senses_cv::{CvAdapter, CvConfig, CvInput};

const W: usize = 96;
const H: usize = 96;
/// Blob centres, in pixels.
const CENTRES: [(usize, usize); 3] = [(20, 20), (60, 30), (40, 70)];

/// Dark background with a 3x3 bright blob at each centre, shifted right by `dx` pixels.
fn scene(dx: usize) -> Vec<u8> {
    let mut data = vec![10u8; W * H];
    for &(cx, cy) in &CENTRES {
        for y in (cy - 1)..=(cy + 1) {
            for x in (cx - 1 + dx)..=(cx + 1 + dx) {
                data[y * W + x] = 240;
            }
        }
    }
    data
}

fn ts(ns: u64) -> Timestamp {
    Timestamp::new(ns, ClockSource::Hardware)
}

fn adapter() -> CvAdapter {
    CvAdapter::new(CvConfig::new(SensorId::new(1)), ObservationId::new(1))
}

#[test]
fn detected_keypoints_become_pixel_frame_observations() {
    let data = scene(0);
    let image = Image::<u8, 1>::new(&data, W, H).expect("valid image");
    let keypoints = orb_detect_and_compute(&image, 40u8, 50);
    assert!(!keypoints.is_empty(), "each blob should yield a keypoint");

    let count = keypoints.len();
    let observations = adapter()
        .adapt(CvInput::Keypoints {
            timestamp: ts(1_000),
            keypoints,
        })
        .expect("keypoints adapt");

    assert_eq!(observations.len(), count);
    for o in &observations {
        assert_eq!(o.modality(), &Modality::Visual);
        let loc = o.location().expect("keypoints carry their pixel position");
        assert_eq!(loc.frame, CoordinateFrame::Pixel);
        let [x, y, _] = loc.position.expect("position present");
        // Every keypoint sits next to one of the blobs.
        let near_blob = CENTRES
            .iter()
            .any(|&(cx, cy)| (x - cx as f64).abs() <= 3.0 && (y - cy as f64).abs() <= 3.0);
        assert!(near_blob, "keypoint at ({x}, {y}) is not on a blob");
    }
}

#[test]
fn tracked_blobs_report_the_motion_they_actually_made() {
    let before = scene(0);
    let after = scene(3);
    let a = Image::<u8, 1>::new(&before, W, H).unwrap();
    let b = Image::<u8, 1>::new(&after, W, H).unwrap();

    // Track each blob centre. The pyramidal tracker is needed: a 3-pixel shift is larger than the
    // blob itself, which single-level Lucas-Kanade cannot follow.
    let points: Vec<(f32, f32)> = CENTRES.iter().map(|&(x, y)| (x as f32, y as f32)).collect();
    let results = track_points_pyramidal(&a, &b, &points, 5, 20, 0.01, 3);
    let tracks: Vec<(Point, _)> = CENTRES
        .iter()
        .zip(results)
        .map(|(&(x, y), r)| (Point::new(x as i32, y as i32), r))
        .collect();

    let observations = adapter()
        .adapt(CvInput::Flow {
            timestamp: ts(2_000),
            tracks,
        })
        .expect("flow adapts");

    assert_eq!(observations.len(), CENTRES.len(), "every blob should track");
    for o in &observations {
        let ObservationValue::Vector(v) = o.value() else {
            panic!("flow is a vector")
        };
        // The blobs moved right by 3 pixels.
        assert!((v[0] - 3.0).abs() < 0.5, "dx was {}", v[0]);
        assert!(v[1].abs() < 0.5, "dy was {}", v[1]);
    }
}

#[test]
fn a_blank_image_produces_no_keypoint_observations() {
    let blank = vec![128u8; W * H];
    let image = Image::<u8, 1>::new(&blank, W, H).unwrap();
    let keypoints = orb_detect_and_compute(&image, 40u8, 50);
    let observations = adapter()
        .adapt(CvInput::Keypoints {
            timestamp: ts(0),
            keypoints,
        })
        .unwrap();
    assert!(observations.is_empty());
}
