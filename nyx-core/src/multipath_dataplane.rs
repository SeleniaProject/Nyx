/// Minimal policy-driven scheduler interface for multipath.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathClass {
    Primary,
    Secondary,
}

#[derive(Debug, Clone)]
pub struct PathMetrics {
    pub latency_ms: u128,
    pub loss_rate: f64,
}

#[derive(Debug, Clone, Copy)]
pub enum SchedulePolicy {
    LowestLatency,
    LowestLoss,
    Weighted { w_latency: f64, w_loss: f64 },
}

pub fn choose_path(m: &PathMetrics, n: &PathMetrics, policy: SchedulePolicy) -> PathClass {
    match policy {
        SchedulePolicy::LowestLatency => {
            if m.latency_ms <= n.latency_ms {
                PathClass::Primary
            } else {
                PathClass::Secondary
            }
        }
        SchedulePolicy::LowestLoss => {
            if m.loss_rate <= n.loss_rate {
                PathClass::Primary
            } else {
                PathClass::Secondary
            }
        }
        SchedulePolicy::Weighted { w_latency, w_loss } => {
            let s1 = w_latency * (m.latency_ms as f64) + w_loss * m.loss_rate;
            let s2 = w_latency * (n.latency_ms as f64) + w_loss * n.loss_rate;
            if s1 <= s2 {
                PathClass::Primary
            } else {
                PathClass::Secondary
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn policy_behaves() {
        let a = PathMetrics {
            latency_ms: 50,
            loss_rate: 0.02,
        };
        let b = PathMetrics {
            latency_ms: 60,
            loss_rate: 0.005,
        };
        assert_eq!(
            choose_path(&a, &b, SchedulePolicy::LowestLatency),
            PathClass::Primary
        );
        assert_eq!(
            choose_path(&a, &b, SchedulePolicy::LowestLoss),
            PathClass::Secondary
        );
        // Heavily weight loss => b should win (lower loss)
        assert_eq!(
            choose_path(
                &a,
                &b,
                SchedulePolicy::Weighted {
                    w_latency: 1.0,
                    w_loss: 10000.0
                }
            ),
            PathClass::Secondary
        );
    }
}
