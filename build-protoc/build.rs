use std::{env, fs, path::Path};

fn main() {
    // If caller already provides PROTOC, respect it and exit early.
    if env::var_os("PROTOC").is_some() {
        return;
    }

    // Always resolve vendored protoc first. If anything fails afterwards,
    // fall back to using this path directly to avoid build panics.
    let vendored = match protoc_bin_vendored::protoc_bin_path() {
        Ok(p) => p,
        Err(_) => {
            // As a last resort, leave PROTOC unset and return gracefully.
            // Downstream build scripts may handle absence differently.
            return;
        }
    };

    // Try to install a copy to CARGO_HOME/bin when available, but never panic.
    let dest = match env::var("CARGO_HOME")
        .ok()
        .map(|cargo_home| Path::new(&cargo_home).join("bin").join("protoc"))
    {
        Some(p) => {
            // Best-effort create/copy; ignore errors to keep builds resilient across CI/OSes.
            if let Some(parent) = p.parent() {
                let _ = fs::create_dir_all(parent);
            }
            if !p.exists() {
                let _ = fs::copy(&vendored, &p);
            }
            p
        }
        None => {
            // Windows often lacks HOME; do not attempt to infer. Use vendored path directly.
            vendored.clone()
        }
    };

    // Prefer the copied destination if it exists, otherwise use vendored path directly.
    let protoc_path = if dest.exists() { dest } else { vendored };
    println!("cargo:rustc-env=PROTOC={}", protoc_path.display());
}
