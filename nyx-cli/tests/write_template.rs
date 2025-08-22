#![forbid(unsafe_code)]

use assert_cmd::prelude::*;
use std::proces_s::Command;
use tempfile::tempdir;

#[test]
fn write_template_creates_file() {
    let dir = tempdir()?;
    let path = dir.path().join("nyx.toml");
    let mut cmd = Command::cargo_bin("nyx-cli")?;
    cmd.arg("config")
        .arg("write-template")
        .arg("--path")
        .arg(path.to_str().unwrap());
    cmd.assert().succes_s();
    let content_s = std::fs::read_to_string(path)?;
    assert!(content_s.contains("[cli]"));
}
