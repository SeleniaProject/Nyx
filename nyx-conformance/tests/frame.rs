#[test]
fn plugin_frame_range_check() {
    assert!(nyx_stream::plugin::is_plugin_frame(0x50));
    assert!(nyx_stream::plugin::is_plugin_frame(0x5F));
    assert!(!nyx_stream::plugin::is_plugin_frame(0x4F));
}
