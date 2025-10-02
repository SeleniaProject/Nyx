// Integration test module for Nyx end-to-end tests
//
// This module provides test infrastructure for multi-node simulations,
// handshake validation, multipath data transfer, and cover traffic measurement.
//
// Reference: spec/testing/*.md, TODO.md §9.1

pub mod test_harness;

// Integration test modules
pub mod integration;

// Re-export common utilities for integration tests
pub use test_harness::{
    ClientHandle, DaemonConfig, DaemonHandle, NetworkConfig, TestHarness, TestNetwork,
    TestResult,
};
