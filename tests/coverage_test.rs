#[path = "../src/coverage/mod.rs"]
mod coverage;

#[test]
fn test_err_when_file_not_found() {
    let result = std::panic::catch_unwind(|| coverage::Coverage::load("not_found.json"));
    assert!(result.is_err());
}

#[test]
fn test_find() {
    let cov = coverage::Coverage::load("tests/fixtures/transmute.json");
    let mut result = Vec::new();
    result.push("/app/spec/models/user_spec.rb");
    assert_eq!(cov.find("/app/app/models/user.rb", 1), result);
}


#[test]
fn test_find_not_found_return_empty() {
    let cov = coverage::Coverage::load("tests/fixtures/transmute.json");
    let result: Vec<String> = Vec::new();
    assert_eq!(cov.find("not-found.rs", 1), result);
}
