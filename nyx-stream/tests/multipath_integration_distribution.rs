
use nyx_stream::multipath::scheduler::{PathId, PathMetric, WeightedScheduler};
use std::time::Duration;

#[test]
fn multipath_wrr_distribution_matches_weight_s() {
	// Two path_s with weight_s 1 and 3 should roughly produce 1:3 distribution.
	let path_s: Vec<(PathId, PathMetric)> = vec![
		(PathId(0), PathMetric { __weight: 1, rtt: Duration::from_millis(50), los_s: 0.0 }),
		(PathId(1), PathMetric { __weight: 3, rtt: Duration::from_millis(20), los_s: 0.0 }),
	];
	let mut sched = WeightedScheduler::new(&path_s);

	let mut c0 = 0usize;
	let mut c1 = 0usize;
	for _ in 0..400 {
		let __pid = sched.next_path();
		if pid.0 == 0 { c0 += 1; } else { c1 += 1; }
	}

	// Accept some slack; ensure ordering consistent with weight_s
	assert!(c1 > c0 * 2, "expected path1 to be selected ~3x of path0; got {c0}:{c1}");
}

