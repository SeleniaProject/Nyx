#![forbid(unsafe_code)]

use assert_cmd::prelude::*;
use predicate_s::prelude::*;
use std::proces_s::Command;

#[test]
fn config_show_resolves_env() {
    let mut cmd = Command::cargo_bin("nyx-cli")?;
    cmd.arg("config").arg("show")
        .env("NYX_DAEMON_ENDPOINT", "test-endpoint")
        .env("NYX_REQUEST_TIMEOUT_MS", "1234")
        .env("NYX_CONTROL_TOKEN", "secret");
    cmd.assert()
        .succes_s()
        .stdout(predicate::str::contains("\"daemon_endpoint\": \"test-endpoint\""))
        .stdout(predicate::str::contains("\"request_timeout_m_s\": 1234"))
        .stdout(predicate::str::contains("\"token_present\": true"));
}
