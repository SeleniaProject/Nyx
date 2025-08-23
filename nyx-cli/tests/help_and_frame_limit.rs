#![forbid(unsafe_code)]

use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::process::Command;

#[test]
fn help_shows_cliname() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("nyx-cli")?;
    cmd.arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("nyx-cli"));
    Ok(())
}

#[test]
fn frame_limit_rejects_out_of_range() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("nyx-cli")?;
    cmd.args(["frame-limit", "--set", "999999999"]);
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("invalid frame limit"));
    Ok(())
}
