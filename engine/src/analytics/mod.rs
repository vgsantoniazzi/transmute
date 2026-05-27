use std::collections::HashMap;

use crate::file::MutableItem;
use crate::runner;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct MutationResult {
    pub file_path: String,
    pub item: MutableItem,
    pub exit_code: i32,
    pub stdout: String,
    pub specs_total: usize,
}

#[derive(Debug, Serialize, Deserialize)]
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

    pub fn add(
        &mut self,
        file_path: &str,
        mutable: &MutableItem,
        exit_code: i32,
        stdout: String,
        specs_total: usize,
    ) {
        self.mutations.push(MutationResult {
            file_path: file_path.to_string(),
            item: mutable.clone(),
            exit_code,
            stdout,
            specs_total,
        })
    }

    pub fn failures(&self) -> usize {
        self.survivor_outcomes()
            .into_values()
            .filter(|s| s.had_real_run && !s.killed)
            .count()
    }

    pub fn uncovered_failures(&self) -> usize {
        self.survivor_outcomes()
            .into_values()
            .filter(|s| s.had_real_run && !s.killed && s.specs_total == 0)
            .count()
    }

    pub fn low_confidence_failures(&self) -> usize {
        self.survivor_outcomes()
            .into_values()
            .filter(|s| {
                s.had_real_run && !s.killed && s.specs_total > 0 && s.specs_run < s.specs_total
            })
            .count()
    }

    fn survivor_outcomes(&self) -> HashMap<MutationKey<'_>, MutationOutcome> {
        let mut state: HashMap<MutationKey, MutationOutcome> = HashMap::new();
        for m in self.mutations.iter() {
            let key: MutationKey = (
                m.file_path.as_str(),
                m.item.line_number,
                m.item.start,
                m.item.end,
            );
            let entry = state.entry(key).or_default();
            let real_kill = m.exit_code != 0 && !runner::is_infra_error(m.exit_code);
            let real_run = !runner::is_infra_error(m.exit_code);
            entry.killed = entry.killed || real_kill;
            entry.had_real_run = entry.had_real_run || real_run;
            entry.specs_total = entry.specs_total.max(m.specs_total);
            if real_run {
                entry.specs_run += 1;
            }
        }
        state
    }
}

type MutationKey<'a> = (&'a str, u32, usize, usize);

#[derive(Default)]
struct MutationOutcome {
    killed: bool,
    had_real_run: bool,
    specs_total: usize,
    specs_run: usize,
}
