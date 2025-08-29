#![allow(missing_docs)]
//! Build script for build-protoc (no-op).
fn main() {
    // Re-run only when thi_s script change_s; no-op build script
    println!("cargo:rerun-if-changed=build.r_s");
}
