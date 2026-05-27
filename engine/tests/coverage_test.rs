#[path = "../src/coverage/mod.rs"]
mod coverage;

use std::fs;
use std::path::PathBuf;

fn write_fixture(name: &str) -> PathBuf {
    let cwd = std::env::current_dir().unwrap().display().to_string();
    let mut path = std::env::temp_dir();
    path.push(format!("transmute_test_{}_{}.json", std::process::id(), name));
    let content = format!(
        r#"{{"{cwd}/tests/fixtures/app/user.rb:3": ["./spec/user_spec.rb"]}}"#
    );
    fs::write(&path, content).unwrap();
    path
}

#[test]
fn test_err_when_file_not_found() {
    let result = std::panic::catch_unwind(|| coverage::Coverage::load("not_found.json"));
    assert!(result.is_err());
}

#[test]
fn test_find_returns_specs_for_known_line() {
    let fixture = write_fixture("test_find");
    let cov = coverage::Coverage::load(fixture.to_str().unwrap());
    assert_eq!(
        cov.find("tests/fixtures/app/user.rb", 3),
        ["./spec/user_spec.rb"]
    );
    fs::remove_file(&fixture).ok();
}

#[test]
fn test_find_returns_empty_when_key_missing() {
    let fixture = write_fixture("test_find_missing");
    let cov = coverage::Coverage::load(fixture.to_str().unwrap());
    let expected: Vec<String> = Vec::new();
    assert_eq!(cov.find("not-found.rs", 1), expected);
    fs::remove_file(&fixture).ok();
}
