//! Analyze Criterion benchmark output for hpke_rekey_overhead and produce CSV / summary.
//! Usage: cargo run --package scripts --bin analyze_hpke_rekey_bench -- <criterion_dir>
//! It scans target/criterion/hpke_rekey_overhead/*/new/estimates.json extracting mean time per iter.

use std::{fs, path::{Path, PathBuf}};
use serde::Deserialize;

#[derive(Deserialize)]
struct Estimates { mean: StatValue }
#[derive(Deserialize)]
struct StatValue { point_estimate: f64 }

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let crit_dir = if args.len() > 1 { PathBuf::from(&args[1]) } else { PathBuf::from("target/criterion") };
    let bench_root = crit_dir.join("hpke_rekey_overhead");
    if !bench_root.exists() { anyhow::bail!("bench root {:?} not found", bench_root); }
    println!("profile,mean_ns_per_iter");
    for entry in fs::read_dir(&bench_root)? {
        let e = entry?; if !e.file_type()?.is_dir() { continue; }
        let name = e.file_name().to_string_lossy().to_string();
        // skip base summary directories
        let est = e.path().join("new").join("estimates.json");
        if !est.exists() { continue; }
        if let Ok(txt) = fs::read_to_string(&est) {
            if let Ok(estimates) = serde_json::from_str::<Estimates>(&txt) {
                println!("{},{}", name, estimates.mean.point_estimate);
            }
        }
    }
    Ok(())
}
