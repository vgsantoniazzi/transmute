use log::{info, trace, warn};
use serde_json::Value;
use std::fs;

pub struct Coverage {
    pub data: Value,
}

impl Coverage {
    pub fn load(path: &str) -> Coverage {
        info!("Loading coverage {}..", path);
        let file = fs::read_to_string(path).expect(&format!("Unable to read file: {}", path));
        return Coverage {
            data: serde_json::from_str(&file).expect("Unable to parse"),
        };
    }

    pub fn find(&self, file: &str, line: u16) -> Vec<String> {
        let accessor = format!("{}:{}", file, line);
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
