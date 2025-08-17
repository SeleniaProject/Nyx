use std::path::PathBuf;

fn main() {
	// Re-run when sources or manifest change
	println!("cargo:rerun-if-changed=build.rs");
	println!("cargo:rerun-if-changed=src/lib.rs");
	println!("cargo:rerun-if-changed=Cargo.toml");

	// Generate C header via cbindgen to keep declarations in sync.
	let crate_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
	let out_dir = PathBuf::from(&crate_dir).join("include");
	let _ = std::fs::create_dir_all(&out_dir);
	let header = out_dir.join("nyx_mobile_ffi.h");
	let config = cbindgen::Config::from_root_or_default(crate_dir.clone());
	cbindgen::Builder::new()
		.with_crate(crate_dir)
		.with_config(config)
		.with_language(cbindgen::Language::C)
		.generate()
		.expect("Unable to generate bindings")
		.write_to_file(header);
}

