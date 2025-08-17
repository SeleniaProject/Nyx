#![forbid(unsafe_code)]

#[derive(Debug, Clone, Default)]
pub struct WindowsSandbox;

impl WindowsSandbox {
	pub fn new() -> Self { Self }
}
