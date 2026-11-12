//! Robustness tests: deterministic fuzzing of the decoders with untrusted bytes, and property
//! tests of the buffers. The random generator is a fixed-seed xorshift, so every run is the same
//! and any failure can be reproduced.
#![cfg(feature = "serde")]
#![allow(
    clippy::cast_possible_truncation,
    reason = "test arithmetic on small indices and a seeded PRNG"
)]

use core::num::NonZeroUsize;

use tpt_senses::prelude::*;
use tpt_senses::{
    BackpressurePolicy, BoundedQueue, Duration, Ingest, PushOutcome, ReorderBuffer, Sample,
};

struct Rng(u64);

impl Rng {
    fn next_u64(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next_u64() % n.max(1)
    }

    fn bytes(&mut self, len: usize) -> Vec<u8> {
        (0..len).map(|_| self.next_u64() as u8).collect()
    }
}

fn sample_observation() -> Observation {
    Observation::new(
        ObservationId::new(1),
        Modality::Thermal,
        Property::new("temperature_c"),
        ObservationValue::Vector(vec![1.0, 2.0]),
        Confidence::new(0.5).unwrap(),
        Timestamp::new(7, ClockSource::Monotonic),
        Provenance::new(Origin::Real).with_step(ProvenanceStep::Sensor(SensorId::new(3))),
    )
}

#[test]
fn random_bytes_never_panic_the_json_decoder() {
    let mut rng = Rng(0x243F_6A88_85A3_08D3);
    let mut accepted = 0;
    for _ in 0..3_000 {
        let len = rng.below(256) as usize;
        let bytes = rng.bytes(len);
        if serde_json::from_slice::<Observation>(&bytes).is_ok() {
            accepted += 1;
        }
    }
    // Random bytes are essentially never a valid observation.
    assert_eq!(accepted, 0);
}

#[test]
fn random_bytes_never_panic_the_cbor_decoder() {
    let mut rng = Rng(0x1319_8A2E_0370_7344);
    for _ in 0..3_000 {
        let len = rng.below(256) as usize;
        let bytes = rng.bytes(len);
        let _ = tpt_senses::from_cbor::<Observation>(&bytes);
        let _ = tpt_senses::from_cbor::<Measurement>(&bytes);
    }
}

#[test]
fn mutated_valid_documents_never_panic() {
    let base_cbor = tpt_senses::to_cbor(&sample_observation()).unwrap();
    let base_json = serde_json::to_vec(&sample_observation()).unwrap();
    let mut rng = Rng(0xA409_3822_299F_31D0);

    for _ in 0..3_000 {
        let mut cbor = base_cbor.clone();
        let mut json = base_json.clone();
        // Flip a few bytes and truncate sometimes.
        for _ in 0..=rng.below(4) {
            let i = rng.below(cbor.len() as u64) as usize;
            cbor[i] ^= rng.next_u64() as u8;
            let j = rng.below(json.len() as u64) as usize;
            json[j] ^= rng.next_u64() as u8;
        }
        if rng.below(4) == 0 {
            cbor.truncate(rng.below(cbor.len() as u64) as usize);
        }
        let _ = tpt_senses::from_cbor::<Observation>(&cbor);
        let _ = serde_json::from_slice::<Observation>(&json);
    }
}

#[test]
fn huge_declared_lengths_are_refused_not_allocated() {
    // CBOR byte string claiming 2^40 bytes, followed by nothing. Must fail cleanly.
    let claim = [0x5b, 0, 0, 1, 0, 0, 0, 0, 0];
    assert!(tpt_senses::from_cbor::<Vec<u8>>(&claim).is_err());
    // CBOR array claiming a huge element count, followed by nothing.
    let array = [0x9b, 0x7f, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff];
    assert!(tpt_senses::from_cbor::<Vec<u32>>(&array).is_err());
}

#[test]
fn bounded_queue_never_exceeds_capacity() {
    let mut rng = Rng(0x0822_EFA9_8EC4_E6C8);
    for policy in [
        BackpressurePolicy::DropOldest,
        BackpressurePolicy::DropNewest,
    ] {
        let cap = 1 + rng.below(16) as usize;
        let mut q = BoundedQueue::new(NonZeroUsize::new(cap).unwrap(), policy);
        let mut pushed = 0u64;
        let mut popped = 0u64;
        for step in 0..5_000u64 {
            if rng.below(3) == 0 {
                if q.try_pop().is_some() {
                    popped += 1;
                }
            } else {
                q.push(step);
                pushed += 1;
            }
            assert!(q.len() <= cap);
        }
        // Every pushed item is either still queued, popped, or counted as dropped.
        assert_eq!(pushed, popped + q.len() as u64 + q.dropped_total());
    }
}

#[test]
fn reorder_output_is_always_non_decreasing_and_conserves_samples() {
    let mut rng = Rng(0x7E2D_3A1F_9B4C_5D6E);
    let lateness = Duration::from_nanos(500);
    let mut rb = ReorderBuffer::new(lateness, NonZeroUsize::new(64).unwrap());
    let mut emitted: Vec<u64> = Vec::new();
    let (mut buffered, mut late, mut overflowed) = (0u64, 0u64, 0u64);
    let mut clock = 10_000u64;

    for _ in 0..20_000 {
        // Mostly advancing time, with jitter so samples arrive out of order.
        clock += rng.below(20);
        let ts = clock.saturating_sub(rng.below(800));
        let outcome = rb
            .push(Sample::new(Timestamp::new(ts, ClockSource::Monotonic), ts))
            .unwrap();
        match outcome {
            Ingest::Buffered => buffered += 1,
            Ingest::Late => late += 1,
            Ingest::Overflowed => overflowed += 1,
        }
        emitted.extend(rb.pop_ready().into_iter().map(|s| s.value));
    }
    emitted.extend(rb.flush().into_iter().map(|s| s.value));

    assert!(
        emitted.windows(2).all(|w| w[0] <= w[1]),
        "reorder buffer emitted samples out of order"
    );
    // Conservation: every accepted sample is emitted, and every rejection is counted.
    assert_eq!(emitted.len() as u64, buffered);
    assert_eq!(late, rb.late_dropped());
    assert_eq!(overflowed, rb.overflow_dropped());
}

#[test]
fn queue_outcomes_match_policy() {
    let mut q = BoundedQueue::new(
        NonZeroUsize::new(1).unwrap(),
        BackpressurePolicy::DropOldest,
    );
    assert_eq!(q.push(1), PushOutcome::Accepted);
    assert_eq!(q.push(2), PushOutcome::DroppedOldest);
    assert_eq!(q.try_pop(), Some(2));
}
