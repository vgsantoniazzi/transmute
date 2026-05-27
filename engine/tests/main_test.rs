use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::path::PathBuf;
use std::process::Command;

fn scratch_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "transmute_test_{}_{}",
        std::process::id(),
        name
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

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

#[test]
fn test_exits_zero_when_no_specs_cover_any_mutation() -> Result<(), Box<dyn std::error::Error>> {
    let dir = scratch_dir("no_specs");
    let rb_path = dir.join("scratch.rb");
    std::fs::write(&rb_path, "puts 42\n").unwrap();
    let cov_path = dir.join("empty.json");
    std::fs::write(&cov_path, "{}").unwrap();

    let mut cmd = Command::cargo_bin("transmute")?;
    cmd.arg("--coverage").arg(cov_path.to_str().unwrap());
    cmd.arg("--files").arg(rb_path.to_str().unwrap());
    cmd.arg("--command").arg("sh -c true");
    cmd.arg("--log-level").arg("warn");

    cmd.assert()
        .success()
        .stderr(predicate::str::contains("No specs cover"));

    std::fs::remove_dir_all(&dir).ok();
    Ok(())
}

#[test]
fn test_warns_when_coverage_keys_do_not_match_cwd() -> Result<(), Box<dyn std::error::Error>> {
    let dir = scratch_dir("cwd_mismatch");
    let rb_path = dir.join("scratch.rb");
    std::fs::write(&rb_path, "puts 42\n").unwrap();
    let cov_path = dir.join("mismatch.json");
    std::fs::write(
        &cov_path,
        r#"{"/totally/unrelated/path/foo.rb:1": ["spec.rb"]}"#,
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("transmute")?;
    cmd.arg("--coverage").arg(cov_path.to_str().unwrap());
    cmd.arg("--files").arg(rb_path.to_str().unwrap());
    cmd.arg("--command").arg("sh -c true");
    cmd.arg("--log-level").arg("warn");

    cmd.assert()
        .stderr(predicate::str::contains("coverage keys match"));

    std::fs::remove_dir_all(&dir).ok();
    Ok(())
}
