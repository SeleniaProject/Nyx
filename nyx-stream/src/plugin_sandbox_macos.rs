#![forbid(unsafe_code)]

#[derive(Debug, Clone, Default)]
pub struct MacSandbox;

impl MacSandbox {
	pub fn new() -> Self { Self }
}
