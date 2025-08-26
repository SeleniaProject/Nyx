#![no_main]

use libfuzzer_sys::fuzz_target;
use nyx_core::path::{PathBuilder, PathConfig, PathQuality};
use nyx_core::types::{NodeId, Timestamp};

fuzz_target!(|data: &[u8]| {
    // Skip inputs that are too small for meaningful path building
    if data.len() < 16 {
        return;
    }

    // Parse node count from first byte (1-7 nodes for valid paths)
    let node_count = ((data[0] % 7) + 1) as usize;
    if data.len() < node_count * 8 {
        return;
    }

    // Create path configuration
    let config = PathConfig {
        min_path_length: 3,
        max_path_length: 7,
        quality_threshold: 0.5,
        latency_weight: 0.4,
        bandwidth_weight: 0.3,
        reliability_weight: 0.3,
        enable_diversity: true,
        max_retries: 3,
    };

    let mut path_builder = PathBuilder::new(config);

    // Generate node IDs from input data
    let mut nodes = Vec::new();
    for i in 0..node_count {
        let offset = i * 8;
        if offset + 8 <= data.len() {
            let node_bytes = &data[offset..offset + 8];
            let node_id = u64::from_le_bytes([
                node_bytes[0], node_bytes[1], node_bytes[2], node_bytes[3],
                node_bytes[4], node_bytes[5], node_bytes[6], node_bytes[7],
            ]);
            nodes.push(NodeId::new(node_id));
        }
    }

    // Test path building with available nodes
    if nodes.len() >= 3 {
        // Try to build a path
        let path_result = path_builder.build_path(&nodes);
        
        match path_result {
            Ok(path) => {
                // Validate the built path
                let _ = path_builder.validate_path(&path);
                
                // Test path quality calculation
                let quality = PathQuality::calculate(&path);
                let _ = quality.overall_score();
                
                // Test path optimization if possible
                if let Ok(optimized) = path_builder.optimize_path(&path) {
                    let _ = optimized;
                }
            }
            Err(_) => {
                // Path building failed, which is acceptable with fuzzing input
            }
        }
    }

    // Test backup path generation if enough data
    if data.len() > node_count * 8 + 4 {
        let remaining_data = &data[node_count * 8..];
        if remaining_data.len() >= 4 {
            let backup_count = (remaining_data[0] % 3) + 1; // 1-3 backup paths
            let _ = path_builder.generate_backup_paths(&nodes, backup_count as usize);
        }
    }

    // Test path selection algorithms
    if nodes.len() > 5 {
        // Test diverse path selection
        let _ = path_builder.select_diverse_paths(&nodes, 2);
        
        // Test quality-based selection
        let _ = path_builder.select_best_quality_path(&nodes);
    }
});
