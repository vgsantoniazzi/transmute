#[path = "../src/file/mod.rs"]
mod file;

use std::panic::AssertUnwindSafe;
use std::path::PathBuf;

fn scratch_path(name: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "transmute_test_{}_{}.rb",
        std::process::id(),
        name
    ));
    path
}

#[test]
fn test_load_all_rb_files() {
    let mut files: Vec<String> = file::File::load("**/*.rb")
        .into_iter()
        .map(|f| f.path)
        .collect();
    files.sort();

    assert_eq!(
        files,
        [
            "tests/fixtures/app/app.rb",
            "tests/fixtures/app/user.rb",
            "tests/fixtures/spec/app_spec.rb",
            "tests/fixtures/spec/spec_helper.rb",
            "tests/fixtures/spec/user_error_spec.rb",
            "tests/fixtures/spec/user_spec.rb"
        ]
    );
}

#[test]
fn test_source_file_restored_when_caller_panics() {
    let scratch = scratch_path("guard_panic");
    let original = b"puts 42\n";
    std::fs::write(&scratch, original).unwrap();

    let item = file::MutableItem {
        line_number: 1,
        implementation: "puts 42".to_string(),
        content: "42".to_string(),
        replace: "999".to_string(),
    };

    let path = scratch.clone();
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        let _guard = file::MutationGuard::apply(path.to_str().unwrap(), &item);
        panic!("simulated runner failure");
    }));
    assert!(result.is_err());

    let after = std::fs::read(&scratch).unwrap();
    assert_eq!(after.as_slice(), original);

    std::fs::remove_file(&scratch).ok();
}

#[test]
fn test_source_file_restored_when_guard_dropped_normally() {
    let scratch = scratch_path("guard_normal");
    let original = b"puts 42\n";
    std::fs::write(&scratch, original).unwrap();

    let item = file::MutableItem {
        line_number: 1,
        implementation: "puts 42".to_string(),
        content: "42".to_string(),
        replace: "999".to_string(),
    };

    {
        let _guard = file::MutationGuard::apply(scratch.to_str().unwrap(), &item);
        let mid = std::fs::read_to_string(&scratch).unwrap();
        assert!(mid.contains("999"), "file should be mutated inside guard scope");
    }

    let after = std::fs::read(&scratch).unwrap();
    assert_eq!(after.as_slice(), original);

    std::fs::remove_file(&scratch).ok();
}

#[test]
fn test_change_content_is_atomic_when_write_target_unavailable() {
    let scratch = scratch_path("atomic_write");
    let original = b"puts \"a\"\nputs 42\n";
    std::fs::write(&scratch, original).unwrap();

    // Sabotage the atomic-write temp path so the rewrite cannot complete.
    let sabotage = format!("{}.transmute.tmp", scratch.display());
    std::fs::create_dir_all(&sabotage).unwrap();

    let item = file::MutableItem {
        line_number: 2,
        implementation: "puts 42".to_string(),
        content: "42".to_string(),
        replace: "999".to_string(),
    };

    let path = scratch.clone();
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        item.transmute(path.to_str().unwrap());
    }));
    assert!(result.is_err(), "transmute should panic when temp path is unavailable");

    let after = std::fs::read(&scratch).unwrap();
    assert_eq!(
        after.as_slice(),
        original,
        "original file must remain intact when atomic write fails"
    );

    std::fs::remove_dir_all(&sabotage).ok();
    std::fs::remove_file(&scratch).ok();
}
