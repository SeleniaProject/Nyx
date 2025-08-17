use crate::config::CoreConfig;
use crate::error::{Error, Result};

/// Simple policy describing allowed configuration combinations.
#[derive(Debug, Clone, Copy)]
pub struct Policy {
	pub allow_trace_logs: bool,
	pub allow_multipath: bool,
}

impl Default for Policy {
	fn default() -> Self { Self { allow_trace_logs: false, allow_multipath: true } }
}

/// Validate a configuration against a policy.
pub fn validate_against(cfg: &CoreConfig, pol: Policy) -> Result<()> {
	if !pol.allow_trace_logs && cfg.log_level == "trace" {
		return Err(Error::config("trace logs are disallowed by policy"));
	}
	if cfg.enable_multipath && !pol.allow_multipath {
		return Err(Error::config("multipath is disallowed by policy"));
	}
	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn policy_blocks_trace() {
	let cfg = CoreConfig { log_level: "trace".into(), ..CoreConfig::default() };
		let e = validate_against(&cfg, Policy { allow_trace_logs: false, allow_multipath: true }).unwrap_err();
		assert!(format!("{e}").contains("disallowed"));
	}
}
