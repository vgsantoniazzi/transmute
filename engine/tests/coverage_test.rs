mod common;

use transmute::coverage;

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
fn test_load_accepts_sqlite_file_named_with_json_extension() {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "transmute_test_{}_renamed.json",
        std::process::id()
    ));
    let abs = absolute("a/b.rb");
    common::write_fixture(&path, &[(abs.as_str(), 1, &["spec.rb"])]);

    coverage::Coverage::load(path.to_str().unwrap())
        .expect("A valid SQLite file must load even if its filename ends in .json");

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
        cov.find("./a/b.rb", 1, 0).specs,
        vec!["spec.rb".to_string()],
        "Leading './' must be stripped before lookup"
    );
    assert_eq!(
        cov.find("a/./b.rb", 1, 0).specs,
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
        cov.find(&abs_file, 3, 0).specs,
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
    let m = cov.find("tests/fixtures/app/user.rb", 3, 0);
    assert_eq!(m.specs, ["./spec/user_spec.rb"]);
    assert_eq!(m.total, 1);

    std::fs::remove_file(&path).ok();
}

#[test]
fn test_find_returns_empty_when_key_missing() {
    let path = fixture_path("missing_key");
    let abs = absolute("tests/fixtures/app/user.rb");
    common::write_fixture(&path, &[(abs.as_str(), 3, &["./spec/user_spec.rb"])]);

    let cov = coverage::Coverage::load(path.to_str().unwrap()).unwrap();
    let m = cov.find("not-found.rs", 1, 0);
    let expected: Vec<String> = Vec::new();
    assert_eq!(m.specs, expected);
    assert_eq!(m.total, 0, "no covering specs => total is zero (uncovered)");

    std::fs::remove_file(&path).ok();
}

#[test]
fn test_find_orders_specs_by_lines_of_target_file_descending() {
    let path = fixture_path("local_ranking");
    let abs = absolute("app/user.rb");
    let other = absolute("app/other.rb");
    common::write_fixture(
        &path,
        &[
            (abs.as_str(), 1, &["narrow_spec.rb", "wide_spec.rb"]),
            (abs.as_str(), 2, &["wide_spec.rb"]),
            (abs.as_str(), 3, &["wide_spec.rb"]),
            (other.as_str(), 1, &["narrow_spec.rb"]),
            (other.as_str(), 2, &["narrow_spec.rb"]),
            (other.as_str(), 3, &["narrow_spec.rb"]),
            (other.as_str(), 4, &["narrow_spec.rb"]),
        ],
    );

    let cov = coverage::Coverage::load(path.to_str().unwrap()).unwrap();
    let m = cov.find("app/user.rb", 1, 0);
    assert_eq!(
        m.specs,
        vec!["wide_spec.rb".to_string(), "narrow_spec.rb".to_string()],
        "wide_spec covers 3 lines of user.rb; narrow_spec covers 1 line — wide_spec ranks higher"
    );
    assert_eq!(m.total, 2);

    std::fs::remove_file(&path).ok();
}

#[test]
fn test_find_breaks_ties_alphabetically_by_spec_path() {
    let path = fixture_path("tie_breaker");
    let abs = absolute("app/user.rb");
    common::write_fixture(
        &path,
        &[(abs.as_str(), 1, &["c_spec.rb", "a_spec.rb", "b_spec.rb"])],
    );

    let cov = coverage::Coverage::load(path.to_str().unwrap()).unwrap();
    let m = cov.find("app/user.rb", 1, 0);
    assert_eq!(
        m.specs,
        vec![
            "a_spec.rb".to_string(),
            "b_spec.rb".to_string(),
            "c_spec.rb".to_string()
        ],
        "Equal line_count => sort alphabetically by path"
    );

    std::fs::remove_file(&path).ok();
}

