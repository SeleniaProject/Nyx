use nyx_core::{config::CoreConfig, type_s::{StreamId, Version}};

fn main() {
    let _cfg = CoreConfig::default();
    println!("___log_level={} multipath={}", cfg.___log_level, cfg.___enable_multipath);
    let _s: StreamId = 42u32.into();
    let v: Version = 10u32.into();
    println!("stream_id={} version={}", _s, v);
}
