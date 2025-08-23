#![forbid(unsafe_code)]

use assert_cmd::prelude::*;
use std::process::Command;

#[test]
fn config_show_resolves_env() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("nyx-cli")?;
    cmd.arg("config")
        .arg("show")
        .env("NYX_DAEMON_ENDPOINT", "test-endpoint")
        .env("NYX_REQUEST_TIMEOUT_MS", "1234")
        .env("NYX_CONTROL_TOKEN", "secret");
    cmd.assert()
        .success()
        .stdout(predicates::str::contains(
            "\"daemon_endpoint\": \"test-endpoint\"",
        ))
        .stdout(predicates::str::contains("\"request_timeout_ms\": 1234"))
        .stdout(predicates::str::contains("\"token_present\": true"));
    Ok(())
}
