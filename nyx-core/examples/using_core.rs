use nyx_core::{config::CoreConfig, types::{StreamId, Version}};

fn main() {
    let cfg = CoreConfig::default();
    println!("log_level={} multipath={}", cfg.log_level, cfg.enable_multipath);
    let s: StreamId = 42u32.into();
    let v: Version = 10u32.into();
    println!("stream_id={} version={}", s, v);
}
