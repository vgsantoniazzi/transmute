use rusqlite::{params, Connection, OptionalExtension};

pub const VERSION: u32 = 1;

const DDL: &str = r#"
CREATE TABLE IF NOT EXISTS schema_meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS files (
    id   INTEGER PRIMARY KEY,
    path TEXT NOT NULL UNIQUE
);

CREATE TABLE IF NOT EXISTS specs (
    id   INTEGER PRIMARY KEY,
    path TEXT NOT NULL UNIQUE
);

CREATE TABLE IF NOT EXISTS coverage (
    file_id INTEGER NOT NULL,
    line    INTEGER NOT NULL,
    spec_id INTEGER NOT NULL,
    PRIMARY KEY (file_id, line, spec_id)
) WITHOUT ROWID;

CREATE INDEX IF NOT EXISTS idx_coverage_spec ON coverage(spec_id);
"#;

pub fn initialize(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(DDL)?;
    conn.execute(
        "INSERT OR REPLACE INTO schema_meta (key, value) VALUES ('version', ?1)",
        params![VERSION.to_string()],
    )?;
    Ok(())
}

pub fn verify(conn: &Connection) -> Result<(), String> {
    let table_exists: Option<String> = conn
        .query_row(
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'schema_meta'",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| {
            format!(
                "coverage DB query failed while checking schema metadata: {}",
                e
            )
        })?;

    if table_exists.is_none() {
        return Err(
            "coverage DB has no schema_meta table; not a transmute coverage database".to_string(),
        );
    }

    let stored: Option<String> = conn
        .query_row(
            "SELECT value FROM schema_meta WHERE key = 'version'",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| format!("coverage DB schema_meta read failed: {}", e))?;

    let stored = match stored {
        Some(v) => v,
        None => return Err("coverage DB schema_meta has no 'version' entry".to_string()),
    };

    if stored != VERSION.to_string() {
        return Err(format!(
            "coverage DB schema version '{}' is not supported; engine expects '{}'. Re-run your test suite with a matching transmute coverage adapter.",
            stored, VERSION
        ));
    }

    Ok(())
}
