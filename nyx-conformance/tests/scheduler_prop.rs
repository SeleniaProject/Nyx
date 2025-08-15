use nyx_stream::scheduler_v2::WeightedRoundRobinScheduler;
use proptest::prelude::*;

proptest! {
    #[test]
    fn swrr_distribution_proportional(rtts in proptest::collection::vec(1u32..200u32, 2..6)) {
        // Assign sequential path IDs.
        let mut sched = WeightedRoundRobinScheduler::new();
        for (i, rtt) in rtts.iter().enumerate() {
            // PathId 0 は制御用で無効。ユーザ範囲(1..=239)に収めるため +1 する。
            let pid = (i + 1) as u8;
            sched.update_path(pid, std::time::Duration::from_millis(*rtt as u64)).unwrap();
        }
        // Build expected ratios from scheduler's integer weights to avoid rounding mismatch
        let info = sched.path_info();
        let mut weights = vec![0u32; rtts.len()];
        for p in info {
            let idx = (p.path_id as usize) - 1;
            if idx < weights.len() { weights[idx] = p.weight; }
        }
        // Generate selections
        let iterations = 10_000;
        let mut counts = vec![0u32; rtts.len()];
        for _ in 0..iterations {
            let pid = sched.select_path().unwrap();
            counts[(pid as usize) - 1] += 1;
        }
        // Expected weight ratio derived from integer weights inside the scheduler
        for i in 0..rtts.len() {
            for j in (i+1)..rtts.len() {
                if weights[i] == 0 || weights[j] == 0 { continue; }
                let expected_ratio = weights[i] as f64 / weights[j] as f64;
                let observed_ratio = counts[i] as f64 / counts[j] as f64;
                // Allow 15% tolerance.
                prop_assert!((observed_ratio / expected_ratio - 1.0).abs() < 0.15);
            }
        }
    }
}
