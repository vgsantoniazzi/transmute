use rusqlite::{params, Connection};
use std::path::Path;
use transmute::coverage::schema;

pub fn write_fixture(db_path: &Path, entries: &[(&str, u32, &[&str])]) {
    if db_path.exists() {
        std::fs::remove_file(db_path).ok();
    }
    let conn = Connection::open(db_path).unwrap();
    schema::initialize(&conn).unwrap();
    insert_entries(&conn, entries);
}

fn insert_entries(conn: &Connection, entries: &[(&str, u32, &[&str])]) {
    let tx = conn.unchecked_transaction().unwrap();
    for (file, line, specs) in entries.iter() {
        tx.execute(
            "INSERT OR IGNORE INTO files (path) VALUES (?1)",
            params![file],
        )
        .unwrap();
        let file_id: i64 = tx
            .query_row(
                "SELECT id FROM files WHERE path = ?1",
                params![file],
                |row| row.get(0),
            )
            .unwrap();

        for spec in specs.iter() {
            tx.execute(
                "INSERT OR IGNORE INTO specs (path) VALUES (?1)",
                params![spec],
            )
            .unwrap();
            let spec_id: i64 = tx
                .query_row(
                    "SELECT id FROM specs WHERE path = ?1",
                    params![spec],
                    |row| row.get(0),
                )
                .unwrap();

            tx.execute(
                "INSERT OR IGNORE INTO coverage (file_id, line, spec_id) VALUES (?1, ?2, ?3)",
                params![file_id, *line, spec_id],
            )
            .unwrap();
        }
    }
    tx.commit().unwrap();
}
