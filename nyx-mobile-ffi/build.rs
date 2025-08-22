use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Re-run when sources or manifest change
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/lib.rs");
    println!("cargo:rerun-if-changed=Cargo.toml");

    // Generate C header via cbindgen to keep declarations in sync.
    let cratedir = std::env::var("CARGO_MANIFEST_DIR")?;
    let outdir = PathBuf::from(&cratedir).join("include");
    let _ = std::fs::create_dir_all(&outdir);
    let header = outdir.join("nyx_mobile_ffi.h");
    let config = cbindgen::Config::from_root_or_default(cratedir.clone());
    cbindgen::Builder::new()
        .with_crate(cratedir)
        .with_config(config)
        .with_language(cbindgen::Language::C)
        .generate()?
        .write_to_file(header);
    Ok(())
}
