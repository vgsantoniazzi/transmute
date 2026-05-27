use glob::glob;
use log::{info, trace, warn};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

pub mod ruby;

struct ActiveMutation {
    token: u64,
    file_path: String,
    original: Vec<u8>,
}

type ActiveMutations = Vec<ActiveMutation>;
static ACTIVE_MUTATIONS: OnceLock<Mutex<ActiveMutations>> = OnceLock::new();
static NEXT_GUARD_TOKEN: AtomicU64 = AtomicU64::new(1);

fn active_mutations() -> &'static Mutex<ActiveMutations> {
    ACTIVE_MUTATIONS.get_or_init(|| Mutex::new(Vec::new()))
}

fn locked_active_mutations() -> std::sync::MutexGuard<'static, ActiveMutations> {
    match active_mutations().lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn tmp_path(file_path: &str) -> String {
    format!("{}.transmute.{}.tmp", file_path, std::process::id())
}

fn restore_tmp_path(file_path: &str) -> String {
    format!("{}.transmute.restore.{}.tmp", file_path, std::process::id())
}

pub fn restore_active_mutations() {
    let mut guard = locked_active_mutations();
    for entry in guard.drain(..) {
        if let Err(e) = std::fs::write(&entry.file_path, &entry.original) {
            eprintln!(
                "FATAL: could not restore {} on signal: {}",
                entry.file_path, e
            );
        }
        let _ = std::fs::remove_file(tmp_path(&entry.file_path));
        let _ = std::fs::remove_file(restore_tmp_path(&entry.file_path));
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

            let entries = match glob(pattern) {
                Ok(e) => e,
                Err(e) => {
                    warn!("Bad --files pattern '{}': {}; skipping.", pattern, e);
                    continue;
                }
            };
            for entry in entries {
                let file_path = match entry {
                    Ok(p) => p.display().to_string(),
                    Err(e) => {
                        warn!("Skipping unreadable path: {}", e);
                        continue;
                    }
                };
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
            Some((prefix, tail)) if tail.parse::<u32>().is_ok() => prefix,
            _ => path,
        }
    }

    pub fn extract_line_number(path: &str) -> u32 {
        path.rsplit_once(':')
            .and_then(|(_, tail)| tail.parse::<u32>().ok())
            .unwrap_or(0)
    }

    pub fn find_mutations(file_path: String, line_number: u32) -> Vec<MutableItem> {
        match Path::new(&file_path).extension().and_then(|s| s.to_str()) {
            Some("rb") => ruby::find_all(&file_path, line_number),
            _ => {
                warn!("File '{}' is not supported. Skipping.", file_path);
                Vec::new()
            }
        }
    }
}

impl MutableItem {
    pub fn transmute(&self, file_path: &str) {
        let original = std::fs::read(file_path).expect("Unable to read file");
        self.write_mutation(&original, file_path)
            .expect("write_mutation failed");
    }

    pub fn write_mutation(&self, original: &[u8], file_path: &str) -> std::io::Result<()> {
        info!(
            "Changing '{}' by '{}' on {}:{}",
            self.content, self.replace, file_path, self.line_number
        );

        let mut line_starts: Vec<usize> = vec![0];
        for (i, &b) in original.iter().enumerate() {
            if b == b'\n' {
                line_starts.push(i + 1);
            }
        }

        let line_idx = (self.line_number as usize).saturating_sub(1);
        if line_idx >= line_starts.len() {
            return Ok(());
        }
        let line_start = line_starts[line_idx];
        let abs_start = line_start + self.start;
        let abs_end = line_start + self.end;
        if abs_end > original.len() {
            return Ok(());
        }

        if &original[abs_start..abs_end] != self.content.as_bytes() {
            eprintln!(
                "transmute: '{}' changed since scan; skipping mutation at line {}",
                file_path, self.line_number
            );
            return Ok(());
        }

        let mut out = Vec::with_capacity(original.len() + self.replace.len());
        out.extend_from_slice(&original[..abs_start]);
        out.extend_from_slice(self.replace.as_bytes());
        out.extend_from_slice(&original[abs_end..]);

        let tmp = tmp_path(file_path);
        std::fs::write(&tmp, &out)?;
        std::fs::rename(&tmp, file_path)?;
        Ok(())
    }
}

pub struct MutationGuard<'a> {
    file_path: &'a str,
    original: Vec<u8>,
    token: u64,
}

impl<'a> MutationGuard<'a> {
    pub fn apply(file_path: &'a str, item: &MutableItem) -> std::io::Result<MutationGuard<'a>> {
        let original = std::fs::read(file_path)?;
        let token = NEXT_GUARD_TOKEN.fetch_add(1, Ordering::Relaxed);
        locked_active_mutations().push(ActiveMutation {
            token,
            file_path: file_path.to_string(),
            original: original.clone(),
        });
        if let Err(e) = item.write_mutation(&original, file_path) {
            locked_active_mutations().retain(|entry| entry.token != token);
            return Err(e);
        }
        Ok(MutationGuard {
            file_path,
            original,
            token,
        })
    }
}

impl<'a> Drop for MutationGuard<'a> {
    fn drop(&mut self) {
        trace!("Restoring {}", self.file_path);
        let _ = std::fs::remove_file(tmp_path(self.file_path));
        let restore_tmp = restore_tmp_path(self.file_path);
        let atomic_restore = std::fs::write(&restore_tmp, &self.original)
            .and_then(|()| std::fs::rename(&restore_tmp, self.file_path));
        if let Err(e) = atomic_restore {
            let _ = std::fs::remove_file(&restore_tmp);
            eprintln!("FATAL: could not restore {}: {}", self.file_path, e);
        }
        locked_active_mutations().retain(|entry| entry.token != self.token);
    }
}

pub fn read_lines<P>(file_path: P) -> Vec<String>
where
    P: AsRef<Path>,
{
    let bytes = match std::fs::read(&file_path) {
        Ok(b) => b,
        Err(e) => {
            warn!(
                "Skipping unreadable '{}': {}",
                file_path.as_ref().display(),
                e
            );
            return Vec::new();
        }
    };
    let mut out: Vec<String> = Vec::new();
    for line in bytes.split(|&b| b == b'\n') {
        let trimmed = if line.last() == Some(&b'\r') {
            &line[..line.len() - 1]
        } else {
            line
        };
        match std::str::from_utf8(trimmed) {
            Ok(s) => out.push(s.to_string()),
            Err(_) => out.push(String::new()),
        }
    }
    out
}
