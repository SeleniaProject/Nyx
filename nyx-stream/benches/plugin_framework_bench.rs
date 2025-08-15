#![forbid(unsafe_code)]

//! Performance benchmarks for Plugin Framework implementation.
//!
//! These benchmarks measure the performance characteristics of:
//! - CBOR header encoding/decoding
//! - Plugin frame parsing and validation
//! - Permission checking and enforcement
//! - Handshake negotiation latency
//! - Frame processing throughput

#[cfg(feature = "plugin")]
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
#[cfg(not(feature = "plugin"))]
use criterion::{criterion_group, criterion_main, Criterion};

#[cfg(feature = "plugin")]
use nyx_stream::{
    build_plugin_frame, Permission, PluginFrameProcessor, PluginHandshakeCoordinator, PluginHeader,
    PluginInfo, PluginRegistry, Setting, SettingsFrame,
};
#[cfg(feature = "plugin")]
use nyx_stream::management::{plugin_support_flags, setting_ids};

#[cfg(feature = "plugin")]
fn bench_plugin_header_cbor_encoding(c: &mut Criterion) {
    let mut group = c.benchmark_group("plugin_header_cbor");

    let test_headers = vec![
        (
            "minimal",
            PluginHeader {
                id: 1,
                flags: 0x01,
                data: b"",
            },
        ),
        (
            "small",
            PluginHeader {
                id: 1001,
                flags: 0x03,
                data: b"small_data",
            },
        ),
        (
            "medium",
            PluginHeader {
                id: 50001,
                flags: 0x0F,
                data: &vec![0x42; 256],
            },
        ),
        (
            "large",
            PluginHeader {
                id: 999999,
                flags: 0xFF,
                data: &vec![0xAA; 1024],
            },
        ),
    ];

    for (name, header) in test_headers {
        group.bench_with_input(BenchmarkId::new("encode", name), &header, |b, h| {
            b.iter(|| black_box(h.encode()))
        });

        // Pre-encode for decode benchmark
        let encoded = header.encode();
        group.bench_with_input(BenchmarkId::new("decode", name), &encoded, |b, data| {
            b.iter(|| black_box(PluginHeader::decode(data).unwrap()))
        });

        group.bench_with_input(BenchmarkId::new("validate", name), &encoded, |b, data| {
            b.iter(|| black_box(PluginHeader::validate(data).unwrap()))
        });
    }

    group.finish();
}

#[cfg(feature = "plugin")]
fn bench_plugin_frame_building(c: &mut Criterion) {
    let mut group = c.benchmark_group("plugin_frame_building");

    let plugin_header = PluginHeader {
        id: 12345,
        flags: 0x01,
        data: b"control_data",
    };

    let payload_sizes = vec![
        ("tiny", vec![0x42; 64]),
        ("small", vec![0x42; 512]),
        ("medium", vec![0x42; 4096]),
        ("large", vec![0x42; 32768]),
    ];

    for (name, payload) in payload_sizes {
        group.throughput(Throughput::Bytes(payload.len() as u64));
        group.bench_with_input(BenchmarkId::new("build_frame", name), &payload, |b, p| {
            b.iter(|| {
                black_box(build_plugin_frame(0x52, 0x00, Some(7u8), &plugin_header, p).unwrap())
            })
        });
    }

    group.finish();
}

#[cfg(feature = "plugin")]
fn bench_plugin_frame_processing(c: &mut Criterion) {
    let mut group = c.benchmark_group("plugin_frame_processing");

    // Set up test environment
    let registry = PluginRegistry::new();
    let dispatcher = nyx_stream::plugin_dispatch::PluginDispatcher::new(registry.clone());
    let processor = PluginFrameProcessor::new(registry, dispatcher);

    let plugin_header = PluginHeader {
        id: 1001,
        flags: 0x00,
        data: b"benchmark_data",
    };

    // Pre-build frames of different sizes
    let test_frames: Vec<(String, Vec<u8>)> =
        vec![("small", 256), ("medium", 2048), ("large", 16384)]
            .into_iter()
            .map(|(name, size)| {
                let payload = vec![0x42; size];
                let frame = build_plugin_frame(0x53, 0x00, None, &plugin_header, &payload).unwrap();
                (name.to_string(), frame)
            })
            .collect();

    for (name, frame_bytes) in test_frames {
        group.throughput(Throughput::Bytes(frame_bytes.len() as u64));

        group.bench_with_input(
            BenchmarkId::new("parse_frame", &name),
            &frame_bytes,
            |b, frame| b.iter(|| black_box(processor.parse_plugin_frame(frame).unwrap())),
        );
    }

    group.finish();
}

