#[path = "../src/runner/mod.rs"]
mod runner;

use std::time::{Duration, Instant};

#[test]
fn test_when_binary_missing_returns_nonzero_exit_code() {
    let (exit_code, _stdout) =
        runner::run("definitely-not-a-real-binary", "", Duration::from_secs(5));
    assert_ne!(
        exit_code, 0,
        "Missing binary should propagate as non-zero exit, not panic"
    );
}

#[test]
fn test_returns_zero_exit_code_when_command_succeeds() {
    let (exit_code, _stdout) = runner::run("sh -c true", "", Duration::from_secs(5));
    assert_eq!(exit_code, 0);
}

#[test]
fn test_returns_nonzero_exit_code_when_command_fails() {
    let (exit_code, _stdout) = runner::run("sh -c false", "", Duration::from_secs(5));
    assert_eq!(exit_code, 1);
}

#[test]
fn test_run_passes_quoted_file_with_spaces_as_single_argument() {
    let path = std::env::temp_dir().join(format!(
        "transmute_test_spaces_{}_with space.txt",
        std::process::id()
    ));
    std::fs::write(&path, "").unwrap();

    let (exit_code, _stdout) = runner::run(
        r#"test -f "{file}""#,
        path.to_str().unwrap(),
        Duration::from_secs(5),
    );
    assert_eq!(
        exit_code, 0,
        "Quoted path with spaces must reach the command as one argument"
    );

    std::fs::remove_file(&path).ok();
}

#[test]
fn test_run_does_not_execute_injected_command_in_spec_path() {
    let canary = std::env::temp_dir().join(format!(
        "transmute_canary_{}_{}",
        std::process::id(),
        "injection"
    ));
    let _ = std::fs::remove_file(&canary);

    let payload = format!("foo; touch {}", canary.display());
    let (_exit_code, _stdout) = runner::run("echo {file}", &payload, Duration::from_secs(5));

    assert!(
        !canary.exists(),
        "Injected command must not run; canary should not exist at {:?}",
        canary
    );

    let _ = std::fs::remove_file(&canary);
}

#[test]
fn test_run_does_not_deadlock_on_large_stdout() {
    let (exit_code, stdout) = runner::run(
        r#"sh -c 'yes x | head -c 524288'"#,
        "",
        Duration::from_secs(10),
    );
    assert_eq!(
        exit_code, 0,
        "Command should complete cleanly (got code {} — 124 means timeout/deadlock)",
        exit_code
    );
    assert!(
        stdout.len() > 100_000,
        "Should have captured the large stdout; got {} bytes",
        stdout.len()
    );
}

#[test]
fn test_run_kills_command_that_exceeds_timeout() {
    let start = Instant::now();
    let (exit_code, _stdout) = runner::run("sleep 30", "", Duration::from_millis(500));
    let elapsed = start.elapsed();

    assert!(
        elapsed < Duration::from_secs(5),
        "Timeout should fire well before 5s; took {:?}",
        elapsed
    );
    assert_ne!(exit_code, 0, "Timed-out command must not report success");
}
