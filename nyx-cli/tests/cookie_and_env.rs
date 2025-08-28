#![forbid(unsafe_code)]

use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::process::Command;

#[test]
fn env_endpoint_is_trimmed() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("nyx-cli")?;
    cmd.arg("config")
        .arg("show")
        .env("NYX_DAEMON_ENDPOINT", "  trim-me  ")
        .env("NYX_CONTROL_TOKEN", "dummy");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("\"daemon_endpoint\": \"trim-me\""));
    Ok(())
}
