use std::fs;
use std::path::Path;
use std::process::Command;

#[test]
fn test_cli_end_to_end() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = std::env::temp_dir().join(format!("cae_test_{}", std::process::id()));
    if temp_dir.exists() {
        let _ = fs::remove_dir_all(&temp_dir);
    }
    fs::create_dir_all(&temp_dir).expect("Failed to create temp dir");

    // Initialize a git repo in temp_dir
    let run = |cmd: &str, args: &[&str]| {
        let status = Command::new(cmd)
            .args(args)
            .current_dir(&temp_dir)
            .status()
            .expect("Failed to execute command");
        assert!(status.success());
    };

    run("git", &["init"]);
    run("git", &["config", "user.name", "Test User"]);
    run("git", &["config", "user.email", "test@example.com"]);

    // Create manifest files
    let cargo_toml = r#"
[package]
name = "test-pkg"
version = "0.2.1"
edition = "2021"

[dependencies]
serde = "1.0"
"#;
    fs::write(temp_dir.join("Cargo.toml"), cargo_toml).expect("write Cargo.toml");

    let package_json = r#"{
  "name": "test-node",
  "version": "1.0.0",
  "dependencies": {
    "lodash": "^4.17.21"
  }
}"#;
    fs::write(temp_dir.join("package.json"), package_json).expect("write package.json");

    // Create agent context files
    let arch_md = r#"# Architecture Document
This describes the system architecture.

## Component Overview
Details here.
"#;
    fs::write(temp_dir.join("ARCHITECTURE.md"), arch_md).expect("write ARCHITECTURE.md");

    let readme_md = r#"# Readme
Welcome to the test project.
"#;
    fs::write(temp_dir.join("README.md"), readme_md).expect("write README.md");

    let design_md = r#"# Design Specs
Design patterns used in this app.
"#;
    fs::write(temp_dir.join("DESIGN.md"), design_md).expect("write DESIGN.md");

    let memory_md = r#"# Agent Memory
Decisions made during development.
"#;
    fs::write(temp_dir.join("MEMORY.md"), memory_md).expect("write MEMORY.md");

    // Commit files in git
    run("git", &["add", "."]);
    run("git", &["commit", "-m", "Initial test commit"]);

    let bin_path = env!("CARGO_BIN_EXE_cae");

    // 0. cae dir & cae status --dir
    let dir_output = Command::new(bin_path)
    .arg("dir")
    .output()?;
    assert!(dir_output.status.success());
    let dir_stdout = String::from_utf8_lossy(&dir_output.stdout).trim().to_string();
    assert!(dir_stdout.ends_with("projects"));
    assert!(Path::new(&dir_stdout).exists(), "Directory does not exist: {}", dir_stdout);

    let status_dir_output = Command::new(bin_path)
        .args(["status", "--dir"])
        .output()
        .expect("Failed to run cae status --dir");
    assert!(status_dir_output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&status_dir_output.stdout).trim(),
        dir_stdout
    );

    // 1. cae track
    let track_output = Command::new(bin_path)
        .arg("track")
        .arg(&temp_dir)
        .arg("--id")
        .arg("test-integration-proj")
        .output()
        .expect("Failed to run cae track");

    assert!(track_output.status.success(), "cae track failed: {:?}", String::from_utf8_lossy(&track_output.stderr));
    let stdout = String::from_utf8_lossy(&track_output.stdout);
    assert!(stdout.contains("Successfully tracked project"));
    assert!(stdout.contains("Cargo.toml"));
    assert!(stdout.contains("package.json"));
    assert!(stdout.contains("ARCHITECTURE.md"));
    assert!(stdout.contains("README.md"));
    assert!(stdout.contains("DESIGN.md"));
    assert!(stdout.contains("MEMORY.md"));

    // 2. cae status --json
    let status_output = Command::new(bin_path)
        .arg("status")
        .arg("test-integration-proj")
        .arg("--json")
        .output()
        .expect("Failed to run cae status --json");

    assert!(status_output.status.success());
    let json_str = String::from_utf8_lossy(&status_output.stdout);
    assert!(json_str.contains("\"id\": \"test-integration-proj\""));
    assert!(json_str.contains("ARCHITECTURE.md"));
    assert!(json_str.contains("MEMORY.md"));

    // 3. cae record-commit
    let record_output = Command::new(bin_path)
        .arg("record-commit")
        .arg("--project")
        .arg("test-integration-proj")
        .arg("--note")
        .arg("Integration test note")
        .output()
        .expect("Failed to run cae record-commit");

    assert!(record_output.status.success());
    let record_stdout = String::from_utf8_lossy(&record_output.stdout);
    assert!(record_stdout.contains("Successfully recorded commit in ledger"));
    assert!(record_stdout.contains("Integration test note"));

    // 4. Verify status displays recorded commit
    let status2_output = Command::new(bin_path)
        .arg("status")
        .arg("test-integration-proj")
        .output()
        .expect("Failed to run cae status");
    let status2_stdout = String::from_utf8_lossy(&status2_output.stdout);
    assert!(status2_stdout.contains("Recorded Commits in Ledger (1)"));
    assert!(status2_stdout.contains("Integration test note"));

    // 5. cae untrack
    let untrack_output = Command::new(bin_path)
        .arg("untrack")
        .arg("test-integration-proj")
        .output()
        .expect("Failed to run cae untrack");

    assert!(untrack_output.status.success());
    let untrack_stdout = String::from_utf8_lossy(&untrack_output.stdout);
    assert!(untrack_stdout.contains("test-integration-proj"));

    // Clean up
    let _ = fs::remove_dir_all(&temp_dir);
}
