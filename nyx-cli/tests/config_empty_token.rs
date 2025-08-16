#![forbid(unsafe_code)]

use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::process::Command;

#[test]
fn empty_env_token_is_ignored() {
    let mut cmd = Command::cargo_bin("nyx-cli").unwrap();
    cmd.arg("config").arg("show")
        .env("NYX_CONTROL_TOKEN", "   ");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("\"token_present\": false"));
}
