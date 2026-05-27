use rusqlite::Connection;
use transmute::coverage::schema;

#[test]
fn test_initialize_creates_schema_and_records_version() {
    let conn = Connection::open_in_memory().unwrap();
    schema::initialize(&conn).unwrap();

    let version: String = conn
        .query_row(
            "SELECT value FROM schema_meta WHERE key = 'version'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(version, schema::VERSION.to_string());
}

#[test]
fn test_initialize_is_idempotent() {
    let conn = Connection::open_in_memory().unwrap();
    schema::initialize(&conn).unwrap();
    schema::initialize(&conn).expect("re-initialization on existing DB must not fail");
}

#[test]
fn test_verify_accepts_matching_version() {
    let conn = Connection::open_in_memory().unwrap();
    schema::initialize(&conn).unwrap();
    schema::verify(&conn).expect("freshly initialized DB must verify");
}

#[test]
fn test_verify_rejects_mismatched_version() {
    let conn = Connection::open_in_memory().unwrap();
    schema::initialize(&conn).unwrap();
    conn.execute(
        "UPDATE schema_meta SET value = '999' WHERE key = 'version'",
        [],
    )
    .unwrap();

    let err = schema::verify(&conn).unwrap_err();
    assert!(
        err.contains("999") && err.contains(&schema::VERSION.to_string()),
        "Mismatch error must name both versions; got: {}",
        err
    );
}

#[test]
fn test_verify_rejects_db_without_schema_meta() {
    let conn = Connection::open_in_memory().unwrap();
    let err = schema::verify(&conn).unwrap_err();
    assert!(
        err.contains("schema") || err.contains("metadata"),
        "Missing-schema error must mention schema; got: {}",
        err
    );
}
