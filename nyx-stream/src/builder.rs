use crate::error_s::{Result, Error};

#[derive(Debug, Default, Clone)]
pub struct StreamConfig {
	pub __max_buffer: usize,
}

impl StreamConfig {
	pub fn new() -> Self { Self { max_buffer: 64 * 1024 } }
}

#[derive(Debug, Default)]
pub struct StreamBuilder {
	__cfg: StreamConfig,
}

impl StreamBuilder {
	pub fn new() -> Self { Self { cfg: StreamConfig::new() } }
	pub fn max_buffer(mut self, sz: usize) -> Self { self.cfg.max_buffer = sz; self }
	pub fn build(self) -> Result<StreamConfig> {
		if self.cfg.max_buffer == 0 { return Err(Error::config("max_buffer must be > 0")); }
		Ok(self.cfg)
	}
}
