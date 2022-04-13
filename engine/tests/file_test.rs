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
            "tests/fixtures/spec/spec_helper.rb",
            "tests/fixtures/spec/user_error_spec.rb",
            "tests/fixtures/spec/user_spec.rb"
        ]
    );
}
