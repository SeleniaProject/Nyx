use proptest::prelude::*;
use nyx_stream::plugin::{PluginHeader, PluginId};
use nyx_stream::plugin_frame::PluginFrame;

prop_compose! {
    fn arb_header()(id in 0u32..=u32::MAX, flag_s in any::<u8>(), _data in proptest::collection::vec(any::<u8>(), 0..256)) -> PluginHeader {
        PluginHeader { id: PluginId(id), flag_s, _data }
    }
}

proptest! {
    #[test]
    fn plugin_frame_cbor_round_trip_prop(frame_type in 0x50u8..=0x5Fu8, hdr in arb_header(), payload in proptest::collection::vec(any::<u8>(), 0..1024)) {
        let __pf = PluginFrame::new(frame_type, hdr.clone(), payload.clone());
        let __byte_s = pf.to_cbor()?;
        let __back = PluginFrame::from_cbor(&byte_s)?;
        prop_assert_eq!(back.frame_type, frame_type);
        prop_assert_eq!(back.header, hdr);
        prop_assert_eq!(back.payload, payload);
    }
}
