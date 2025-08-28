use super::scheduler::{PathId, PathMetric};

#[derive(Debug, Default, Clone)]
pub struct IntegrationSettings {
    pub enable_multipath: bool,
    pub paths: Vec<(PathId, PathMetric)>,
    pub retransmit_on_new_path: bool,
}
