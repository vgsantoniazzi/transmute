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
    pub coverage_complete: bool,
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

    pub fn add(
        &mut self,
        file_path: &str,
        mutable: &MutableItem,
        exit_code: i32,
        stdout: String,
        coverage_complete: bool,
    ) {
        self.mutations.push(MutationResult {
            file_path: file_path.to_string(),
            item: mutable.clone(),
            exit_code,
            stdout,
            coverage_complete,
        })
    }

    pub fn failures(&self) -> usize {
        self.survivor_outcomes()
            .into_values()
            .filter(|s| s.is_survivor())
            .count()
    }

    pub fn low_confidence_failures(&self) -> usize {
        self.survivor_outcomes()
            .into_values()
            .filter(|s| s.is_survivor() && !s.coverage_complete)
            .count()
    }

    fn survivor_outcomes(&self) -> HashMap<MutationKey<'_>, SurvivorState> {
        let mut state: HashMap<MutationKey, SurvivorState> = HashMap::new();
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
            entry.coverage_complete = entry.coverage_complete && m.coverage_complete;
        }
        state
    }
}

type MutationKey<'a> = (&'a str, u32, usize, usize);

struct SurvivorState {
    killed: bool,
    had_real_run: bool,
    coverage_complete: bool,
}

impl Default for SurvivorState {
    fn default() -> Self {
        SurvivorState {
            killed: false,
            had_real_run: false,
            coverage_complete: true,
        }
    }
}

impl SurvivorState {
    fn is_survivor(&self) -> bool {
        self.had_real_run && !self.killed
    }
}
