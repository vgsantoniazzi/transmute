#[path = "../src/file/mod.rs"]
mod file;

#[test]
fn test_load_all_rb_files() {
    let files: Vec<String> = file::File::load("**/*.rb")
        .into_iter()
        .map(|f| f.path)
        .collect();

    assert_eq!(
        files,
        [
            "tests/fixtures/app/user.rb",
            "tests/fixtures/specs/user_error_spec.rb",
            "tests/fixtures/specs/user_spec.rb"
        ]
    );
}
