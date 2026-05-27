use log::{info, trace, warn};
use serde_json::Value;
use std::fs;
use std::path::Path;
use std::sync::OnceLock;

static CWD: OnceLock<String> = OnceLock::new();

pub struct Coverage {
    pub data: Value,
}

impl Coverage {
    pub fn load(file_path: &str) -> Result<Coverage, String> {
        info!("Loading coverage {}..", file_path);
        let file = fs::read_to_string(file_path)
            .map_err(|e| format!("unable to read coverage file '{}': {}", file_path, e))?;
        let data = serde_json::from_str(&file)
            .map_err(|e| format!("unable to parse coverage JSON '{}': {}", file_path, e))?;
        let cov = Coverage { data };
        cov.warn_if_no_coverage_keys_match_cwd();
        Ok(cov)
    }

    pub fn find(&self, file: &str, line: u32) -> Vec<String> {
        let normalized: std::path::PathBuf = Path::new(file)
            .components()
            .filter(|c| !matches!(c, std::path::Component::CurDir))
            .collect();
        let accessor = if normalized.is_absolute() {
            format!("{}:{}", normalized.display(), line)
        } else {
            format!("{}/{}:{}", cwd(), normalized.display(), line)
        };
        trace!("loading specs for {}", accessor);

        let empty: Vec<Value> = Vec::new();
        self.data[&accessor]
            .as_array()
            .unwrap_or(&empty)
            .iter()
            .filter_map(|item| item.as_str().map(|s| s.to_string()))
            .collect()
    }

    fn warn_if_no_coverage_keys_match_cwd(&self) {
        let prefix = format!("{}/", cwd());
        let any_match = self
            .data
            .as_object()
            .map(|o| o.keys().any(|k| k.starts_with(&prefix)))
            .unwrap_or(false);
        if !any_match {
            warn!(
                "No coverage keys match cwd '{}'. Re-run coverage from the same directory transmute runs from, or the lookup will always return empty.",
                cwd()
            );
        }
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