#[test]
fn test_find_caps_specs_when_max_specs_is_nonzero() {
    let path = fixture_path("cap_max_specs");
    let abs = absolute("app/user.rb");
    common::write_fixture(
        &path,
        &[(
            abs.as_str(),
            1,
            &["s_01.rb", "s_02.rb", "s_03.rb", "s_04.rb", "s_05.rb"],
        )],
    );

    let cov = coverage::Coverage::load(path.to_str().unwrap()).unwrap();
    let m = cov.find("app/user.rb", 1, 3);
    assert_eq!(m.specs.len(), 3);
    assert_eq!(m.total, 5, "total reflects covering set before truncation");

    std::fs::remove_file(&path).ok();
}

#[test]
fn test_find_does_not_truncate_when_max_specs_is_zero() {
    let path = fixture_path("unlimited");
    let abs = absolute("app/user.rb");
    common::write_fixture(
        &path,
        &[(abs.as_str(), 1, &["s_01.rb", "s_02.rb", "s_03.rb"])],
    );

    let cov = coverage::Coverage::load(path.to_str().unwrap()).unwrap();
    let m = cov.find("app/user.rb", 1, 0);
    assert_eq!(m.specs.len(), 3);
    assert_eq!(m.total, 3);

    std::fs::remove_file(&path).ok();
}

fn insert_meta(conn: &rusqlite::Connection, key: &str, value: &str) {
    conn.execute(
        "INSERT OR REPLACE INTO schema_meta (key, value) VALUES (?1, ?2)",
        rusqlite::params![key, value],
    )
    .unwrap();
}

fn insert_one_row(conn: &rusqlite::Connection, file: &str, line: u32, spec: &str) {
    conn.execute(
        "INSERT OR IGNORE INTO files (path) VALUES (?1)",
        rusqlite::params![file],
    )
    .unwrap();
    conn.execute(
        "INSERT OR IGNORE INTO specs (path) VALUES (?1)",
        rusqlite::params![spec],
    )
    .unwrap();
    let file_id: i64 = conn
        .query_row(
            "SELECT id FROM files WHERE path = ?1",
            rusqlite::params![file],
            |r| r.get(0),
        )
        .unwrap();
    let spec_id: i64 = conn
        .query_row(
            "SELECT id FROM specs WHERE path = ?1",
            rusqlite::params![spec],
            |r| r.get(0),
        )
        .unwrap();
    conn.execute(
        "INSERT OR IGNORE INTO coverage (file_id, line, spec_id) VALUES (?1, ?2, ?3)",
        rusqlite::params![file_id, line, spec_id],
    )
    .unwrap();
}

#[test]
fn test_coverage_cwd_is_exposed_after_load() {
    let path = fixture_path("cwd_exposed");
    let conn = rusqlite::Connection::open(&path).unwrap();
    transmute::coverage::schema::initialize(&conn).unwrap();
    insert_meta(&conn, "cwd", "/captured/at/this/path");
    drop(conn);

    let cov = coverage::Coverage::load(path.to_str().unwrap()).unwrap();
    assert_eq!(cov.coverage_cwd(), Some("/captured/at/this/path"));

    std::fs::remove_file(&path).ok();
}

#[test]
fn test_coverage_cwd_is_none_when_schema_meta_lacks_it() {
    let path = fixture_path("no_cwd_meta");
    common::write_fixture(&path, &[]);

    let cov = coverage::Coverage::load(path.to_str().unwrap()).unwrap();
    assert_eq!(
        cov.coverage_cwd(),
        None,
        "Legacy DBs without cwd metadata must report None"
    );

    std::fs::remove_file(&path).ok();
}

