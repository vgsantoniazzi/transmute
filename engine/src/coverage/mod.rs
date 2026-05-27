pub mod schema;

use log::{info, trace, warn};
use rusqlite::{params, Connection, OpenFlags};
use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;

static CWD: OnceLock<String> = OnceLock::new();

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum CoverageMode {
    Low,
    Medium,
    High,
}

impl CoverageMode {
    pub fn parse(value: &str) -> Result<CoverageMode, String> {
        match value {
            "low" => Ok(CoverageMode::Low),
            "medium" => Ok(CoverageMode::Medium),
            "high" => Ok(CoverageMode::High),
            other => Err(format!(
                "unknown --coverage-mode '{}'; valid: low, medium, high",
                other
            )),
        }
    }

    fn limit(self) -> Option<usize> {
        match self {
            CoverageMode::Low => Some(3),
            CoverageMode::Medium => Some(10),
            CoverageMode::High => None,
        }
    }
}

pub struct CoverageMatch {
    pub specs: Vec<String>,
    pub complete: bool,
}

pub struct Coverage {
    conn: Connection,
    spec_frequency: HashMap<i64, u32>,
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

        schema::verify(&conn).map_err(|e| {
            format!(
                "coverage file '{}' is not a valid transmute database: {}",
                file_path, e
            )
        })?;

        let spec_frequency = compute_spec_frequency(&conn).map_err(|e| {
            format!(
                "coverage file '{}' could not be summarized for prioritization: {}",
                file_path, e
            )
        })?;

        let cov = Coverage {
            conn,
            spec_frequency,
        };
        cov.warn_if_no_coverage_files_match_cwd();
        Ok(cov)
    }

    pub fn find(&self, file: &str, line: u32, mode: CoverageMode) -> CoverageMatch {
        let key = canonical_key(file);
        trace!("loading specs for {}:{} (mode={:?})", key, line, mode);

        let mut covering = match self.query_covering(&key, line) {
            Ok(v) => v,
            Err(e) => {
                warn!("coverage query failed for {}:{}: {}", key, line, e);
                return CoverageMatch {
                    specs: Vec::new(),
                    complete: true,
                };
            }
        };

        covering.sort_by(|a, b| {
            let fa = self.spec_frequency.get(&a.0).copied().unwrap_or(0);
            let fb = self.spec_frequency.get(&b.0).copied().unwrap_or(0);
            fa.cmp(&fb).then_with(|| a.1.cmp(&b.1))
        });

        let total = covering.len();
        let limit = mode.limit();
        let complete = match limit {
            None => true,
            Some(n) => n >= total,
        };
        let specs: Vec<String> = covering
            .into_iter()
            .take(limit.unwrap_or(usize::MAX))
            .map(|(_, path)| path)
            .collect();

        if !complete {
            trace!(
                "filtered {}:{} specs from {} to {} under {:?}",
                key,
                line,
                total,
                specs.len(),
                mode
            );
        }

        CoverageMatch { specs, complete }
    }

    fn query_covering(&self, key: &str, line: u32) -> rusqlite::Result<Vec<(i64, String)>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT s.id, s.path
             FROM coverage c
             JOIN files f ON c.file_id = f.id
             JOIN specs s ON c.spec_id = s.id
             WHERE f.path = ?1 AND c.line = ?2",
        )?;
        let rows = stmt.query_map(params![key, line], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?;
        rows.collect()
    }

    fn warn_if_no_coverage_files_match_cwd(&self) {
        let prefix = format!("{}/", cwd());
        let pattern = format!("{}%", prefix);
        let any: i64 = self
            .conn
            .query_row(
                "SELECT EXISTS (SELECT 1 FROM files WHERE path LIKE ?1)",
                params![pattern],
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

fn compute_spec_frequency(conn: &Connection) -> rusqlite::Result<HashMap<i64, u32>> {
    let mut stmt =
        conn.prepare("SELECT spec_id, COUNT(DISTINCT file_id) FROM coverage GROUP BY spec_id")?;
    let mut rows = stmt.query([])?;
    let mut out = HashMap::new();
    while let Some(row) = rows.next()? {
        let spec_id: i64 = row.get(0)?;
        let count: i64 = row.get(1)?;
        out.insert(spec_id, count.max(0) as u32);
    }
    Ok(out)
}

fn looks_like_json_coverage(file_path: &str) -> bool {
    let path = Path::new(file_path);
    let extension_is_json = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("json"))
        .unwrap_or(false);
    if extension_is_json {
        return true;
    }
    match std::fs::File::open(file_path) {
        Ok(mut f) => {
            use std::io::Read;
            let mut buf = [0u8; 1];
            match f.read(&mut buf) {
                Ok(1) => matches!(buf[0], b'{' | b'['),
                _ => false,
            }
        }
        Err(_) => false,
    }
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
