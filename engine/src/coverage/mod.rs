pub mod schema;

use log::{error, info, trace, warn};
use rusqlite::{params, Connection, OpenFlags};
use std::path::Path;
use std::sync::OnceLock;

static CWD: OnceLock<String> = OnceLock::new();

pub struct CoverageMatch {
    pub specs: Vec<String>,
    pub total: usize,
}

pub struct Coverage {
    conn: Connection,
}

impl Coverage {
    pub fn load(file_path: &str) -> Result<Coverage, String> {
        info!("Loading coverage {}..", file_path);

        if !Path::new(file_path).exists() {
            return Err(format!(
                "unable to open coverage file '{}': file not found",
                file_path
            ));
        }

        if looks_like_json_coverage(file_path) {
            return Err(format!(
                "coverage file '{}' looks like the legacy JSON format. JSON coverage was removed; upgrade transmute-ruby (or your adapter) to write a SQLite database and re-run your test suite to regenerate it as transmute.sqlite.",
                file_path
            ));
        }

        let conn = Connection::open_with_flags(file_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|e| format!("unable to open coverage file '{}': {}", file_path, e))?;

        conn.busy_timeout(std::time::Duration::from_millis(5000))
            .map_err(|e| format!("coverage DB busy_timeout setup failed: {}", e))?;

        schema::verify(&conn).map_err(|e| {
            format!(
                "coverage file '{}' is not a valid transmute database: {}",
                file_path, e
            )
        })?;

        let cov = Coverage { conn };
        cov.warn_if_coverage_table_is_empty();
        cov.warn_if_no_coverage_files_match_cwd();
        Ok(cov)
    }

    pub fn find(&self, file: &str, line: u32, max_specs: usize) -> CoverageMatch {
        let key = canonical_key(file);
        trace!(
            "loading specs for {}:{} (max_specs={})",
            key,
            line,
            max_specs
        );

        let mut covering = match self.query_covering_with_local_score(&key, line) {
            Ok(v) => v,
            Err(e) => {
                error!(
                    "coverage query failed for {}:{}: {}; treating as uncovered",
                    key, line, e
                );
                return CoverageMatch {
                    specs: Vec::new(),
                    total: 0,
                };
            }
        };

        covering.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.2.cmp(&b.2)));

        let total = covering.len();
        let limit = if max_specs == 0 {
            usize::MAX
        } else {
            max_specs
        };
        let specs: Vec<String> = covering
            .into_iter()
            .take(limit)
            .map(|(_, _, p)| p)
            .collect();

        if specs.len() < total {
            trace!(
                "filtered {}:{} specs from {} to {} (max_specs={})",
                key,
                line,
                total,
                specs.len(),
                max_specs
            );
        }

        CoverageMatch { specs, total }
    }

    fn query_covering_with_local_score(
        &self,
        key: &str,
        line: u32,
    ) -> rusqlite::Result<Vec<(i64, i64, String)>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT s.id,
                    (SELECT COUNT(*) FROM coverage c2
                     WHERE c2.file_id = f.id AND c2.spec_id = s.id) AS lines_in_file,
                    s.path
             FROM coverage c
             JOIN files f ON c.file_id = f.id
             JOIN specs s ON c.spec_id = s.id
             WHERE f.path = ?1 AND c.line = ?2",
        )?;
        let rows = stmt.query_map(params![key, line], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;
        rows.collect()
    }

    fn warn_if_coverage_table_is_empty(&self) {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM coverage", [], |row| row.get(0))
            .unwrap_or(0);
        if count == 0 {
            warn!(
                "Coverage database has zero rows. Every mutation will be reported as surviving with no covering spec; the report will be uninformative. Re-generate coverage from your test suite."
            );
        }
    }

    fn warn_if_no_coverage_files_match_cwd(&self) {
        let prefix = format!("{}/%", cwd());
        let any: i64 = self
            .conn
            .query_row(
                "SELECT EXISTS (SELECT 1 FROM files WHERE path LIKE ?1)",
                params![prefix],
                |row| row.get(0),
            )
            .unwrap_or(0);
        if any == 0 {
            warn!(
                "No coverage files match cwd '{}'. Re-run coverage from the same directory transmute runs from, or the lookup will always return empty.",
                cwd()
            );
        }
    }
}

fn looks_like_json_coverage(file_path: &str) -> bool {
    use std::io::Read;

    let mut file = match std::fs::File::open(file_path) {
        Ok(f) => f,
        Err(_) => return false,
    };

    let mut header = [0u8; 16];
    let read = match file.read(&mut header) {
        Ok(n) => n,
        Err(_) => return false,
    };

    if read >= 16 && &header == b"SQLite format 3\0" {
        return false;
    }

    if read >= 1 && matches!(header[0], b'{' | b'[') {
        return true;
    }

    Path::new(file_path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("json"))
        .unwrap_or(false)
}

fn canonical_key(file: &str) -> String {
    let normalized: std::path::PathBuf = Path::new(file)
        .components()
        .filter(|c| !matches!(c, std::path::Component::CurDir))
        .collect();
    if normalized.is_absolute() {
        normalized.display().to_string()
    } else {
        format!("{}/{}", cwd(), normalized.display())
    }
}

fn cwd() -> &'static str {
    CWD.get_or_init(|| {
        std::env::current_dir()
            .ok()
            .and_then(|p| p.into_os_string().into_string().ok())
            .unwrap_or_default()
    })
}
