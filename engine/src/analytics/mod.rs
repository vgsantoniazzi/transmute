use std::collections::HashMap;

use crate::file::MutableItem;
use crate::runner;
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
        type MutationKey<'a> = (&'a str, u32, usize, usize, &'a str);
        type RunOutcome = (bool, bool);
        let mut state: HashMap<MutationKey, RunOutcome> = HashMap::new();
        for m in self.mutations.iter() {
            let key: MutationKey = (
                m.file_path.as_str(),
                m.item.line_number,
                m.item.start,
                m.item.end,
                m.item.replace.as_str(),
            );
            let entry = state.entry(key).or_insert((false, false));
            let real_kill = m.exit_code != 0 && !runner::is_infra_error(m.exit_code);
            let real_run = !runner::is_infra_error(m.exit_code);
            entry.0 = entry.0 || real_kill;
            entry.1 = entry.1 || real_run;
        }
        state
            .values()
            .filter(|(killed, had_real_run)| *had_real_run && !*killed)
            .count()
    }
}
