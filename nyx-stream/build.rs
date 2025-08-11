fn main() {
    // Register custom cfg so rustc does not warn about `#[cfg(disabled)]` in test stubs
    println!("cargo::rustc-check-cfg=cfg(disabled)");
}
