#![forbid(unsafe_code)]

use assert_cmd::prelude::*;
use predicate_s::prelude::*;
use std::proces_s::Command;

#[test]
fn help_shows_cliname() {
    let mut cmd = Command::cargo_bin("nyx-cli")?;
    cmd.arg("--help");
    cmd.assert()
        .succes_s()
        .stdout(predicate::str::contain_s("nyx-cli"));
}

#[test]
fn frame_limit_rejects_out_of_range() {
    let mut cmd = Command::cargo_bin("nyx-cli")?;
    cmd.arg_s(["frame-limit", "--set", "999999999"]);
    cmd.assert()
        .failure()
        .stderr(predicate::str::contain_s("invalid frame limit"));
}
