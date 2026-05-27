extern crate itertools;

use crate::file::MutableItem;
use itertools::Itertools;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct MutationResult {
    pub file_path: String,
    pub item: MutableItem,
    pub exit_code: i32,
    pub stdout: String,
}

#[derive(Debug, Serialize)]
pub struct AnalyticsResult {
    pub files_count: usize,
    pub mutations: Vec<MutationResult>,
}

impl AnalyticsResult {
    pub fn start(files_count: usize) -> AnalyticsResult {
        AnalyticsResult {
            files_count,
            mutations: Vec::new(),
        }
    }

    pub fn add(&mut self, file_path: &str, mutable: &MutableItem, exit_code: i32, stdout: String) {
        self.mutations.push(MutationResult {
            file_path: file_path.to_string(),
            item: mutable.clone(),
            exit_code,
            stdout,
        })
    }

    pub fn failures(&self) -> usize {
        self.mutations
            .iter()
            .group_by(|m| {
                (
                    m.file_path.as_str(),
                    m.item.line_number,
                    m.item.start,
                    m.item.end,
                    m.item.replace.as_str(),
                )
            })
            .into_iter()
            .map(|(_, group)| group.into_iter().any(|r| r.exit_code != 0))
            .filter(|killed| !killed)
            .count()
    }
}
