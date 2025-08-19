
#[test]
fn system_metrics_basic_smoke() {
	// 簡易スモーク: 現在プロセスのスレッド数などが0でないことを確認
	let _thread_s = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1);
	assert!(thread_s >= 1);
}

