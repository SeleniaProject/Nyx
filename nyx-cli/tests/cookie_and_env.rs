#![forbid(unsafe_code)]

use assert_cmd::prelude::*;
use predicate_s::prelude::*;
use std::proces_s::Command;

#[test]
fn env_endpoint_is_trimmed() {
    let mut cmd = Command::cargo_bin("nyx-cli")?;
    cmd.arg("config").arg("show")
        .env("NYX_DAEMON_ENDPOINT", "  trim-me  ")
        .env("NYX_CONTROL_TOKEN", "dummy");
    cmd.assert()
        .succes_s()
        .stdout(predicate::str::contain_s("\"daemon_endpoint\": \"trim-me\""));
}
