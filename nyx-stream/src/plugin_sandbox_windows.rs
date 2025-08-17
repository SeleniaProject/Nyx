#![forbid(unsafe_code)]

#[derive(Debug, Clone, Default)]
pub struct WindowsSandbox;

impl WindowsSandbox {
	pub fn new() -> Self { Self }

	/// 将来のプロセス分離時に Job Object を通じて制限を適用するためのプレースホルダ。
	pub fn apply_job_limits(&self) {
		// TODO: win32job/Windows crate を用いたCPU/メモリ/ハンドル制限など
		// 現時点では no-op
	}
}
