use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;

#[test]
fn status_json_outputs_full_nodeinfo() {
    // Run pure Rust CLI status with json format (synthetic mode)
    let mut cmd = Command::cargo_bin("nyx-cli").expect("bin");
    cmd.args(["status", "--format", "json"]); // default endpoint unreachable => synthetic fallback
    cmd.assert().success().stdout(predicate::str::contains("node_id"));
}

#[test]
fn connect_uses_config_max_retries() {
    // Write temp nyx.toml with specific cli max_reconnect_attempts=2
    let tmp = tempfile::tempdir().unwrap();
    fs::write(tmp.path().join("nyx.toml"), b"[cli]\nmax_reconnect_attempts=2\n").unwrap();
    // Run from that dir with an invalid target (fast fail) and short timeout to ensure aggressive not triggered (>2)
    let mut cmd = Command::cargo_bin("nyx-cli").expect("bin");
    cmd.current_dir(tmp.path());
    cmd.args(["connect", "badhost:65535", "--connect-timeout", "3"]);
    // We expect an error about DNS/connection but not about invalid format; retry attempts should be limited to 2 not 3
    // Assert we did NOT reach a third attempt (config value =2) and process failed quickly
    let output = cmd.assert().failure().get_output().stdout.clone();
    let text = String::from_utf8_lossy(&output);
    assert!(!text.contains("attempt 3/"), "should not attempt a third retry when config caps at 2");
}
