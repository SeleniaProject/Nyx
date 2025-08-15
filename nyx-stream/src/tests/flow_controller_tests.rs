mod flow_controller_tests {
    use crate::flow_controller::*;

    #[test]
    fn test_flow_controller_creation() {
        let controller = FlowController::new(1000);
        // API: use stats to confirm window size
        assert_eq!(controller.get_stats().flow_window_size, 1000);
    }

    #[test]
    fn test_congestion_controller_creation() {
        let controller = CongestionController::new(1000, 10_000);
        assert_eq!(controller.state(), CongestionState::SlowStart);
        assert_eq!(controller.cwnd(), 1000);
    }

    #[test]
    fn test_window_updates() {
        let mut controller = FlowController::new(1000);
        controller.update_flow_window(500).unwrap();
        assert_eq!(controller.get_stats().flow_window_size, 500);
    }
}
