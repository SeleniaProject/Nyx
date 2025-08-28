#![forbid(unsafe_code)]

use assert_cmd::prelude::*;
use std::process::Command;
use tempfile::tempdir;

#[test]
fn write_template_creates_file() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempdir()?;
    let path = dir.path().join("nyx.toml");
    let mut cmd = Command::cargo_bin("nyx-cli")?;
    cmd.arg("config")
        .arg("write-template")
        .arg("--path")
        .arg(path.to_str().unwrap());
    cmd.assert().success();
    let contents = std::fs::read_to_string(path)?;
    assert!(contents.contains("[cli]"));
    Ok(())
}
