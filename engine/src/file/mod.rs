use glob::glob;
use log::{info, trace, warn};
use serde::Serialize;
use std::io::Write;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

mod ruby;

type ActiveMutation = Option<(String, Vec<u8>)>;
static ACTIVE_MUTATION: OnceLock<Mutex<ActiveMutation>> = OnceLock::new();

fn active_mutation() -> &'static Mutex<ActiveMutation> {
    ACTIVE_MUTATION.get_or_init(|| Mutex::new(None))
}

pub fn restore_active_mutation() {
    if let Some((path, bytes)) = active_mutation().lock().unwrap().take() {
        let _ = std::fs::write(&path, &bytes);
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct MutableItem {
    pub line_number: u16,
    pub start: usize,
    pub end: usize,
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
                let file_path = file.unwrap().display().to_string();
                files.push(File {
                    path: file_path.clone(),
                    mutable_items: File::find_mutations(file_path, line_number),
                });
            }
        }
        files
    }

    pub fn extract_glob_pattern(path: &str) -> &str {
        match path.rsplit_once(':') {
            Some((prefix, tail)) if tail.parse::<u16>().is_ok() => prefix,
            _ => path,
        }
    }

    pub fn extract_line_number(path: &str) -> u16 {
        path.rsplit_once(':')
            .and_then(|(_, tail)| tail.parse::<u16>().ok())
            .unwrap_or(0)
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

        let file_content = read_lines(file_path);
        let tmp_path = format!("{}.transmute.tmp", file_path);
        {
            let mut file = std::fs::File::create(&tmp_path).expect("Can't open file for writing");
            let mut line_counter = 0;
            for mut line_content in file_content {
                line_counter += 1;
                if line_counter == self.line_number {
                    line_content.replace_range(self.start..self.end, &self.replace);
                }
                writeln!(file, "{}", line_content).unwrap();
            }
        }
        std::fs::rename(&tmp_path, file_path).expect("Can't rename mutated file");
    }
}

pub struct MutationGuard<'a> {
    file_path: &'a str,
    original: Vec<u8>,
}

impl<'a> MutationGuard<'a> {
    pub fn apply(file_path: &'a str, item: &MutableItem) -> MutationGuard<'a> {
        let original = std::fs::read(file_path).expect("Unable to read file");
        *active_mutation().lock().unwrap() = Some((file_path.to_string(), original.clone()));
        let guard = MutationGuard {
            file_path,
            original,
        };
        item.transmute(file_path);
        guard
    }
}

impl<'a> Drop for MutationGuard<'a> {
    fn drop(&mut self) {
        trace!("Restoring {}", self.file_path);
        if let Err(e) = std::fs::write(self.file_path, &self.original) {
            eprintln!("FATAL: could not restore {}: {}", self.file_path, e);
        }
        *active_mutation().lock().unwrap() = None;
    }
}

fn read_lines<P>(file_path: P) -> Vec<String>
where
    P: AsRef<Path>,
{
    let bytes = std::fs::read(&file_path).expect("Unable to read file");
    String::from_utf8_lossy(&bytes)
        .lines()
        .map(|line| line.to_string())
        .collect()
}
