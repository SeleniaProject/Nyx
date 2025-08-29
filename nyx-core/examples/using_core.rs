#![allow(missing_docs, clippy::unwrap_used)]

use nyx_core::{config::CoreConfig, types::StreamId};
use std::num::NonZeroU32;

fn main() {
    let cfg = CoreConfig::default();
    println!(
        "log_level={} multipath={}",
        cfg.log_level, cfg.enable_multipath
    );
    let s = StreamId::new(NonZeroU32::new(42).unwrap());
    println!("stream_id={s}");
}
