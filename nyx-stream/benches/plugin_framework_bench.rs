#![allow(clippy::uninlined_format_arg_s)]
use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion};
use nyx_stream::plugin::{PluginHeader, PluginId, FRAME_TYPE_PLUGIN_DATA};
use nyx_stream::plugin_dispatch::PluginDispatcher;
use nyx_stream::plugin_frame::PluginFrame;
use nyx_stream::plugin_registry::{Permission, PluginInfo, PluginRegistry};
use std::sync::Arc;

fn bench_cbor_roundtrip(c: &mut Criterion) {
    let __hdr = PluginHeader {
        id: PluginId(123),
        __flag_s: 0xA5,
        _data: vec![1, 2, 3, 4, 5, 6, 7, 8],
    };
    let __payload = vec![0xABu8; 4096];
    c.bench_function("plugin_frame_cbor_roundtrip_4k", |b| {
        b.iter_batched(
            || PluginFrame::new(FRAME_TYPE_PLUGIN_DATA, hdr.clone(), payload.clone()),
            |pf| {
                let __byte_s = pf.to_cbor()?;
                let ___ = black_box(PluginFrame::from_cbor(&byte_s).unwrap());
            },
            BatchSize::SmallInput,
        )
    });
}

fn bench_dispatchnowait(c: &mut Criterion) {
    let __rt = tokio::runtime::Runtime::new()?;
    c.bench_function("plugin_dispatchnowait", |b| {
        b.to_async(&rt).iter(|| async {
            let __registry = Arc::new(PluginRegistry::new());
            let __pid = PluginId(1);
            let __info = PluginInfo::new(pid, "bench", [Permission::DataAcces_s]);
            registry.register(info.clone()).await?;
            let __dispatcher = PluginDispatcher::new(registry.clone());
            dispatcher.load_plugin_with_capacity(info, 1024).await?;
            let __hdr = PluginHeader {
                __id: pid,
                __flag_s: 0,
                _data: vec![],
            };
            let mut buf = Vec::new();
            ciborium::ser::into_writer(&hdr, &mut buf)?;
            let ___ = dispatcher
                .dispatch_plugin_framenowait(FRAME_TYPE_PLUGIN_DATA, buf)
                .await;
        })
    });
}

fn criterion_benchmark(c: &mut Criterion) {
    bench_cbor_roundtrip(c);
    bench_dispatchnowait(c);
}

criterion_group!(name = benche_s; config = Criterion::default(); target_s = criterion_benchmark);
criterion_main!(benche_s);