#[test]
fn test_find_translates_relative_path_through_stored_cwd() {
    let path = fixture_path("translate_relative");
    let captured_cwd = "/captured/somewhere";
    let stored_file = format!("{}/app/user.rb", captured_cwd);
    let conn = rusqlite::Connection::open(&path).unwrap();
    transmute::coverage::schema::initialize(&conn).unwrap();
    insert_meta(&conn, "cwd", captured_cwd);
    insert_one_row(&conn, &stored_file, 1, "spec.rb");
    drop(conn);

    let cov = coverage::Coverage::load(path.to_str().unwrap()).unwrap();
    let m = cov.find("app/user.rb", 1, 0);
    assert_eq!(
        m.specs,
        vec!["spec.rb".to_string()],
        "relative input resolved against runtime cwd, then translated through stored cwd"
    );

    std::fs::remove_file(&path).ok();
}

#[test]
fn test_find_translates_absolute_path_under_runtime_cwd() {
    let path = fixture_path("translate_absolute_under");
    let captured_cwd = "/captured/elsewhere";
    let stored_file = format!("{}/lib/util.rb", captured_cwd);
    let conn = rusqlite::Connection::open(&path).unwrap();
    transmute::coverage::schema::initialize(&conn).unwrap();
    insert_meta(&conn, "cwd", captured_cwd);
    insert_one_row(&conn, &stored_file, 7, "util_spec.rb");
    drop(conn);

    let cov = coverage::Coverage::load(path.to_str().unwrap()).unwrap();
    let abs_runtime = absolute("lib/util.rb");
    let m = cov.find(&abs_runtime, 7, 0);
    assert_eq!(
        m.specs,
        vec!["util_spec.rb".to_string()],
        "absolute input under runtime cwd must be translated to stored cwd prefix"
    );

    std::fs::remove_file(&path).ok();
}

#[test]
fn test_find_does_not_translate_absolute_path_outside_runtime_cwd() {
    let path = fixture_path("translate_outside");
    let captured_cwd = "/captured/elsewhere";
    let foreign_file = "/strange/place/file.rb";
    let conn = rusqlite::Connection::open(&path).unwrap();
    transmute::coverage::schema::initialize(&conn).unwrap();
    insert_meta(&conn, "cwd", captured_cwd);
    insert_one_row(&conn, foreign_file, 1, "weird_spec.rb");
    drop(conn);

    let cov = coverage::Coverage::load(path.to_str().unwrap()).unwrap();
    let m = cov.find(foreign_file, 1, 0);
    assert_eq!(
        m.specs,
        vec!["weird_spec.rb".to_string()],
        "Absolute paths outside runtime cwd must look up as-is, not be translated"
    );

    std::fs::remove_file(&path).ok();
}

#[test]
fn test_find_skips_translation_when_stored_cwd_matches_runtime_cwd() {
    let path = fixture_path("translate_noop");
    let abs = absolute("app/user.rb");
    let conn = rusqlite::Connection::open(&path).unwrap();
    transmute::coverage::schema::initialize(&conn).unwrap();
    let runtime_cwd = std::env::current_dir().unwrap().display().to_string();
    insert_meta(&conn, "cwd", &runtime_cwd);
    insert_one_row(&conn, &abs, 1, "spec.rb");
    drop(conn);

    let cov = coverage::Coverage::load(path.to_str().unwrap()).unwrap();
    let m = cov.find("app/user.rb", 1, 0);
    assert_eq!(
        m.specs,
        vec!["spec.rb".to_string()],
        "stored cwd == runtime cwd => no-op translation, direct lookup succeeds"
    );

    std::fs::remove_file(&path).ok();
}

#[test]
fn test_find_returns_full_set_when_covering_count_equals_max_specs() {
    let path = fixture_path("boundary");
    let abs = absolute("app/user.rb");
    common::write_fixture(
        &path,
        &[(abs.as_str(), 1, &["s_01.rb", "s_02.rb", "s_03.rb"])],
    );

    let cov = coverage::Coverage::load(path.to_str().unwrap()).unwrap();
    let m = cov.find("app/user.rb", 1, 3);
    assert_eq!(m.specs.len(), 3, "n == limit => no truncation");
    assert_eq!(m.total, 3);

    std::fs::remove_file(&path).ok();
}
