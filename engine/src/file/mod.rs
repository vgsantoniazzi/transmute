use glob::glob;
use log::{info, trace, warn};
use serde::Serialize;
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
    pub line_number: u32,
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

    pub fn extract_line_number(path: &str) -> u32 {
        path.rsplit_once(':')
            .and_then(|(_, tail)| tail.parse::<u32>().ok())
            .unwrap_or(0)
    }

    pub fn find_mutations(file_path: String, line_number: u32) -> Vec<MutableItem> {
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

        let original = std::fs::read(file_path).expect("Unable to read file");

        let mut line_starts: Vec<usize> = vec![0];
        for (i, &b) in original.iter().enumerate() {
            if b == b'\n' {
                line_starts.push(i + 1);
            }
        }

        let line_idx = (self.line_number as usize).saturating_sub(1);
        if line_idx >= line_starts.len() {
            return;
        }
        let line_start = line_starts[line_idx];
        let abs_start = line_start + self.start;
        let abs_end = line_start + self.end;
        if abs_end > original.len() {
            return;
        }

        if &original[abs_start..abs_end] != self.content.as_bytes() {
            eprintln!(
                "transmute: '{}' changed since scan; skipping mutation at line {}",
                file_path, self.line_number
            );
            return;
        }

        let mut out = Vec::with_capacity(original.len() + self.replace.len());
        out.extend_from_slice(&original[..abs_start]);
        out.extend_from_slice(self.replace.as_bytes());
        out.extend_from_slice(&original[abs_end..]);

        let tmp_path = format!("{}.transmute.tmp", file_path);
        std::fs::write(&tmp_path, &out).expect("Can't write temp file");
        std::fs::rename(&tmp_path, file_path).expect("Can't rename mutated file");
    }
}

pub struct MutationGuard<'a> {
    file_path: &'a str,
    original: Vec<u8>,
}

impl<'a> MutationGuard<'a> {
    pub fn apply(file_path: &'a str, item: &MutableItem) -> std::io::Result<MutationGuard<'a>> {
        let original = std::fs::read(file_path)?;
        *active_mutation().lock().unwrap() = Some((file_path.to_string(), original.clone()));
        let guard = MutationGuard {
            file_path,
            original,
        };
        item.transmute(file_path);
        Ok(guard)
    }
}

impl<'a> Drop for MutationGuard<'a> {
    fn drop(&mut self) {
        trace!("Restoring {}", self.file_path);
        let tmp_path = format!("{}.transmute.tmp", self.file_path);
        let _ = std::fs::remove_file(&tmp_path);
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
