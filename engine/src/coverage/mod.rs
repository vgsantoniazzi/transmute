use log::{info, trace, warn};
use serde_json::Value;
use std::fs;

pub struct Coverage {
    pub data: Value,
}

impl Coverage {
    pub fn load(file_path: &str) -> Coverage {
        info!("Loading coverage {}..", file_path);
        let file = fs::read_to_string(file_path)
            .unwrap_or_else(|_| panic!("Unable to read file: {}", file_path));
        let cov = Coverage {
            data: serde_json::from_str(&file).expect("Unable to parse"),
        };
        cov.warn_if_no_coverage_keys_match_cwd();
        cov
    }

    pub fn find(&self, file: &str, line: u16) -> Vec<String> {
        let accessor = format!("{}/{}:{}", cwd(), file, line);
        trace!("loading specs for {}", accessor);

        let empty: Vec<Value> = Vec::new();
        self.data[&accessor]
            .as_array()
            .unwrap_or(&empty)
            .iter()
            .map(|item| item.as_str().unwrap().to_string())
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

fn cwd() -> String {
    std::env::current_dir()
        .unwrap()
        .into_os_string()
        .into_string()
        .unwrap()
}
