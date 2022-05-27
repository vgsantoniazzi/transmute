use log::{info, trace, warn};
use serde_json::Value;
use std::fs;

pub struct Coverage {
    pub data: Value,
}

impl Coverage {
    pub fn load(file_path: &str) -> Coverage {
        info!("Loading coverage {}..", file_path);
        let file =
            fs::read_to_string(file_path).expect(&format!("Unable to read file: {}", file_path));
        return Coverage {
            data: serde_json::from_str(&file).expect("Unable to parse"),
        };
    }

    pub fn find(&self, file: &str, line: u16) -> Vec<String> {
        let current_dir = std::env::current_dir()
            .unwrap()
            .into_os_string()
            .into_string()
            .unwrap();
        let accessor = format!("{}/{}:{}", current_dir, file, line);
        trace!("loading specs specs for {}", accessor);

        let mut result = Vec::new();
        for item in self.data[accessor.clone()]
            .as_array()
            .unwrap_or(&Vec::new())
            .iter()
        {
            result.push(item.as_str().unwrap().to_string());
        }

        if result.is_empty() {
            warn!("not found specs for {}", accessor)
        };

        return result;
    }
}
