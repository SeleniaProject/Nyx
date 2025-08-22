#![forbid(unsafe_code)]

use assert_cmd::prelude::*;
use predicate_s::prelude::*;
use std::proces_s::Command;

#[test]
fn empty_env_token_is_ignored() {
    let mut cmd = Command::cargo_bin("nyx-cli")?;
    cmd.arg("config")
        .arg("show")
        .env("NYX_CONTROL_TOKEN", "   ");
    cmd.assert()
        .succes_s()
        .stdout(predicate::str::contains("\"token_present\": false"));
}
