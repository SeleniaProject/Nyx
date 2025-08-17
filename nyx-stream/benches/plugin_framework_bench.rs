
#![allow(clippy::uninlined_format_args)]
use criterion::{criterion_group, criterion_main, Criterion, BatchSize, black_box};
use nyx_stream::plugin::{PluginHeader, PluginId, FRAME_TYPE_PLUGIN_DATA};
use nyx_stream::plugin_frame::PluginFrame;
use nyx_stream::plugin_registry::{PluginRegistry, Permission, PluginInfo};
use nyx_stream::plugin_dispatch::PluginDispatcher;
use std::sync::Arc;

fn bench_cbor_roundtrip(c: &mut Criterion) {
	let hdr = PluginHeader { id: PluginId(123), flags: 0xA5, data: vec![1,2,3,4,5,6,7,8] };
	let payload = vec![0xABu8; 4096];
	c.bench_function("plugin_frame_cbor_roundtrip_4k", |b| {
		b.iter_batched(
			|| PluginFrame::new(FRAME_TYPE_PLUGIN_DATA, hdr.clone(), payload.clone()),
			|pf| {
				let bytes = pf.to_cbor().unwrap();
				let _ = black_box(PluginFrame::from_cbor(&bytes).unwrap());
			},
			BatchSize::SmallInput,
		)
	});
}

fn bench_dispatch_nowait(c: &mut Criterion) {
	let rt = tokio::runtime::Runtime::new().unwrap();
	c.bench_function("plugin_dispatch_nowait", |b| {
		b.to_async(&rt).iter(|| async {
			let registry = Arc::new(PluginRegistry::new());
			let pid = PluginId(1);
			let info = PluginInfo::new(pid, "bench", [Permission::DataAccess]);
			registry.register(info.clone()).await.unwrap();
			let dispatcher = PluginDispatcher::new(registry.clone());
			dispatcher.load_plugin_with_capacity(info, 1024).await.unwrap();
			let hdr = PluginHeader { id: pid, flags: 0, data: vec![] };
			let mut buf = Vec::new();
			ciborium::ser::into_writer(&hdr, &mut buf).unwrap();
			let _ = dispatcher.dispatch_plugin_frame_nowait(FRAME_TYPE_PLUGIN_DATA, buf).await;
		})
	});
}

fn criterion_benchmark(c: &mut Criterion) {
	bench_cbor_roundtrip(c);
	bench_dispatch_nowait(c);
}

criterion_group!(name = benches; config = Criterion::default(); targets = criterion_benchmark);
criterion_main!(benches);

