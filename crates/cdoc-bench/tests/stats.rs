//! Statistics on known samples, and the sampler's warmup and iteration counts.

use cdoc_bench::stats::{basis_points, sample, summarize, Clock, Iterations, StatsError};
use std::cell::Cell;

#[test]
fn one_to_a_hundred_has_median_fifty_and_p95_ninety_five() {
    let samples: Vec<u64> = (1..=100).rev().collect();
    let s = summarize(&samples).unwrap();
    assert_eq!(
        (s.min_ns, s.median_ns, s.p95_ns, s.iterations),
        (1, 50, 95, 100)
    );
}

#[test]
fn an_odd_count_has_its_middle_as_median() {
    let s = summarize(&[9, 1, 5, 3, 7]).unwrap();
    assert_eq!((s.min_ns, s.median_ns, s.p95_ns), (1, 5, 9));
}

#[test]
fn twenty_samples_put_p95_on_the_nineteenth() {
    let samples: Vec<u64> = (1..=20).map(|k| k * 10).collect();
    assert_eq!(summarize(&samples).unwrap().p95_ns, 190);
}

#[test]
fn one_sample_is_every_statistic() {
    let s = summarize(&[42]).unwrap();
    assert_eq!(
        (s.min_ns, s.median_ns, s.p95_ns, s.iterations),
        (42, 42, 42, 1)
    );
}

#[test]
fn no_samples_is_an_error() {
    assert_eq!(summarize(&[]), Err(StatsError::Empty));
}

/// A clock that advances by the step number on every read: sample k takes k ns. The one place a
/// fake stands in for production code, because the real port is the machine's clock.
struct Steps(Cell<u64>, Cell<u64>);

impl Clock for Steps {
    fn now_ns(&self) -> u64 {
        let t = self.0.get();
        self.0.set(t + self.1.get());
        t
    }
}

#[test]
fn the_sampler_warms_up_untimed_then_times_each_iteration() {
    let clock = Steps(Cell::new(0), Cell::new(0));
    let mut calls = 0_u32;
    let plan = Iterations {
        warmup: 3,
        iterations: 5,
    };
    let s = sample::<StatsError>(&clock, plan, || {
        calls += 1;
        clock.1.set(u64::from(calls.saturating_sub(3)));
        Ok(())
    })
    .unwrap();
    assert_eq!(calls, 8);
    assert_eq!(s.iterations, 5);
    assert!(s.min_ns <= s.median_ns && s.median_ns <= s.p95_ns);
}

#[test]
fn ratios_are_integer_basis_points() {
    assert_eq!(basis_points(1, 2), Some(5_000));
    assert_eq!(basis_points(3, 1), Some(30_000));
    assert_eq!(basis_points(1, 0), None);
}
