use glob::glob;
use log::{info, trace};

#[derive(Debug)]
pub struct Line {
    pub line_number: u16,
    pub content: String,
}

#[derive(Debug)]
pub struct File {
    pub path: String,
    pub mutable_items: Vec<Line>,
}

impl File {
    pub fn load(paths: &str) -> Vec<File> {
        info!("Loading files {}..", paths);
        let mut files: Vec<File> = Vec::new();

        for file in glob(paths).expect("Failed to read glob pattern") {
            let path = String::from(file.unwrap().display().to_string());
            trace!("Find {} file", path);
            files.push(File {
                path: path,
                mutable_items: vec![Line {
                    line_number: 1,
                    content: "const NUMBER = 42".to_string(),
                }],
            });
        }
        return files;
    }
}

impl Line {
    pub fn transmute(&self, _file_path: &str) {
        trace!(
            "Changing '{}' by '{}' on line {}",
            self.content,
            self.content,
            self.line_number
        )
    }
}
