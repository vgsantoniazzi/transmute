mod common;

use transmute::coverage;
use transmute::coverage::CoverageMode;

use std::path::PathBuf;

fn fixture_path(name: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "transmute_test_{}_{}.sqlite",
        std::process::id(),
        name
    ));
    path
}

fn absolute(path: &str) -> String {
    let cwd = std::env::current_dir().unwrap().display().to_string();
    format!("{}/{}", cwd, path)
}

#[test]
fn test_load_returns_err_when_file_not_found() {
    let err = match coverage::Coverage::load("not_found.sqlite") {
        Err(e) => e,
        Ok(_) => panic!("Missing file should return Err, not Ok"),
    };
    assert!(
        err.contains("unable to open coverage file") && err.contains("not_found.sqlite"),
        "Error message must name the missing file; got: {}",
        err
    );
}

#[test]
fn test_load_returns_err_with_migration_hint_when_path_has_json_extension() {
    let mut path = std::env::temp_dir();
    path.push(format!("transmute_test_{}_legacy.json", std::process::id()));
    std::fs::write(&path, r#"{"some_file.rb:1": ["spec.rb"]}"#).unwrap();

    let err = match coverage::Coverage::load(path.to_str().unwrap()) {
        Err(e) => e,
        Ok(_) => panic!("Legacy .json path should return Err"),
    };
    assert!(
        err.contains("legacy JSON format") && err.contains("transmute.sqlite"),
        "Error must point users at the migration path; got: {}",
        err
    );

    std::fs::remove_file(&path).ok();
}

#[test]
fn test_load_returns_err_with_migration_hint_when_content_starts_with_brace() {
    let path = fixture_path("json_no_ext");
    std::fs::write(&path, r#"{"some_file.rb:1": ["spec.rb"]}"#).unwrap();

    let err = match coverage::Coverage::load(path.to_str().unwrap()) {
        Err(e) => e,
        Ok(_) => panic!("JSON-content file should return Err"),
    };
    assert!(
        err.contains("legacy JSON format"),
        "JSON content (regardless of extension) must trigger migration hint; got: {}",
        err
    );

    std::fs::remove_file(&path).ok();
}

#[test]
fn test_load_returns_err_when_db_is_not_a_transmute_coverage_database() {
    let path = fixture_path("not_transmute");
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute("CREATE TABLE something_else (id INTEGER)", [])
        .unwrap();
    drop(conn);

    let err = match coverage::Coverage::load(path.to_str().unwrap()) {
        Err(e) => e,
        Ok(_) => panic!("Non-transmute DB should return Err"),
    };
    assert!(
        err.contains("not a valid transmute database"),
        "Error must explain the file is not a transmute DB; got: {}",
        err
    );
    std::fs::remove_file(&path).ok();
}

#[test]
fn test_load_returns_err_when_schema_version_mismatches() {
    let path = fixture_path("bad_version");
    let conn = rusqlite::Connection::open(&path).unwrap();
    transmute::coverage::schema::initialize(&conn).unwrap();
    conn.execute(
        "UPDATE schema_meta SET value = '999' WHERE key = 'version'",
        [],
    )
    .unwrap();
    drop(conn);

    let err = match coverage::Coverage::load(path.to_str().unwrap()) {
        Err(e) => e,
        Ok(_) => panic!("Mismatched schema version should return Err"),
    };
    assert!(
        err.contains("999") && err.contains("schema version"),
        "Error must name the mismatched version; got: {}",
        err
    );
    std::fs::remove_file(&path).ok();
}

#[test]
fn test_find_normalizes_curdir_components_in_path() {
    let path = fixture_path("curdir");
    let abs = absolute("a/b.rb");
    common::write_fixture(&path, &[(abs.as_str(), 1, &["spec.rb"])]);

    let cov = coverage::Coverage::load(path.to_str().unwrap()).unwrap();
    assert_eq!(
        cov.find("./a/b.rb", 1, CoverageMode::High).specs,
        vec!["spec.rb".to_string()],
        "Leading './' must be stripped before lookup so the relative path matches the canonical key"
    );
    assert_eq!(
        cov.find("a/./b.rb", 1, CoverageMode::High).specs,
        vec!["spec.rb".to_string()],
        "Mid-path './' segment must be normalized away"
    );

    std::fs::remove_file(&path).ok();
}

#[test]
fn test_find_returns_specs_when_file_path_is_absolute() {
    let path = fixture_path("abs");
    let abs_file = absolute("tests/fixtures/app/user.rb");
    common::write_fixture(&path, &[(abs_file.as_str(), 3, &["./spec/user_spec.rb"])]);

    let cov = coverage::Coverage::load(path.to_str().unwrap()).unwrap();
    assert_eq!(
        cov.find(&abs_file, 3, CoverageMode::High).specs,
        ["./spec/user_spec.rb"],
        "Absolute file path must look up the same key the gem wrote"
    );

    std::fs::remove_file(&path).ok();
}

#[test]
fn test_find_returns_specs_for_known_line() {
    let path = fixture_path("known_line");
    let abs = absolute("tests/fixtures/app/user.rb");
    common::write_fixture(&path, &[(abs.as_str(), 3, &["./spec/user_spec.rb"])]);

    let cov = coverage::Coverage::load(path.to_str().unwrap()).unwrap();
    let m = cov.find("tests/fixtures/app/user.rb", 3, CoverageMode::High);
    assert_eq!(m.specs, ["./spec/user_spec.rb"]);
    assert!(m.complete, "single covering spec returned in full");

    std::fs::remove_file(&path).ok();
}

#[test]
fn test_find_returns_empty_when_key_missing() {
    let path = fixture_path("missing_key");
    let abs = absolute("tests/fixtures/app/user.rb");
    common::write_fixture(&path, &[(abs.as_str(), 3, &["./spec/user_spec.rb"])]);

    let cov = coverage::Coverage::load(path.to_str().unwrap()).unwrap();
    let m = cov.find("not-found.rs", 1, CoverageMode::High);
    let expected: Vec<String> = Vec::new();
    assert_eq!(m.specs, expected);
    assert!(m.complete, "empty match is trivially complete");

    std::fs::remove_file(&path).ok();
}

#[test]
fn test_find_returns_multiple_specs_in_stable_order() {
    let path = fixture_path("multi_specs");
    let abs = absolute("app/user.rb");
    common::write_fixture(
        &path,
        &[(abs.as_str(), 5, &["a_spec.rb", "b_spec.rb", "c_spec.rb"])],
    );

    let cov = coverage::Coverage::load(path.to_str().unwrap()).unwrap();
    let m = cov.find("app/user.rb", 5, CoverageMode::High);
    let mut specs = m.specs;
    specs.sort();
    assert_eq!(specs, vec!["a_spec.rb", "b_spec.rb", "c_spec.rb"]);

    std::fs::remove_file(&path).ok();
}

fn write_layered_fixture(name: &str) -> PathBuf {
    let path = fixture_path(name);
    let abs = absolute("app/user.rb");
    let mut entries: Vec<(String, u32, Vec<String>)> = Vec::new();
    let target_specs: Vec<String> = (0..15).map(|i| format!("spec_{:02}.rb", i)).collect();
    entries.push((abs.clone(), 1, target_specs.clone()));
    for (i, spec) in target_specs.iter().enumerate() {
        for f in 0..i {
            entries.push((
                absolute(&format!("app/other_{:02}_{}.rb", i, f)),
                1,
                vec![spec.clone()],
            ));
        }
    }
    let entry_refs: Vec<(&str, u32, Vec<&str>)> = entries
        .iter()
        .map(|(file, line, specs)| {
            (
                file.as_str(),
                *line,
                specs.iter().map(|s| s.as_str()).collect(),
            )
        })
        .collect();
    let entry_slices: Vec<(&str, u32, &[&str])> = entry_refs
        .iter()
        .map(|(file, line, specs)| (*file, *line, specs.as_slice()))
        .collect();
    common::write_fixture(&path, &entry_slices);
    path
}

#[test]
fn test_find_orders_specs_by_global_file_count_ascending() {
    let path = write_layered_fixture("ordered");
    let cov = coverage::Coverage::load(path.to_str().unwrap()).unwrap();

    let m = cov.find("app/user.rb", 1, CoverageMode::High);
    let expected: Vec<String> = (0..15).map(|i| format!("spec_{:02}.rb", i)).collect();
    assert_eq!(
        m.specs, expected,
        "specs must be ordered by global file_count ascending (narrowest first)"
    );
    assert!(
        m.complete,
        "high mode returns everything; result is complete"
    );

    std::fs::remove_file(&path).ok();
}

#[test]
fn test_find_low_mode_keeps_top_three_narrowest() {
    let path = write_layered_fixture("low_mode");
    let cov = coverage::Coverage::load(path.to_str().unwrap()).unwrap();

    let m = cov.find("app/user.rb", 1, CoverageMode::Low);
    assert_eq!(
        m.specs,
        vec![
            "spec_00.rb".to_string(),
            "spec_01.rb".to_string(),
            "spec_02.rb".to_string()
        ],
        "low mode keeps only the 3 narrowest specs"
    );
    assert!(
        !m.complete,
        "low mode dropped specs; result must be flagged as incomplete"
    );

    std::fs::remove_file(&path).ok();
}

#[test]
fn test_find_medium_mode_keeps_top_ten_narrowest() {
    let path = write_layered_fixture("medium_mode");
    let cov = coverage::Coverage::load(path.to_str().unwrap()).unwrap();

    let m = cov.find("app/user.rb", 1, CoverageMode::Medium);
    let expected: Vec<String> = (0..10).map(|i| format!("spec_{:02}.rb", i)).collect();
    assert_eq!(
        m.specs, expected,
        "medium mode keeps the 10 narrowest specs"
    );
    assert!(!m.complete, "medium mode dropped specs; flagged incomplete");

    std::fs::remove_file(&path).ok();
}

#[test]
fn test_find_complete_is_true_when_covering_set_smaller_than_limit() {
    let path = fixture_path("small_covering");
    let abs = absolute("app/user.rb");
    common::write_fixture(&path, &[(abs.as_str(), 1, &["only_spec.rb"])]);

    let cov = coverage::Coverage::load(path.to_str().unwrap()).unwrap();
    let m = cov.find("app/user.rb", 1, CoverageMode::Low);
    assert_eq!(m.specs, vec!["only_spec.rb".to_string()]);
    assert!(
        m.complete,
        "covering set (1) <= limit (3); result must be complete"
    );

    std::fs::remove_file(&path).ok();
}

#[test]
fn test_coverage_mode_parse_accepts_valid_strings() {
    assert!(matches!(CoverageMode::parse("low"), Ok(CoverageMode::Low)));
    assert!(matches!(
        CoverageMode::parse("medium"),
        Ok(CoverageMode::Medium)
    ));
    assert!(matches!(
        CoverageMode::parse("high"),
        Ok(CoverageMode::High)
    ));
}

#[test]
fn test_coverage_mode_parse_rejects_unknown_string() {
    let err = CoverageMode::parse("aggressive").unwrap_err();
    assert!(
        err.contains("aggressive") && err.contains("low") && err.contains("high"),
        "Error must name the bad value and list valid options; got: {}",
        err
    );
}
