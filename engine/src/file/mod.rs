use glob::glob;
use log::{info, trace, warn};
use serde::Serialize;
use std::io::Write;
use std::io::{self, BufRead};
use std::path::Path;

mod ruby;

#[derive(Debug, Clone, Serialize)]
pub struct MutableItem {
    pub line_number: u16,
    pub implementation: String,
    pub content: String,
    pub replace: String,
}

#[derive(Debug)]
pub struct File {
    pub path: String,
    pub mutable_items: Vec<MutableItem>,
}

impl File {
    pub fn load(files_paths: &str) -> Vec<File> {
        info!("processing files paths {}..", files_paths);
        let mut files: Vec<File> = Vec::new();

        for path in files_paths.split(";") {
            let pattern = File::extract_glob_pattern(path);
            let line_number = File::extract_line_number(path);

            for file in glob(pattern).expect("Failed to read glob pattern") {
                let file_path = String::from(file.unwrap().display().to_string());
                files.push(File {
                    path: file_path.clone(),
                    mutable_items: File::find_mutations(file_path, line_number),
                });
            }
        }
        return files;
    }

    pub fn extract_glob_pattern(path: &str) -> &str {
        let splitted: Vec<&str> = path.split(":").collect();
        return splitted.first().unwrap();
    }

    pub fn extract_line_number(path: &str) -> u16 {
        let splitted: Vec<&str> = path.split(":").collect();

        if splitted.len() == 1 {
            return 0;
        } else {
            let line_number = splitted.last().unwrap().parse::<u16>().unwrap();
            return line_number;
        }
    }

    pub fn find_mutations(file_path: String, line_number: u16) -> Vec<MutableItem> {
        let signature: Vec<&str> = file_path.split(".").collect();
        match signature[signature.len() - 1] {
            "rb" => ruby::find_all(&file_path, line_number),
            _ => {
                warn!("File '{}' is not supported. Skipping.", file_path);
                Vec::new()
            }
        }
    }
}

impl MutableItem {
    pub fn transmute(&self, file_path: &str) {
        info!(
            "Changing '{}' by '{}' on {}:{}",
            self.content, self.replace, file_path, self.line_number
        );

        self.change_content(file_path, true);
    }

    pub fn undo(&self, file_path: &str) {
        trace!("Undoing {}:{}", file_path, self.line_number);

        self.change_content(file_path, false);
    }

    fn change_content(&self, file_path: &str, transmute: bool) {
        let file_content = read_lines(file_path);
        let mut file = std::fs::File::create(file_path).expect("Can't open file for writing");
        let mut line_counter = 0;
        for line_content in file_content {
            line_counter += 1;
            if line_counter == self.line_number {
                if transmute {
                    write!(
                        file,
                        "{}\n",
                        line_content.replace(&self.content, &self.replace)
                    )
                    .unwrap();
                } else {
                    write!(
                        file,
                        "{}\n",
                        line_content.replace(&self.replace, &self.content)
                    )
                    .unwrap();
                }
            } else {
                write!(file, "{}\n", line_content).unwrap();
            }
        }
    }
}

fn read_lines<P>(file_path: P) -> Vec<String>
where
    P: AsRef<Path>,
{
    return io::BufReader::new(open_file(file_path))
        .lines()
        .collect::<Result<_, _>>()
        .unwrap();
}

fn open_file<P>(file_path: P) -> std::fs::File
where
    P: AsRef<Path>,
{
    return std::fs::File::open(file_path).expect("Unable to find file!");
}
