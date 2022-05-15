use crate::file::MutableItem;

#[derive(Debug)]
pub struct MutationResult {
    pub file_path: String,
    pub item: MutableItem,
    pub exit_code: i32,
    pub stdout: String,
}

#[derive(Debug)]
pub struct AnalyticsResult {
    pub files_count: i32,
    pub mutations: Vec<MutationResult>,
}

impl AnalyticsResult {
    pub fn start(files_count: i32) -> AnalyticsResult {
        let mutations: Vec<MutationResult> = Vec::new();
        return AnalyticsResult { files_count: files_count, mutations: mutations }
    }

    pub fn add(&mut self, file_path: &str, mutable: &MutableItem, exit_code: i32, stdout: String) {
        self.mutations.push(MutationResult { file_path: file_path.to_string(), item: mutable.clone(), exit_code: exit_code, stdout: stdout })
    }
}
