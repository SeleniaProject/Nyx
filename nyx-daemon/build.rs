#![allow(missing_docs)]
//! Build script for nyx-daemon (no-op).
fn main() {
    // Re-run only when this script changes; no-op build script
    println!("cargo:rerun-if-changed=build.rs");
}
