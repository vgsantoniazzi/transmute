mod common;

use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

fn read_to_pretty<P: AsRef<Path>>(p: P) -> String {
    std::fs::read_to_string(p).unwrap()
}

fn scratch_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("transmute_test_{}_{}", std::process::id(), name));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn write_coverage_for(rb_path: &Path, line: u32, cov_path: &Path) {
    let key = if rb_path.is_absolute() {
        rb_path.display().to_string()
    } else {
        let cwd = std::env::current_dir().unwrap();
        format!("{}/{}", cwd.display(), rb_path.display())
    };
    common::write_fixture(cov_path, &[(key.as_str(), line, &["dummy_spec.rb"])]);
}

fn write_empty_coverage(cov_path: &Path) {
    common::write_fixture(cov_path, &[]);
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
    cmd.arg("--coverage").arg("tests/fixtures/transmute.sqlite");
    cmd.arg("--log-level").arg("info");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Starting transmute"));

    Ok(())
}

#[test]
fn test_uncovered_mutation_is_recorded_as_surviving() -> Result<(), Box<dyn std::error::Error>> {
    let dir = scratch_dir("no_specs");
    let rb_path = dir.join("scratch.rb");
    std::fs::write(&rb_path, "puts 42\n").unwrap();
    let cov_path = dir.join("empty.sqlite");
    write_empty_coverage(&cov_path);

    let mut cmd = Command::cargo_bin("transmute")?;
    cmd.arg("--coverage").arg(cov_path.to_str().unwrap());
    cmd.arg("--files").arg(rb_path.to_str().unwrap());
    cmd.arg("--command").arg("sh -c true");
    cmd.arg("--log-level").arg("warn");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("recording as surviving"));

    std::fs::remove_dir_all(&dir).ok();
    Ok(())
}

#[test]
fn test_writes_json_with_failure_count_to_custom_output_path(
) -> Result<(), Box<dyn std::error::Error>> {
    let dir = scratch_dir("output_path");
    let rb_path = dir.join("scratch.rb");
    std::fs::write(&rb_path, "puts 42\n").unwrap();
    let cov_path = dir.join("cov.sqlite");
    write_empty_coverage(&cov_path);
    let output_path = dir.join("custom.json");

    let mut cmd = Command::cargo_bin("transmute")?;
    cmd.arg("--coverage").arg(cov_path.to_str().unwrap());
    cmd.arg("--files").arg(rb_path.to_str().unwrap());
    cmd.arg("--command").arg("sh -c true");
    cmd.arg("--output").arg(output_path.to_str().unwrap());
    cmd.arg("--log-level").arg("warn");

    cmd.assert().failure();
    assert!(
        output_path.exists(),
        "Output should be written to specified --output path"
    );

    let content = std::fs::read_to_string(&output_path).unwrap();
    let json: serde_json::Value =
        serde_json::from_str(&content).expect("output must be valid JSON");
    assert_eq!(
        json["failures"], 1,
        "Exactly one surviving mutation expected (uncovered '42' on line 1); JSON: {}",
        content
    );
    assert!(
        json["analytics"]["mutations"].is_array(),
        "analytics.mutations must be present as an array; JSON: {}",
        content
    );

    std::fs::remove_dir_all(&dir).ok();
    Ok(())
}

#[test]
fn test_all_infra_runs_emit_inconclusive_warning() -> Result<(), Box<dyn std::error::Error>> {
    let dir = scratch_dir("infra_only");
    let rb_path = dir.join("scratch.rb");
    std::fs::write(&rb_path, "puts 42\n").unwrap();
    let cov_path = dir.join("cov.sqlite");
    write_coverage_for(&rb_path, 1, &cov_path);
    let output_path = dir.join("result.json");

    let output = Command::cargo_bin("transmute")?
        .arg("--coverage")
        .arg(&cov_path)
        .arg("--files")
        .arg(&rb_path)
        .arg("--command")
        .arg("sh -c 'exit 127'")
        .arg("--output")
        .arg(&output_path)
        .arg("--log-level")
        .arg("warn")
        .output()?;

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Report is inconclusive"),
        "Infra-only run must surface inconclusive warning; stderr: {}",
        stderr
    );

    let content = std::fs::read_to_string(&output_path).unwrap();
    let json: serde_json::Value = serde_json::from_str(&content)?;
    assert_eq!(
        json["failures"], 0,
        "Infra-only outcomes must not be counted as survivors; JSON: {}",
        content
    );

    std::fs::remove_dir_all(&dir).ok();
    Ok(())
}