#[cfg(feature = "plugin")]
fn bench_permission_checking(c: &mut Criterion) {
    let mut group = c.benchmark_group("permission_checking");

    let mut registry = PluginRegistry::new();

    // Register plugins with different permission sets
    let permission_sets = vec![
        ("minimal", Permission::RECEIVE_FRAMES),
        (
            "standard",
            Permission::RECEIVE_FRAMES | Permission::SEND_FRAMES,
        ),
        ("privileged", Permission::all()),
    ];

    for (name, perms) in permission_sets {
        let plugin_info = PluginInfo {
            id: 1001,
            name: format!("BenchPlugin_{}", name),
            version: semver::Version::new(1, 0, 0),
            permissions: perms,
            description: "Benchmark plugin".to_string(),
            author: "Benchmark Suite".to_string(),
        };

        registry.register(&plugin_info).unwrap();

        group.bench_with_input(
            BenchmarkId::new("check_permission", name),
            &perms,
            |b, p| b.iter(|| black_box(registry.check_permission(1001, *p).unwrap())),
        );
    }

    group.finish();
}

#[cfg(feature = "plugin")]
fn bench_plugin_handshake(c: &mut Criterion) {
    let mut group = c.benchmark_group("plugin_handshake");

    let runtime = tokio::runtime::Runtime::new().unwrap();

    // Benchmark settings building
    group.bench_function("build_settings", |b| {
        b.iter(|| {
            let registry = PluginRegistry::new();
            let coordinator = PluginHandshakeCoordinator::new(
                registry,
                plugin_support_flags::BASIC_FRAMES,
                0,
                vec![1001, 1002],
                vec![2001, 2002],
            );
            black_box(coordinator.build_plugin_settings())
        })
    });

    // Benchmark settings processing
    let registry = PluginRegistry::new();
    let mut coordinator = PluginHandshakeCoordinator::new(
        registry,
        plugin_support_flags::BASIC_FRAMES,
        0,
        vec![],
        vec![],
    );

    let peer_settings = SettingsFrame {
        settings: vec![Setting {
            id: setting_ids::PLUGIN_SUPPORT,
            value: plugin_support_flags::BASIC_FRAMES,
        }],
    };

    group.bench_function("process_settings", |b| {
        b.to_async(&runtime).iter(|| async {
            black_box(
                coordinator
                    .process_peer_settings(&peer_settings)
                    .await
                    .unwrap(),
            )
        })
    });

    group.finish();
}

#[cfg(feature = "plugin")]
fn bench_plugin_registry_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("plugin_registry");

    let mut registry = PluginRegistry::new();

    // Pre-register some plugins
    for i in 1..=100 {
        let plugin_info = PluginInfo {
            id: i,
            name: format!("Plugin_{}", i),
            version: semver::Version::new(1, 0, 0),
            permissions: Permission::RECEIVE_FRAMES,
            description: format!("Test plugin {}", i),
            author: "Benchmark Suite".to_string(),
        };
        registry.register(&plugin_info).unwrap();
    }

    group.bench_function("register_plugin", |b| {
        let mut plugin_id = 1001;
        b.iter(|| {
            let plugin_info = PluginInfo {
                id: plugin_id,
                name: format!("NewPlugin_{}", plugin_id),
                version: semver::Version::new(1, 0, 0),
                permissions: Permission::RECEIVE_FRAMES,
                description: "Benchmark plugin".to_string(),
                author: "Benchmark Suite".to_string(),
            };
            plugin_id += 1;
            black_box(registry.register(&plugin_info).unwrap());
        })
    });

    group.bench_function("lookup_plugin", |b| {
        b.iter(|| black_box(registry.get_plugin_info(black_box(50)).unwrap()))
    });

    group.bench_function("check_permission", |b| {
        b.iter(|| {
            black_box(
                registry
                    .check_permission(black_box(25), black_box(Permission::RECEIVE_FRAMES))
                    .unwrap(),
            )
        })
    });

    group.finish();
}

#[cfg(feature = "plugin")]
fn bench_frame_type_validation(c: &mut Criterion) {
    let mut group = c.benchmark_group("frame_type_validation");

    let frame_types: Vec<u8> = (0x00u8..=0xFFu8).collect();

    group.bench_function("validate_frame_types", |b| {
        b.iter(|| {
            for &frame_type in &frame_types {
                black_box(PluginFrameProcessor::is_plugin_frame_type(frame_type));
            }
        })
    });

    group.finish();
}

#[cfg(feature = "plugin")]
criterion_group!(
    plugin_benches,
    bench_plugin_header_cbor_encoding,
    bench_plugin_frame_building,
    bench_plugin_frame_processing,
    bench_permission_checking,
    bench_plugin_handshake,
    bench_plugin_registry_operations,
    bench_frame_type_validation,
);

#[cfg(not(feature = "plugin"))]
fn noop_bench(_c: &mut Criterion) {}
#[cfg(not(feature = "plugin"))]
criterion_group!(plugin_benches, noop_bench);

criterion_main!(plugin_benches);
