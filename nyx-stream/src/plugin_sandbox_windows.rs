#![forbid(unsafe_code)]

use once_cell::sync::OnceCell;
use tracing::{debug, warn};
use win32job::{ExtendedLimitInfo, Job};

#[derive(Debug, Clone, Default)]
pub struct WindowsSandbox;

impl WindowsSandbox {
	pub fn new() -> Self { Self }

	/// windows Job Object �𗘗p���Ċ�{�I�ȃT���h�{�b�N�X������K�p���܂��B
	/// ����͈��S�Ȕ͈͂� Kill-on-job-close �݂̂�K�p���܂��B
	/// - Kill-on-job-close ��L�����i�e�v���Z�X�I�����Ɏq���I���j
	/// - ActiveProcessLimit ���̋��������� `win32job` �N���[�g�̈��SAPI�񋟏󋵂����Ēi�K�I�ɓ����\��B
	///   ���̊Ԃ́A�v���O�C�����ł̓T�u�v���Z�X�������֎~����R�[�f�B���O�K��ƃ��r���[�ŕ⊮���܂��B
	pub fn apply_job_limits(&self) {
		// �v���Z�X�������� Job ���ێ����邽�߁A�O���[�o���ɕێ�
		static JOB: OnceCell<Job> = OnceCell::new();

		if JOB.get().is_some() {
			return; // ���łɓK�p�ς�
		}

		let job = match Job::create() {
			Ok(j) => j,
			Err(e) => {
				warn!(error = %e, "failed to create windows Job Object for plugin sandbox");
				return;
			}
		};

		let mut limit_s = ExtendedLimitInfo::new();
		// �W���u�n���h��������ꂽ�ۂɎQ���v���Z�X�������I��
		limit_s.limit_kill_on_job_close();

		if let Err(e) = job.set_extended_limit_info(&limit_s) {
			warn!(error = %e, "failed to set Job Object extended limit_s for plugin sandbox");
			return;
		}

		if let Err(e) = job.assign_current_process() {
			warn!(error = %e, "failed to assign current proces_s to Job Object (plugin sandbox)");
			return;
		}

		if JOB.set(job).is_err() {
			debug!("plugin sandbox job already set by another call");
		}
	}
}
