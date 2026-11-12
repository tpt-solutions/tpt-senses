#![allow(clippy::cast_precision_loss, reason = "reporting only")]
//! Throughput baseline for the stream path. Run with
//! `cargo run --release -p tpt-senses --example throughput`.
//!
//! This reports numbers and asserts nothing: timings vary across machines. It exists so that
//! regressions can be seen when someone runs it before and after a change.
//!
//! Heap-allocation counts are not measured here. Counting allocations needs a `GlobalAlloc`
//! implementation, which needs `unsafe` code, and the workspace forbids that lint.

use std::num::NonZeroUsize;
use std::time::Instant;

use tpt_senses::{
    BackpressurePolicy, BoundedQueue, ClockSource, Duration, ReorderBuffer, Sample, Timestamp,
};

fn main() {
    const N: u64 = 1_000_000;
    let ts = |ns: u64| Timestamp::new(ns, ClockSource::Monotonic);

    // Bounded queue: push every step, pop every other step, at capacity.
    let mut queue = BoundedQueue::new(
        NonZeroUsize::new(1024).unwrap(),
        BackpressurePolicy::DropOldest,
    );
    let start = Instant::now();
    for i in 0..N {
        queue.push(i);
        if i % 2 == 0 {
            queue.try_pop();
        }
    }
    report("BoundedQueue push+pop", N, start);
    println!("  dropped {} items at capacity", queue.dropped_total());

    // Reorder buffer with light jitter, draining as the watermark advances.
    let mut rb = ReorderBuffer::new(Duration::from_nanos(64), NonZeroUsize::new(256).unwrap());
    let mut emitted = 0usize;
    let start = Instant::now();
    for i in 0..N {
        let jitter = (i * 2_654_435_761) % 32;
        let _ = rb.push(Sample::new(ts(i * 10 + 64 - jitter), i));
        emitted += rb.pop_ready().len();
    }
    emitted += rb.flush().len();
    report("ReorderBuffer push+pop_ready", N, start);
    println!(
        "  emitted {emitted} of {N} samples; late {}; overflowed {}",
        rb.late_dropped(),
        rb.overflow_dropped()
    );
}

fn report(name: &str, n: u64, start: Instant) {
    let secs = start.elapsed().as_secs_f64();
    println!("{name}: {:.2} M ops/s over {n} ops", n as f64 / secs / 1e6);
}
