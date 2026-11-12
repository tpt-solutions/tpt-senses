//! Phase 2 tests: streams, ordering, temporal alignment, calibration, trust, events, spatial
//! references, and fault cases (stale data, bad timestamps, out-of-order messages, sensor noise,
//! calibration failure).

use core::num::NonZeroUsize;
use core::pin::Pin;
use core::task::{Context, Poll, Waker};
use std::collections::VecDeque;

use futures_core::Stream;
use tpt_senses::prelude::*;
use tpt_senses::{
    AuthenticationStatus, BackpressurePolicy, CalibrationKind, CalibrationRecord, CalibrationState,
    CoordinateFrame, Diagnostics, Duration, Event, EventKind, Freshness, Ingest, RateLimiter,
    ReorderBuffer, Sample, SensorCapabilities, SensorStream, SourceIdentity, SpatialReference,
    TimingInfo, Transform, Translation, TrustLevel, TrustMetadata, align_linear, align_nearest,
    freshness, simultaneous, window_mean,
};

fn mono(ns: u64) -> Timestamp {
    Timestamp::new(ns, ClockSource::Monotonic)
}

fn cap(n: usize) -> NonZeroUsize {
    NonZeroUsize::new(n).unwrap()
}

/// A stream that yields a fixed list of items and then ends.
struct Feed {
    items: VecDeque<u64>,
}

impl Feed {
    fn new(items: &[u64]) -> Self {
        Self {
            items: items.iter().copied().collect(),
        }
    }
}

