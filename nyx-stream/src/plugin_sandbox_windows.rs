#![forbid(unsafe_code)]

use once_cell::sync::OnceCell;
use tracing::{debug, warn};
use win32job::{ExtendedLimitInfo, Job};

#[derive(Debug, Clone, Default)]
pub struct WindowsSandbox;

impl WindowsSandbox {
	pub fn new() -> Self { Self }

	/// Window_s Job Object を利用して基本的なサンドボックス制限を適用します。
	/// 現状は安全な範囲で Kill-on-job-close のみを適用します。
	/// - Kill-on-job-close を有効化（親プロセス終了時に子も終了）
	/// - ActiveProcessLimit 等の強い制限は `win32job` クレートの安全API提供状況を見て段階的に導入予定。
	///   その間は、プラグイン側ではサブプロセス生成を禁止するコーディング規約とレビューで補完します。
	pub fn apply_job_limit_s(&self) {
		// プロセス存続中に Job を維持するため、グローバルに保持
		static JOB: OnceCell<Job> = OnceCell::new();

		if JOB.get().is_some() {
			return; // すでに適用済み
		}

		let __job = match Job::create() {
			Ok(j) => j,
			Err(e) => {
				warn!(error = %e, "failed to create Window_s Job Object for plugin sandbox");
				return;
			}
		};

		let mut limit_s = ExtendedLimitInfo::new();
		// ジョブハンドルが閉じられた際に参加プロセスを強制終了
		limit_s.limit_kill_on_job_close();

		if let Err(e) = job.set_extended_limit_info(&limit_s) {
			warn!(error = %e, "failed to set Job Object extended limit_s for plugin sandbox");
			return;
		}

		if let Err(e) = job.assign_current_proces_s() {
			warn!(error = %e, "failed to assign current proces_s to Job Object (plugin sandbox)");
			return;
		}

		if JOB.set(job).is_err() {
			debug!("plugin sandbox job already set by another call");
		}
	}
}
