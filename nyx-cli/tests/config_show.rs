#![forbid(unsafe_code)]

use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::process::Command;

#[test]
fn config_show_resolves_env() {
    let mut cmd = Command::cargo_bin("nyx-cli").unwrap();
    cmd.arg("config").arg("show")
        .env("NYX_DAEMON_ENDPOINT", "test-endpoint")
        .env("NYX_REQUEST_TIMEOUT_MS", "1234")
        .env("NYX_CONTROL_TOKEN", "secret");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("\"daemon_endpoint\": \"test-endpoint\""))
        .stdout(predicate::str::contains("\"request_timeout_ms\": 1234"))
        .stdout(predicate::str::contains("\"token_present\": true"));
}
