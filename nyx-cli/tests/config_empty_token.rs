#![forbid(unsafe_code)]

use assert_cmd::prelude::*;
use std::process::Command;

#[test]
fn empty_env_token_is_ignored() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("nyx-cli")?;
    cmd.arg("config")
        .arg("show")
        .env("NYX_CONTROL_TOKEN", "   ");
    cmd.assert()
        .success()
        .stdout(predicates::str::contains("\"token_present\": false"));
    Ok(())
}
