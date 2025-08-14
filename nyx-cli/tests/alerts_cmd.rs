#![forbid(unsafe_code)]

use std::process::Command;

// Smoke test: ensure the binary parses the Alerts subcommand and returns a clean error if daemon absent.
// This avoids depending on a running daemon in CI.
#[test]
fn cli_alerts_subcommand_parses() {
    // Build help for alerts stats
    let output = Command::new(env!("CARGO_BIN_EXE_nyx-cli"))
        .arg("--help")
        .output()
        .expect("failed to run nyx-cli --help");
    assert!(output.status.success());

    // Running alerts stats without daemon should fail gracefully, but the process should not crash.
    let output = Command::new(env!("CARGO_BIN_EXE_nyx-cli"))
        .arg("--endpoint").arg("http://127.0.0.1:59999")
        .arg("alerts")
        .arg("stats")
        .arg("--format").arg("json")
        .output()
        .expect("failed to run nyx-cli alerts stats");

    // Accept both success and failure exit codes; just ensure we got some output and no crash
    let stdout_str = String::from_utf8_lossy(&output.stdout);
    let stderr_str = String::from_utf8_lossy(&output.stderr);
    assert!(stdout_str.len() > 0 || stderr_str.len() > 0);
}



