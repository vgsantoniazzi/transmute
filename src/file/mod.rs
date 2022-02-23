use glob::glob;
use log::{info, trace};

mod language;

#[derive(Debug)]
pub struct MutableItem {
    pub line_number: u16,
    pub content: String,
}

#[derive(Debug)]
pub struct File {
    pub path: String,
    pub mutable_items: Vec<MutableItem>,
}

impl File {
    pub fn load(files_paths: &str) -> Vec<File> {
        info!("Loading files {}..", files_paths);
        let mut files: Vec<File> = Vec::new();

        for file in glob(files_paths).expect("Failed to read glob pattern") {
            let file_path = String::from(file.unwrap().display().to_string());
            trace!("Found '{}'", file_path);
            files.push(File {
                path: file_path.clone(),
                mutable_items: language::find_mutations(file_path),
            });
        }
        return files;
    }
}

impl MutableItem {
    pub fn transmute(&self, file_path: &str) {
        trace!(
            "Changing '{}' by '{}' on {}:{}",
            self.content,
            self.content,
            file_path,
            self.line_number
        )
    }

    pub fn undo(&self, file_path: &str) {
        trace!("Undo {}:{}", file_path, self.line_number)
    }
}
