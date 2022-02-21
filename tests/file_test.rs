#[path = "../src/files/mod.rs"]
mod files;

#[test]
fn test_load_all_rb_files() {
    let files: Vec<String> = files::File::load("**/*.rb")
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