#[test]
fn test_unknown_formatter_exits_with_code_2() -> Result<(), Box<dyn std::error::Error>> {
    let dir = scratch_dir("bad_formatter");
    let rb_path = dir.join("scratch.rb");
    std::fs::write(&rb_path, "puts 42\n").unwrap();
    let cov_path = dir.join("cov.sqlite");
    write_empty_coverage(&cov_path);

    let output = Command::cargo_bin("transmute")?
        .arg("--coverage")
        .arg(&cov_path)
        .arg("--files")
        .arg(&rb_path)
        .arg("--command")
        .arg("sh -c true")
        .arg("--formatter")
        .arg("pdf")
        .arg("--log-level")
        .arg("warn")
        .output()?;

    assert_eq!(
        output.status.code(),
        Some(2),
        "Unknown --formatter must exit with code 2 (argparse infra)"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("unknown --formatter"),
        "Error must name the unknown formatter; stderr: {}",
        stderr
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
    let cov_path = dir.join("cov.sqlite");
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
    let cov_path = dir.join("cov.sqlite");
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
fn test_invalid_coverage_db_exits_cleanly() -> Result<(), Box<dyn std::error::Error>> {
    let dir = scratch_dir("invalid_db");
    let cov_path = dir.join("bad.sqlite");
    std::fs::write(&cov_path, "not a sqlite file").unwrap();
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
    assert_eq!(
        output.status.code(),
        Some(2),
        "Invalid coverage DB must exit with code 2; status: {:?}",
        output.status
    );
    assert!(
        !stderr.contains("panicked at") && !stderr.contains("RUST_BACKTRACE"),
        "Should exit cleanly, not panic; stderr: {}",
        stderr
    );
    assert!(
        stderr.contains("not a valid transmute database")
            || stderr.contains("unable to open coverage file"),
        "Error message must explain the load failure; stderr: {}",
        stderr
    );

    std::fs::remove_dir_all(&dir).ok();
    Ok(())
}

#[test]
fn test_seed_produces_deterministic_mutation_replacements() -> Result<(), Box<dyn std::error::Error>>
{
    let dir = scratch_dir("seed_repro");
    let rb_path = dir.join("scratch.rb");
    std::fs::write(&rb_path, "puts 42\n").unwrap();
    let cov_path = dir.join("cov.sqlite");
    write_empty_coverage(&cov_path);

    let run = |out: &Path| {
        Command::cargo_bin("transmute")
            .unwrap()
            .arg("--coverage")
            .arg(&cov_path)
            .arg("--files")
            .arg(&rb_path)
            .arg("--command")
            .arg("sh -c true")
            .arg("--output")
            .arg(out)
            .arg("--seed")
            .arg("42")
            .arg("--log-level")
            .arg("warn")
            .output()
            .unwrap()
    };

    let out_a = dir.join("a.json");
    let out_b = dir.join("b.json");
    let _ = run(&out_a);
    let _ = run(&out_b);

    let a = read_to_pretty(&out_a);
    let b = read_to_pretty(&out_b);
    assert_eq!(a, b, "Same --seed should produce identical reports");

    std::fs::remove_dir_all(&dir).ok();
    Ok(())
}

#[test]
fn test_warns_when_no_coverage_files_match_cwd() -> Result<(), Box<dyn std::error::Error>> {
    let dir = scratch_dir("cwd_mismatch");
    let rb_path = dir.join("scratch.rb");
    std::fs::write(&rb_path, "puts 42\n").unwrap();
    let cov_path = dir.join("mismatch.sqlite");
    common::write_fixture(
        &cov_path,
        &[("/totally/unrelated/path/foo.rb", 1, &["spec.rb"])],
    );

    let mut cmd = Command::cargo_bin("transmute")?;
    cmd.arg("--coverage").arg(cov_path.to_str().unwrap());
    cmd.arg("--files").arg(rb_path.to_str().unwrap());
    cmd.arg("--command").arg("sh -c true");
    cmd.arg("--log-level").arg("warn");

    cmd.assert()
        .stderr(predicate::str::contains("coverage files match"));

    std::fs::remove_dir_all(&dir).ok();
    Ok(())
}
