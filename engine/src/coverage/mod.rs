pub mod schema;

use log::{error, info, trace, warn};
use rusqlite::{params, Connection, OpenFlags, OptionalExtension};
use std::path::Path;
use std::sync::OnceLock;

static CWD: OnceLock<String> = OnceLock::new();

pub struct CoverageMatch {
    pub specs: Vec<String>,
    pub total: usize,
}

pub struct Coverage {
    conn: Connection,
    coverage_cwd: Option<String>,
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

        let coverage_cwd: Option<String> = conn
            .query_row(
                "SELECT value FROM schema_meta WHERE key = 'cwd'",
                [],
                |row| row.get(0),
            )
            .optional()
            .ok()
            .flatten();

        let cov = Coverage { conn, coverage_cwd };
        cov.warn_about_data_health();
        Ok(cov)
    }

    pub fn coverage_cwd(&self) -> Option<&str> {
        self.coverage_cwd.as_deref()
    }

    pub fn find(&self, file: &str, line: u32, max_specs: Option<usize>) -> CoverageMatch {
        let key = self.canonical_key(file);
        trace!(
            "loading specs for {}:{} (max_specs={:?})",
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

        covering.sort_by_cached_key(|(_, lines, path)| {
            (
                proximity_rank(path, file),
                std::cmp::Reverse(*lines),
                path.clone(),
            )
        });

        let total = covering.len();
        let specs: Vec<String> = covering
            .into_iter()
            .take(max_specs.unwrap_or(usize::MAX))
            .map(|(_, _, p)| p)
            .collect();

        if specs.len() < total {
            trace!(
                "filtered {}:{} specs from {} to {} (max_specs={:?})",
                key,
                line,
                total,
                specs.len(),
                max_specs
            );
        }

        CoverageMatch { specs, total }
    }

    fn canonical_key(&self, file: &str) -> String {
        let normalized: std::path::PathBuf = Path::new(file)
            .components()
            .filter(|c| !matches!(c, std::path::Component::CurDir))
            .collect();
        let absolute = if normalized.is_absolute() {
            normalized.display().to_string()
        } else {
            format!("{}/{}", runtime_cwd(), normalized.display())
        };

        let stored = match self.coverage_cwd.as_deref() {
            Some(s) if s != runtime_cwd() => s,
            _ => return absolute,
        };
        let runtime_prefix = format!("{}/", runtime_cwd());
        match absolute.strip_prefix(&runtime_prefix) {
            Some(rest) => format!("{}/{}", stored, rest),
            None => absolute,
        }
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

    fn warn_about_data_health(&self) {
        let prefix_root = self
            .coverage_cwd
            .as_deref()
            .unwrap_or_else(|| runtime_cwd());
        let pattern = format!("{}/%", prefix_root);
        let (row_count, any_cwd_match): (i64, i64) = self
            .conn
            .query_row(
                "SELECT (SELECT COUNT(*) FROM coverage),
                        EXISTS (SELECT 1 FROM files WHERE path LIKE ?1)",
                params![pattern],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap_or((0, 0));
        if row_count == 0 {
            warn!(
                "Coverage database has zero rows. Every mutation will be reported as surviving with no covering spec; the report will be uninformative. Re-generate coverage from your test suite."
            );
        }
        if any_cwd_match == 0 {
            warn!(
                "No coverage files match '{}'. Re-run coverage from the same directory transmute runs from, or the lookup will always return empty.",
                prefix_root
            );
        }
    }
}

const SOURCE_ROOTS: &[&str] = &["app", "lib", "src"];
const TEST_ROOTS: &[&str] = &["spec", "tests", "test", "__tests__"];
const TEST_SUFFIX_MARKERS: &[&str] = &["_spec", "_test", ".spec", ".test", "-spec", "-test"];
const TEST_PREFIX_MARKERS: &[&str] = &["test_", "spec_", "test.", "spec.", "test-", "spec-"];

pub fn proximity_rank(spec_path: &str, source_path: &str) -> u8 {
    let (source_stem, source_dir) = normalized_parts(source_path, SOURCE_ROOTS);
    let (spec_stem, spec_dir) = normalized_parts(spec_path, TEST_ROOTS);
    let spec_stem_unmarked = strip_test_markers(&spec_stem);

    let name_match = !source_stem.is_empty() && source_stem == spec_stem_unmarked;
    let dir_match = source_dir == spec_dir;

    match (name_match, dir_match) {
        (true, true) => 0,
        (true, false) => 1,
        (false, true) => 2,
        (false, false) => 3,
    }
}

fn normalized_parts(file_path: &str, roots: &[&str]) -> (String, Vec<String>) {
    let path = Path::new(file_path);
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();
    let parent_components: Vec<String> = path
        .parent()
        .map(|p| {
            p.components()
                .filter_map(|c| match c {
                    std::path::Component::Normal(s) => s.to_str().map(String::from),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default();

    let strip_at = parent_components
        .iter()
        .position(|c| roots.contains(&c.as_str()));
    let remaining = match strip_at {
        Some(i) => parent_components[i + 1..].to_vec(),
        None => parent_components,
    };
    (stem, remaining)
}

fn strip_test_markers(stem: &str) -> String {
    let lowered = stem.to_lowercase();
    for suffix in TEST_SUFFIX_MARKERS {
        if lowered.ends_with(suffix) && lowered.len() > suffix.len() {
            return stem[..stem.len() - suffix.len()].to_string();
        }
    }
    for prefix in TEST_PREFIX_MARKERS {
        if lowered.starts_with(prefix) && lowered.len() > prefix.len() {
            return stem[prefix.len()..].to_string();
        }
    }
    stem.to_string()
}

fn looks_like_json_coverage(file_path: &str) -> bool {
    use std::io::Read;
    let mut header = [0u8; 1];
    std::fs::File::open(file_path)
        .and_then(|mut f| f.read(&mut header))
        .map(|n| n >= 1 && matches!(header[0], b'{' | b'['))
        .unwrap_or(false)
}

fn runtime_cwd() -> &'static str {
    CWD.get_or_init(|| {
        std::env::current_dir()
            .ok()
            .and_then(|p| p.into_os_string().into_string().ok())
            .unwrap_or_default()
    })
}
