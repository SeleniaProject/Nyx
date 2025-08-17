use proptest::prelude::*;
use nyx_stream::plugin::{PluginHeader, PluginId};
use nyx_stream::plugin_frame::PluginFrame;

prop_compose! {
    fn arb_header()(id in 0u32..=u32::MAX, flags in any::<u8>(), data in proptest::collection::vec(any::<u8>(), 0..256)) -> PluginHeader {
        PluginHeader { id: PluginId(id), flags, data }
    }
}

proptest! {
    #[test]
    fn plugin_frame_cbor_round_trip_prop(frame_type in 0x50u8..=0x5Fu8, hdr in arb_header(), payload in proptest::collection::vec(any::<u8>(), 0..1024)) {
        let pf = PluginFrame::new(frame_type, hdr.clone(), payload.clone());
        let bytes = pf.to_cbor().expect("serialize");
        let back = PluginFrame::from_cbor(&bytes).expect("deserialize");
        prop_assert_eq!(back.frame_type, frame_type);
        prop_assert_eq!(back.header, hdr);
        prop_assert_eq!(back.payload, payload);
    }
}
