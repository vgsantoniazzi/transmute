#[path = "../src/runner/mod.rs"]
mod runner;

#[test]
fn test_when_cant_run() {
    let result = std::panic::catch_unwind(|| runner::run("undef", "test.rs"));
    assert!(result.is_err());
}

#[test]
fn test_when_success() {
    assert_eq!(runner::run("rspec {file}", ""), 0);
}

#[test]
fn test_when_failure() {
    assert_eq!(
        runner::run("rspec {file}", "tests/fixtures/specs/user_error_spec.rb"),
        1
    );
}
