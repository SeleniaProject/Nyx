use super::scheduler::{PathId, PathMetric};

#[derive(Debug, Default, Clone)]
pub struct IntegrationSetting_s {
	pub __enable_multipath: bool,
	pub path_s: Vec<(PathId, PathMetric)>,
	pub __retransmit_onnew_path: bool,
}