impl Stream for Feed {
    type Item = u64;
    fn poll_next(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<u64>> {
        Poll::Ready(self.items.pop_front())
    }
}

/// A stream that is Pending once before yielding a single item.
struct Gate {
    armed: bool,
    item: Option<u64>,
}

impl Stream for Gate {
    type Item = u64;
    fn poll_next(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<u64>> {
        if !self.armed {
            self.armed = true;
            return Poll::Pending;
        }
        Poll::Ready(self.item.take())
    }
}

fn poll_now<S: Stream + Unpin>(stream: &mut S) -> Poll<Option<S::Item>> {
    let mut cx = Context::from_waker(Waker::noop());
    Pin::new(stream).poll_next(&mut cx)
}

fn drain<S: Stream + Unpin>(stream: &mut S) -> Vec<S::Item> {
    let mut out = Vec::new();
    while let Poll::Ready(Some(item)) = poll_now(stream) {
        out.push(item);
    }
    out
}

// ---- Streams -------------------------------------------------------------------------------

#[test]
fn sensor_stream_passes_items_through_in_order() {
    let mut s = SensorStream::new(
        Feed::new(&[1, 2, 3]),
        cap(8),
        BackpressurePolicy::DropOldest,
    );
    assert_eq!(drain(&mut s), vec![1, 2, 3]);
    assert_eq!(s.dropped_total(), 0);
}

#[test]
fn sensor_stream_reports_drops_under_overload() {
    // Budget equals capacity (2): the producer is pulled faster than the consumer drains it.
    let mut s = SensorStream::new(
        Feed::new(&[1, 2, 3, 4, 5]),
        cap(2),
        BackpressurePolicy::DropOldest,
    );
    let got = drain(&mut s);
    assert_eq!(got, vec![1, 3, 4, 5]);
    assert_eq!(s.dropped_total(), 1);
    assert_eq!(s.take_dropped(), 1);
    assert_eq!(s.take_dropped(), 0);
}

#[test]
fn drop_newest_policy_keeps_backlog_and_counts() {
    let mut s = SensorStream::new(
        Feed::new(&[1, 2, 3, 4]),
        cap(2),
        BackpressurePolicy::DropNewest,
    );
    let got = drain(&mut s);
    assert_eq!(got, vec![1, 2, 3]);
    assert_eq!(s.dropped_total(), 1);
}

#[test]
fn pending_inner_stream_yields_pending_then_item() {
    let mut s = SensorStream::new(
        Gate {
            armed: false,
            item: Some(7),
        },
        cap(4),
        BackpressurePolicy::DropOldest,
    );
    assert!(poll_now(&mut s).is_pending());
    assert_eq!(poll_now(&mut s), Poll::Ready(Some(7)));
}

#[test]
fn cancellation_ends_stream_and_discards_buffer() {
    let mut s = SensorStream::new(
        Feed::new(&[1, 2, 3]),
        cap(4),
        BackpressurePolicy::DropOldest,
    );
    assert_eq!(poll_now(&mut s), Poll::Ready(Some(1)));
    s.cancel();
    assert!(s.is_cancelled());
    assert_eq!(poll_now(&mut s), Poll::Ready(None));
}

#[test]
fn stream_ends_when_inner_ends() {
    let mut s = SensorStream::new(Feed::new(&[]), cap(4), BackpressurePolicy::DropOldest);
    assert_eq!(poll_now(&mut s), Poll::Ready(None));
}

// ---- Ordering, out-of-order and stale messages ---------------------------------------------

#[test]
fn out_of_order_messages_are_restored() {
    let mut rb = ReorderBuffer::new(Duration::from_nanos(50), cap(64));
    for ns in [30u64, 10, 20, 90, 60] {
        rb.push(Sample::new(mono(ns), ns)).unwrap();
    }
    let ordered: Vec<u64> = rb.pop_ready().iter().map(|s| s.value).collect();
    assert_eq!(ordered, vec![10, 20, 30]);
    let rest: Vec<u64> = rb.flush().iter().map(|s| s.value).collect();
    assert_eq!(rest, vec![60, 90]);
    assert_eq!(rb.late_dropped(), 0);
}

#[test]
fn message_older_than_emitted_point_is_late() {
    let mut rb = ReorderBuffer::new(Duration::from_nanos(5), cap(64));
    rb.push(Sample::new(mono(100), "a")).unwrap();
    rb.push(Sample::new(mono(200), "b")).unwrap();
    let _ = rb.pop_ready();
    assert_eq!(
        rb.push(Sample::new(mono(10), "stale")).unwrap(),
        Ingest::Late
    );
    assert_eq!(rb.late_dropped(), 1);
}

#[test]
fn reorder_buffer_is_bounded_under_flood() {
    // A burst larger than the buffer must not grow memory; the excess is counted.
    let mut rb = ReorderBuffer::new(Duration::from_secs(3600), cap(4));
    let outcomes: Vec<Ingest> = (0..10u64)
        .map(|ns| rb.push(Sample::new(mono(ns), ns)).unwrap())
        .collect();
    assert_eq!(
        outcomes.iter().filter(|o| **o == Ingest::Buffered).count(),
        4
    );
    assert_eq!(
        outcomes
            .iter()
            .filter(|o| **o == Ingest::Overflowed)
            .count(),
        6
    );
    assert_eq!(rb.pending_len(), 4);
    assert_eq!(rb.overflow_dropped(), 6);
}

#[test]
fn stale_data_is_detected() {
    let now = mono(10_000_000);
    let max_age = Duration::from_millis(1);
    assert_eq!(
        freshness(mono(9_999_000), now, max_age),
        Some(Freshness::Fresh)
    );
    assert_eq!(freshness(mono(1_000), now, max_age), Some(Freshness::Stale));
}

#[test]
fn bad_timestamp_from_future_is_flagged() {
    let now = mono(1_000);
    assert_eq!(
        freshness(mono(5_000), now, Duration::from_secs(1)),
        Some(Freshness::FromFuture)
    );
}

// ---- Temporal alignment, no silent simultaneity ---------------------------------------------

#[test]
fn mixed_rate_sensors_align_by_interpolation() {
    // A 10 Hz camera and a 1 Hz thermometer, aligned at the camera frame time.
    let thermometer = [
        Sample::new(mono(0), 20.0),
        Sample::new(mono(1_000_000_000), 22.0),
    ];
    let frame_time = mono(250_000_000);
    let value = align_linear(&thermometer, frame_time).unwrap().unwrap();
    assert!((value - 20.5).abs() < 1e-9);
}

#[test]
fn nearest_alignment_refuses_samples_outside_tolerance() {
    let samples = [Sample::new(mono(0), 1.0)];
    assert!(
        align_nearest(&samples, mono(10_000), Duration::from_nanos(100))
            .unwrap()
            .is_none()
    );
    assert!(
        window_mean(&samples, mono(0), Duration::from_nanos(1))
            .unwrap()
            .is_some()
    );
}

#[test]
fn asynchronous_clocks_are_never_simultaneous() {
    let camera = Timestamp::new(1_000, ClockSource::Hardware);
    let lidar = Timestamp::new(1_000, ClockSource::Wall);
    assert_eq!(simultaneous(camera, lidar, Duration::from_secs(10)), None);
    assert_eq!(simultaneous(camera, camera, Duration::ZERO), Some(true));
}

#[test]
fn timing_info_tracks_processing_latency() {
    let mut timing = TimingInfo::acquired(mono(0));
    timing.processed = Some(mono(2_000_000));
    assert_eq!(timing.latency(), Some(Duration::from_millis(2)));
}

// ---- Calibration and provenance ------------------------------------------------------------

#[test]
fn calibration_is_recorded_in_provenance() {
    let mut state = CalibrationState::new();
    state.record(
        CalibrationRecord::new(CalibrationKind::Intrinsic, mono(0))
            .with_parameter("focal_px", 800.0),
    );
    let cal = state
        .usable(CalibrationKind::Intrinsic, mono(10))
        .unwrap()
        .clone();
    let provenance = Provenance::new(Origin::Real).with_step(ProvenanceStep::Calibration(cal));
    let recorded = provenance.calibrations().next().unwrap();
    assert_eq!(recorded.parameter("focal_px"), Some(800.0));
}

#[test]
fn failed_calibration_blocks_use() {
    let mut state = CalibrationState::new();
    state.record(CalibrationRecord::new(CalibrationKind::ZeroPoint, mono(0)).failed());
    assert!(state.usable(CalibrationKind::ZeroPoint, mono(5)).is_none());
}

#[test]
fn expired_calibration_blocks_use() {
    let mut state = CalibrationState::new();
    state.record(
        CalibrationRecord::new(CalibrationKind::Temperature, mono(0)).valid_until(mono(100)),
    );
    assert!(
        state
            .usable(CalibrationKind::Temperature, mono(50))
            .is_some()
    );
    assert!(
        state
            .usable(CalibrationKind::Temperature, mono(500))
            .is_none()
    );
}

// ---- Sensor health and noise ----------------------------------------------------------------

#[test]
fn diagnostics_capture_noise_and_drops() {
    let diag = Diagnostics {
        signal_quality: Some(0.2),
        noise: Some(3.5),
        drop_rate: Some(0.1),
        ..Default::default()
    };
    assert!(diag.signal_quality.unwrap() < 0.5);
    assert!(diag.noise.unwrap() > 1.0);
}

// ---- Trust ----------------------------------------------------------------------------------

#[test]
fn observation_from_network_source_is_not_trusted_by_default() {
    let obs = Observation::new(
        ObservationId::new(11),
        Modality::Position,
        Property::new("vehicle_position"),
        ObservationValue::Vector(vec![1.0, 2.0, 0.0]),
        Confidence::new(0.99).unwrap(),
        mono(0),
        Provenance::new(Origin::Unknown),
    );
    assert_eq!(obs.trust().level, TrustLevel::Unknown);
    assert_eq!(
        obs.trust().authentication,
        AuthenticationStatus::Unauthenticated
    );
    assert_eq!(obs.state(), ObservationState::Raw);
}

#[test]
fn authenticated_source_metadata_is_recorded() {
    let trust = TrustMetadata {
        source: Some(SourceIdentity::new("robot-7")),
        authentication: AuthenticationStatus::Authenticated,
        level: TrustLevel::Verified,
        ..TrustMetadata::unverified()
    };
    let obs = Observation::new(
        ObservationId::new(12),
        Modality::Custom("telemetry".into()),
        Property::new("battery"),
        ObservationValue::Scalar(0.8),
        Confidence::new(0.9).unwrap(),
        mono(0),
        Provenance::new(Origin::Real),
    )
    .with_trust(trust.clone());
    assert_eq!(obs.trust(), &trust);
    assert!(!obs.trust().is_known_bad());
}

// ---- Spatial references and events ----------------------------------------------------------

#[test]
fn observation_location_is_transformed_between_frames() {
    let camera_to_robot = Translation::new(
        CoordinateFrame::Camera,
        CoordinateFrame::Robot,
        [0.0, 0.0, 0.3],
    );
    let in_camera = SpatialReference::at(CoordinateFrame::Camera, [1.0, 0.0, 2.0]);
    let obs = Observation::new(
        ObservationId::new(13),
        Modality::Visual,
        Property::new("obstacle"),
        ObservationValue::Boolean(true),
        Confidence::new(0.7).unwrap(),
        mono(0),
        Provenance::new(Origin::Real),
    )
    .with_location(in_camera.clone());
    let robot_frame = camera_to_robot
        .transform_reference(obs.location().unwrap())
        .unwrap();
    assert_eq!(robot_frame.position, Some([1.0, 0.0, 2.3]));
    assert_eq!(obs.location(), Some(&in_camera));
}

#[test]
fn event_carries_entity_and_supporting_measurements() {
    let m = Measurement::new(
        SensorId::new(2),
        Modality::Lidar,
        MeasurementValue::Scalar(4.2),
        mono(10),
        Provenance::new(Origin::Real),
    );
    let event = Event::new(
        EventKind::CollisionDetected,
        mono(10),
        SensorId::new(2),
        Modality::Lidar,
        Confidence::new(0.6).unwrap(),
        Provenance::new(Origin::Real).with_step(ProvenanceStep::Sensor(SensorId::new(2))),
    )
    .with_entity(EntityId::new(27))
    .with_measurement(m);
    assert_eq!(event.measurements.len(), 1);
    assert_eq!(event.entity, Some(EntityId::new(27)));
    assert_eq!(event.kind, EventKind::CollisionDetected);
}

// ---- Rate limiting --------------------------------------------------------------------------

#[test]
fn rate_limiter_thins_a_high_frequency_stream() {
    let mut rl = RateLimiter::new(Duration::from_millis(10));
    let kept = (0..100u64)
        .filter(|i| rl.allow(mono(i * 1_000_000)).unwrap())
        .count();
    // 100 samples at 1 ms spacing, limited to one per 10 ms.
    assert_eq!(kept, 10);
}

// ---- Sensor exposure and history ------------------------------------------------------------

struct Bare {
    caps: SensorCapabilities,
    cal: Option<CalibrationState>,
}

impl Sensor for Bare {
    type Measurement = Measurement;
    fn identity(&self) -> SensorId {
        SensorId::new(77)
    }
    fn capabilities(&self) -> &SensorCapabilities {
        &self.caps
    }
    fn calibration(&self) -> Option<&CalibrationState> {
        self.cal.as_ref()
    }
    fn read(&mut self) -> Result<Measurement, SensorError> {
        Err(SensorError::NotReady)
    }
}

#[test]
fn sensor_exposure_defaults_to_not_provided() {
    let plain = Bare {
        caps: SensorCapabilities::new(vec![Modality::Thermal]),
        cal: None,
    };
    assert!(plain.calibration().is_none());
    assert!(plain.last_reading_at().is_none());

    let mut state = CalibrationState::new();
    state.record(CalibrationRecord::new(
        CalibrationKind::Temperature,
        mono(0),
    ));
    let calibrated = Bare {
        caps: SensorCapabilities::new(vec![Modality::Thermal]),
        cal: Some(state),
    };
    assert_eq!(calibrated.calibration().unwrap().records().len(), 1);
}

#[test]
fn history_window_is_half_open_and_clock_aware() {
    let make = |id: u64, ns: u64, clock: ClockSource| {
        Observation::new(
            ObservationId::new(id),
            Modality::Thermal,
            Property::new("t"),
            ObservationValue::Scalar(0.0),
            Confidence::new(0.5).unwrap(),
            Timestamp::new(ns, clock),
            Provenance::new(Origin::Recorded),
        )
    };
    let observations = [
        make(1, 99, ClockSource::Monotonic),
        make(2, 100, ClockSource::Monotonic),
        make(3, 199, ClockSource::Monotonic),
        make(4, 150, ClockSource::Wall),
        make(5, 200, ClockSource::Monotonic),
    ];
    let picked: Vec<u64> = tpt_senses::observations_between(&observations, mono(100), mono(200))
        .iter()
        .map(|o| o.id().get())
        .collect();
    assert_eq!(picked, vec![2, 3]);
}
