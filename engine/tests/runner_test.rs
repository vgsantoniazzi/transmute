use transmute::runner;

use std::time::{Duration, Instant};

#[test]
fn test_is_infra_error_pins_exit_code_taxonomy() {
    assert!(runner::is_infra_error(2), "exit 2 (argparse) is infra");
    assert!(runner::is_infra_error(124), "exit 124 (timeout) is infra");
    assert!(
        runner::is_infra_error(125),
        "exit 125 (wait failure) is infra"
    );
    assert!(
        runner::is_infra_error(127),
        "exit 127 (spawn failure) is infra"
    );
    assert!(
        !runner::is_infra_error(0),
        "exit 0 (pass) is not an infra error"
    );
    assert!(
        !runner::is_infra_error(1),
        "exit 1 (real failure) is not an infra error"
    );
    assert!(
        !runner::is_infra_error(3),
        "exit 3 must not be classified as infra"
    );
    assert!(
        !runner::is_infra_error(126),
        "exit 126 must not be classified as infra"
    );
}

#[test]
fn test_when_binary_missing_returns_spawn_failure_code_127() {
    let (exit_code, _stdout) =
        runner::run("definitely-not-a-real-binary", "", Duration::from_secs(5));
    assert_eq!(
        exit_code, 127,
        "Missing binary must report spawn failure as exit 127, the canonical infra code"
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
        stdout.len() >= 524_000,
        "Must drain the full 524288-byte pipe (line endings add a few bytes); got {} bytes",
        stdout.len()
    );
}

#[test]
fn test_run_kills_command_that_exceeds_timeout_with_code_124() {
    let start = Instant::now();
    let (exit_code, stdout) = runner::run("sleep 30", "", Duration::from_millis(500));
    let elapsed = start.elapsed();

    assert!(
        elapsed < Duration::from_secs(5),
        "Timeout should fire well before 5s; took {:?}",
        elapsed
    );
    assert_eq!(
        exit_code, 124,
        "Timed-out command must report exit 124, the canonical timeout infra code"
    );
    assert!(
        stdout.contains("timed out"),
        "Stdout should explain the timeout; got: {}",
        stdout
    );
}

#[test]
fn test_run_merges_stderr_into_stdout_with_separator() {
    let (exit_code, stdout) = runner::run(
        r#"sh -c 'echo OUT_LINE; echo ERR_LINE 1>&2'"#,
        "",
        Duration::from_secs(5),
    );
    assert_eq!(exit_code, 0);
    assert!(
        stdout.contains("OUT_LINE"),
        "stdout content must be captured; got: {}",
        stdout
    );
    assert!(
        stdout.contains("--- stderr ---"),
        "Separator must appear when stderr is non-empty; got: {}",
        stdout
    );
    assert!(
        stdout.contains("ERR_LINE"),
        "stderr content must be captured into the combined buffer; got: {}",
        stdout
    );
}

#[test]
fn test_run_omits_stderr_separator_when_stderr_is_empty() {
    let (exit_code, stdout) =
        runner::run(r#"sh -c 'echo only_stdout'"#, "", Duration::from_secs(5));
    assert_eq!(exit_code, 0);
    assert!(stdout.contains("only_stdout"));
    assert!(
        !stdout.contains("--- stderr ---"),
        "Separator must NOT appear when stderr is empty; got: {}",
        stdout
    );
}

#[test]
fn test_run_returns_exit_2_when_command_string_is_unparseable() {
    let (exit_code, stdout) = runner::run("\"unclosed", "", Duration::from_secs(5));
    assert_eq!(
        exit_code, 2,
        "Unparseable --command must report exit 2 (argparse infra)"
    );
    assert!(
        stdout.contains("cannot parse --command"),
        "Stdout should explain the parse failure; got: {}",
        stdout
    );
}
