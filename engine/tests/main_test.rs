use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

fn scratch_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("transmute_test_{}_{}", std::process::id(), name));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn write_coverage_for(rb_path: &Path, line: u32, cov_path: &Path) {
    let key = if rb_path.is_absolute() {
        format!("{}:{}", rb_path.display(), line)
    } else {
        let cwd = std::env::current_dir().unwrap();
        format!("{}/{}:{}", cwd.display(), rb_path.display(), line)
    };
    let content = format!(r#"{{"{}": ["dummy_spec.rb"]}}"#, key);
    std::fs::write(cov_path, content).unwrap();
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
fn test_writes_json_with_failure_count_to_custom_output_path(
) -> Result<(), Box<dyn std::error::Error>> {
    let dir = scratch_dir("output_path");
    let rb_path = dir.join("scratch.rb");
    std::fs::write(&rb_path, "puts 42\n").unwrap();
    let cov_path = dir.join("cov.json");
    std::fs::write(&cov_path, "{}").unwrap();
    let output_path = dir.join("custom.json");

    let mut cmd = Command::cargo_bin("transmute")?;
    cmd.arg("--coverage").arg(cov_path.to_str().unwrap());
    cmd.arg("--files").arg(rb_path.to_str().unwrap());
    cmd.arg("--command").arg("sh -c true");
    cmd.arg("--output").arg(output_path.to_str().unwrap());
    cmd.arg("--log-level").arg("warn");

    cmd.assert().success();
    assert!(
        output_path.exists(),
        "Output should be written to specified --output path"
    );

    let content = std::fs::read_to_string(&output_path).unwrap();
    assert!(
        content.contains(r#""failures""#),
        "JSON output should include the failures count; got: {}",
        content
    );

    std::fs::remove_dir_all(&dir).ok();
    Ok(())
}

#[test]
fn test_fail_fast_restores_source_before_exit() -> Result<(), Box<dyn std::error::Error>> {
    let dir = scratch_dir("fail_fast");
    let rb_path = dir.join("scratch.rb");
    let original = "puts 42\n";
    std::fs::write(&rb_path, original).unwrap();
    let cov_path = dir.join("cov.json");
    write_coverage_for(&rb_path, 1, &cov_path);

    let mut cmd = Command::cargo_bin("transmute")?;
    cmd.arg("--coverage").arg(&cov_path);
    cmd.arg("--files").arg(&rb_path);
    cmd.arg("--command").arg("sh -c true");
    cmd.arg("--fail-fast");
    cmd.arg("--log-level").arg("warn");

    cmd.assert().failure();

    let after = std::fs::read_to_string(&rb_path).unwrap();
    assert_eq!(
        after, original,
        "Source must be restored before --fail-fast exits"
    );

    std::fs::remove_dir_all(&dir).ok();
    Ok(())
}

#[test]
fn test_sigint_during_run_restores_source() -> Result<(), Box<dyn std::error::Error>> {
    let dir = scratch_dir("sigint");
    let rb_path = dir.join("scratch.rb");
    let original = "puts 42\n";
    std::fs::write(&rb_path, original).unwrap();
    let cov_path = dir.join("cov.json");
    write_coverage_for(&rb_path, 1, &cov_path);

    let binary = assert_cmd::cargo::cargo_bin("transmute");
    let mut child = std::process::Command::new(binary)
        .arg("--coverage")
        .arg(&cov_path)
        .arg("--files")
        .arg(&rb_path)
        .arg("--command")
        .arg("sleep 30")
        .arg("--log-level")
        .arg("warn")
        .spawn()?;

    let start = std::time::Instant::now();
    loop {
        if std::fs::read_to_string(&rb_path).unwrap() != original {
            break;
        }
        if start.elapsed() > Duration::from_secs(5) {
            child.kill().ok();
            panic!("mutation was not applied within 5s");
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    std::process::Command::new("kill")
        .arg("-INT")
        .arg(child.id().to_string())
        .status()?;

    let _ = child.wait();

    let after = std::fs::read_to_string(&rb_path).unwrap();
    assert_eq!(after, original, "Source must be restored after SIGINT");

    std::fs::remove_dir_all(&dir).ok();
    Ok(())
}

#[test]
fn test_malformed_coverage_json_exits_cleanly() -> Result<(), Box<dyn std::error::Error>> {
    let dir = scratch_dir("invalid_json");
    let cov_path = dir.join("bad.json");
    std::fs::write(&cov_path, "not json").unwrap();
    let rb_path = dir.join("scratch.rb");
    std::fs::write(&rb_path, "puts 42\n").unwrap();

    let output = Command::cargo_bin("transmute")?
        .arg("--coverage")
        .arg(&cov_path)
        .arg("--files")
        .arg(&rb_path)
        .arg("--command")
        .arg("sh -c true")
        .arg("--log-level")
        .arg("warn")
        .output()?;

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "Should exit non-zero; status: {:?}",
        output.status
    );
    assert!(
        !stderr.contains("panicked at") && !stderr.contains("RUST_BACKTRACE"),
        "Should exit cleanly, not panic; stderr: {}",
        stderr
    );
    assert!(
        stderr.to_lowercase().contains("coverage"),
        "Error message should mention coverage; stderr: {}",
        stderr
    );

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
