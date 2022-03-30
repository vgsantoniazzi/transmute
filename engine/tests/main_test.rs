use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::process::Command;

#[test]
fn test_required_files() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("transmute")?;

    cmd.arg("--files").arg("**/*.rb");

    cmd.assert().failure().stderr(predicate::str::contains(
        "error: The following required arguments were not provided:\n    --command <COMMAND>",
    ));

    Ok(())
}

#[test]
fn test_required_command() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("transmute")?;

    cmd.arg("--command")
        .arg("docker-compose run web rspec {file}");

    cmd.assert().failure().stderr(predicate::str::contains(
        "error: The following required arguments were not provided:\n    --files <FILES>",
    ));

    Ok(())
}

#[test]
fn test_logging() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("transmute")?;

    cmd.arg("--command")
        .arg("docker-compose run web rspec {file}");
    cmd.arg("--files").arg("**/*.rb");
    cmd.arg("--coverage").arg("tests/fixtures/transmute.json");
    cmd.arg("--log-level").arg("info");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Starting transmute"));

    Ok(())
}
