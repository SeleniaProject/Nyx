
use proptest::prelude::*;

// 8. Capability Negotiation → capability_id_strategy
proptest! {
	#[test]
	fn capability_id_strategy(ft in 0u8..=255) {
		let is = nyx_stream::plugin::is_plugin_frame(ft);
		let expect = (0x50..=0x5F).contains(&ft);
		prop_assert_eq!(is, expect);
	}
}

