#[path = "../src/runner/mod.rs"]
mod runner;

#[test]
fn test_when_cant_run() {
    let result = std::panic::catch_unwind(|| runner::run("undef", "test.rs"));
    assert!(result.is_err());
}

#[test]
fn test_returns_zero_exit_code_when_command_succeeds() {
    let (exit_code, _stdout) = runner::run("sh -c true", "");
    assert_eq!(exit_code, 0);
}

#[test]
fn test_returns_nonzero_exit_code_when_command_fails() {
    let (exit_code, _stdout) = runner::run("sh -c false", "");
    assert_eq!(exit_code, 1);
}
