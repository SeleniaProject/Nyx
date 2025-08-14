//! Feature Matrix smoke tests for nyx-stream crate.
//! Ensures key feature combinations compile and basic symbols are usable.
//! This does not exhaustively test runtime behavior beyond construction.

#[cfg(test)]
mod tests {
    use std::time::Duration;

    // Each cfg block corresponds to a feature combo we want to ensure links.
    // Matrix (subset): base, hpke, hpke+telemetry, plugin, mpr_experimental, fec.

    #[test]
    fn fm_base_compile() {
        // Always true; acts as sentinel build check.
        assert_eq!(2+2,4);
    }

    #[cfg(feature="hpke")]
    #[test]
    fn fm_hpke_manager_basic() {
        use nyx_stream::{HpkeRekeyManager, RekeyPolicy};
        use nyx_crypto::noise::SessionKey;
        let policy = RekeyPolicy { time_interval: Duration::from_millis(10), packet_interval: 5, grace_period: Duration::from_millis(5), min_cooldown: Duration::from_millis(1) };
        let mgr = HpkeRekeyManager::new(policy, SessionKey([0u8;32]));
        assert!(mgr.last_rekey_elapsed() <= Duration::from_millis(50));
    }

    #[cfg(all(feature="hpke", feature="telemetry"))]
    #[test]
    fn fm_hpke_telemetry_symbols() {
        nyx_telemetry::ensure_hpke_rekey_metrics_registered(&prometheus::default_registry());
        nyx_telemetry::inc_hpke_rekey_initiated();
    }

    #[cfg(feature="plugin")]
    #[test]
    fn fm_plugin_feature_compiles() {
        // Just verify a type gated by plugin feature exists.
        use nyx_stream::PluginDescriptor; // Assuming this is exported; adjust if needed.
        let _maybe: Option<PluginDescriptor> = None; // type presence check
        assert!(_maybe.is_none());
    }

    #[cfg(feature="mpr_experimental")]
    #[test]
    fn fm_mpr_experimental_marker() {
        // Construct MprDispatcher and exercise a few basic calls to ensure symbols link.
        use nyx_stream::{WeightedRrScheduler, MprDispatcher};
        let mut sched = WeightedRrScheduler::new();
        sched.update_path(1, 10.0);
        sched.update_path(2, 20.0);
        let mut mpr = MprDispatcher::new(sched, 2);
        let chosen = mpr.choose_paths();
        assert!(!chosen.is_empty());
        // Provide feedback and verify redundancy stays within bounds [1,4]
        mpr.record_feedback(false);
        let k = mpr.redundancy();
        assert!(k >= 1 && k <= 4);
    }

    #[cfg(feature="fec")]
    #[test]
    fn fm_fec_enabled_types() {
        // Confirm Packet type is available via nyx_stream re-export of nyx_fec when fec feature on.
        use nyx_stream::Packet;
        let p = Packet(vec![1,2,3]);
        assert_eq!(p.0.len(),3);
    }
}
