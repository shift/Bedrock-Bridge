use std::process::Command;

fn cli() -> Command {
    let bin = env!("CARGO_BIN_EXE_bedrock-bridge");
    Command::new(bin)
}

/// Helper: create a temp dir for isolated profile storage
fn temp_store_dir() -> tempfile::TempDir {
    tempfile::tempdir().expect("create temp dir")
}

/// Helper: run CLI with XDG_CONFIG_HOME set to temp dir
fn cli_with_store(store_dir: &std::path::Path) -> Command {
    let mut cmd = cli();
    cmd.env("XDG_CONFIG_HOME", store_dir);
    cmd
}

#[test]
fn test_help_flag() {
    let output = cli().arg("--help").output().expect("run help");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("UDP relay"));
    assert!(stdout.contains("run"));
    assert!(stdout.contains("profiles"));
}

#[test]
fn test_profiles_list_empty() {
    let dir = temp_store_dir();
    let output = cli_with_store(dir.path())
        .args(["profiles", "list"])
        .output()
        .expect("run profiles list");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("No profiles") || stdout.trim().is_empty());
}

#[test]
fn test_profiles_add_and_list() {
    let dir = temp_store_dir();

    // Add a profile
    let output = cli_with_store(dir.path())
        .args(["profiles", "add", "--label", "Test Server", "--host", "10.0.0.1", "--port", "19132"])
        .output()
        .expect("run profiles add");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Added profile"));
    assert!(stdout.contains("Test Server"));

    // List should show it
    let output = cli_with_store(dir.path())
        .args(["profiles", "list"])
        .output()
        .expect("run profiles list");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Test Server"));
    assert!(stdout.contains("10.0.0.1"));
}

#[test]
fn test_profiles_remove() {
    let dir = temp_store_dir();

    // Add then remove
    cli_with_store(dir.path())
        .args(["profiles", "add", "--label", "To Remove", "--host", "1.2.3.4"])
        .output()
        .expect("add");

    let output = cli_with_store(dir.path())
        .args(["profiles", "remove", "To Remove"])
        .output()
        .expect("remove");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Removed"));

    // List should be empty
    let output = cli_with_store(dir.path())
        .args(["profiles", "list"])
        .output()
        .expect("list");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("No profiles") || !stdout.contains("To Remove"));
}

#[test]
fn test_profiles_remove_nonexistent() {
    let dir = temp_store_dir();
    let output = cli_with_store(dir.path())
        .args(["profiles", "remove", "nonexistent"])
        .output()
        .expect("remove nonexistent");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("not found") || stderr.contains("error"));
}

#[test]
fn test_run_help() {
    let output = cli().args(["run", "--help"]).output().expect("run help");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--label"));
    assert!(stdout.contains("--host"));
    assert!(stdout.contains("--port"));
}

#[test]
fn test_profiles_add_missing_host() {
    let dir = temp_store_dir();
    let output = cli_with_store(dir.path())
        .args(["profiles", "add", "--label", "No Host"])
        .output()
        .expect("add missing host");
    // clap should reject missing required --host
    assert!(!output.status.success());
}

#[test]
fn test_version_flag() {
    let output = cli().arg("--version").output().expect("version");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("0.1.0"));
}
