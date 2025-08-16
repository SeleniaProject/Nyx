#![forbid(unsafe_code)]

use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::process::Command;
use tempfile::tempdir;

#[test]
fn write_template_creates_file() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("nyx.toml");
    let mut cmd = Command::cargo_bin("nyx-cli").unwrap();
    cmd.arg("config").arg("write-template").arg("--path").arg(path.to_str().unwrap());
    cmd.assert().success();
    let contents = std::fs::read_to_string(path).unwrap();
    assert!(contents.contains("[cli]"));
}
