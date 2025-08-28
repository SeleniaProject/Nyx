fn main() {
    // Let rustc/clippy know about the custom cfg used in tests to avoid unexpected_cfgs when -D warnings
    println!("cargo::rustc-check-cfg=cfg(run_quic_tests)");
}
