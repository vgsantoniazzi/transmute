#[path = "../src/coverage/mod.rs"]
mod coverage;

use std::fs;
use std::path::PathBuf;

fn write_fixture(name: &str) -> PathBuf {
    let cwd = std::env::current_dir().unwrap().display().to_string();
    let mut path = std::env::temp_dir();
    path.push(format!(
        "transmute_test_{}_{}.json",
        std::process::id(),
        name
    ));
    let content = format!(r#"{{"{cwd}/tests/fixtures/app/user.rb:3": ["./spec/user_spec.rb"]}}"#);
    fs::write(&path, content).unwrap();
    path
}

#[test]
fn test_load_returns_err_when_file_not_found() {
    let result = coverage::Coverage::load("not_found.json");
    assert!(result.is_err(), "Missing file should return Err, not panic");
}

#[test]
fn test_load_returns_err_when_json_is_malformed() {
    let mut path = std::env::temp_dir();
    path.push(format!("transmute_test_{}_bad.json", std::process::id()));
    fs::write(&path, "not json").unwrap();
    let result = coverage::Coverage::load(path.to_str().unwrap());
    assert!(result.is_err(), "Invalid JSON should return Err, not panic");
    fs::remove_file(&path).ok();
}

#[test]
fn test_find_skips_non_string_entries_without_panic() {
    let cwd = std::env::current_dir().unwrap().display().to_string();
    let mut path = std::env::temp_dir();
    path.push(format!(
        "transmute_test_{}_nonstring.json",
        std::process::id()
    ));
    let content = format!(r#"{{"{}/a.rb:1": ["ok.rb", 42, null, "ok2.rb"]}}"#, cwd);
    fs::write(&path, content).unwrap();

    let cov = coverage::Coverage::load(path.to_str().unwrap()).unwrap();
    assert_eq!(
        cov.find("a.rb", 1),
        vec!["ok.rb".to_string(), "ok2.rb".to_string()],
        "Non-string entries should be silently filtered, not panic"
    );

    fs::remove_file(&path).ok();
}

#[test]
fn test_find_returns_specs_for_known_line() {
    let fixture = write_fixture("test_find");
    let cov = coverage::Coverage::load(fixture.to_str().unwrap()).unwrap();
    assert_eq!(
        cov.find("tests/fixtures/app/user.rb", 3),
        ["./spec/user_spec.rb"]
    );
    fs::remove_file(&fixture).ok();
}

#[test]
fn test_find_returns_empty_when_key_missing() {
    let fixture = write_fixture("test_find_missing");
    let cov = coverage::Coverage::load(fixture.to_str().unwrap()).unwrap();
    let expected: Vec<String> = Vec::new();
    assert_eq!(cov.find("not-found.rs", 1), expected);
    fs::remove_file(&fixture).ok();
}
