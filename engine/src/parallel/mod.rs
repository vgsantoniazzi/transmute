use glob::glob;
use log::{info, warn};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::{Mutex, OnceLock};

use crate::analytics::{AnalyticsResult, MutationResult};
use crate::worktree::{ensure_clean_tree, Worktree};

static ACTIVE_WORKER_PIDS: OnceLock<Mutex<Vec<u32>>> = OnceLock::new();

fn active_worker_pids() -> &'static Mutex<Vec<u32>> {
    ACTIVE_WORKER_PIDS.get_or_init(|| Mutex::new(Vec::new()))
}

fn locked_active_worker_pids() -> std::sync::MutexGuard<'static, Vec<u32>> {
    match active_worker_pids().lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    }
}

pub fn kill_active_workers() {
    let pids: Vec<u32> = locked_active_worker_pids().drain(..).collect();
    for pid in pids {
        let _ = Command::new("kill")
            .arg("-TERM")
            .arg(pid.to_string())
            .status();
    }
}

pub struct ParallelInput<'a> {
    pub files: &'a str,
    pub coverage: &'a str,
    pub command: &'a str,
    pub log_level: &'a str,
    pub timeout: u64,
    pub seed: u64,
    pub max_specs_per_mutation: usize,
    pub jobs: usize,
    pub setup_command: Option<&'a str>,
}

pub struct ParallelResult {
    pub analytics: AnalyticsResult,
    pub any_worker_failed_to_produce_output: bool,
}

pub fn run(input: &ParallelInput<'_>) -> Result<ParallelResult, String> {
    let repo =
        std::env::current_dir().map_err(|e| format!("could not read current directory: {}", e))?;
    ensure_clean_tree(&repo)?;

    let coverage_path = std::fs::canonicalize(input.coverage)
        .map_err(|e| format!("--coverage path '{}' not found: {}", input.coverage, e))?;
    require_coverage_cwd(&coverage_path)?;

    let files = expand_files(&repo, input.files);
    if files.is_empty() {
        return Err(format!(
            "--files pattern '{}' matched no files; nothing to mutate",
            input.files
        ));
    }

    let jobs = input.jobs.min(files.len()).max(1);
    let partitions = partition_round_robin(&files, jobs);
    info!(
        "parallel: {} files across {} workers ({} files/worker avg)",
        files.len(),
        jobs,
        files.len() / jobs
    );

    let worktrees = create_worktrees(&repo, jobs, input.setup_command)?;
    let exe = std::env::current_exe()
        .map_err(|e| format!("could not resolve current executable: {}", e))?;
    let (children, output_paths) =
        spawn_workers(&exe, &worktrees, &partitions, &coverage_path, input)?;
    let any_worker_failed = wait_for_workers(children);

    let any_output_missing = output_paths.iter().any(|p| !p.exists());
    if any_output_missing {
        warn!("one or more workers did not write their result file; the merged report will be incomplete");
    }

    let merged = merge_outputs(&output_paths);
    for path in &output_paths {
        let _ = std::fs::remove_file(path);
    }

    Ok(ParallelResult {
        analytics: merged,
        any_worker_failed_to_produce_output: any_worker_failed || any_output_missing,
    })
}

fn require_coverage_cwd(coverage_path: &Path) -> Result<(), String> {
    let cov = crate::coverage::Coverage::load(
        coverage_path
            .to_str()
            .ok_or_else(|| "coverage path is not valid UTF-8".to_string())?,
    )?;
    if cov.coverage_cwd().is_none() {
        return Err(
            "--jobs > 1 requires coverage produced by transmute-ruby 0.3+ (it writes the capture-time cwd into schema_meta so workers can translate paths). Upgrade the gem and regenerate transmute.sqlite."
                .to_string(),
        );
    }
    Ok(())
}

type FileEntry = (PathBuf, Option<u32>);

fn expand_files(repo: &Path, pattern: &str) -> Vec<FileEntry> {
    let mut out = Vec::new();
    for part in pattern.split(';') {
        let (raw, line_suffix) = split_line_suffix(part);
        let entries = match glob(raw) {
            Ok(e) => e,
            Err(e) => {
                warn!("bad glob '{}': {}; skipping", raw, e);
                continue;
            }
        };
        for entry in entries.flatten() {
            let rel = to_repo_relative(repo, &entry);
            out.push((rel, line_suffix));
        }
    }
    out
}

fn split_line_suffix(pattern: &str) -> (&str, Option<u32>) {
    match pattern.rsplit_once(':') {
        Some((prefix, tail)) => match tail.parse::<u32>() {
            Ok(n) => (prefix, Some(n)),
            Err(_) => (pattern, None),
        },
        None => (pattern, None),
    }
}

fn to_repo_relative(repo: &Path, path: &Path) -> PathBuf {
    path.strip_prefix(repo)
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|_| path.to_path_buf())
}

fn partition_round_robin(files: &[FileEntry], n: usize) -> Vec<Vec<FileEntry>> {
    let mut buckets: Vec<Vec<FileEntry>> = (0..n).map(|_| Vec::new()).collect();
    for (i, entry) in files.iter().enumerate() {
        buckets[i % n].push(entry.clone());
    }
    buckets
}

fn format_files_arg(partition: &[FileEntry]) -> String {
    partition
        .iter()
        .map(|(path, line)| match line {
            Some(n) => format!("{}:{}", path.display(), n),
            None => path.display().to_string(),
        })
        .collect::<Vec<_>>()
        .join(";")
}

fn create_worktrees(
    repo: &Path,
    jobs: usize,
    setup_command: Option<&str>,
) -> Result<Vec<Worktree>, String> {
    let mut worktrees = Vec::with_capacity(jobs);
    for i in 0..jobs {
        let wt = Worktree::create(repo, &format!("w{}", i))?;
        if let Some(cmd) = setup_command {
            if let Err(e) = wt.run_setup(cmd) {
                let mut wt = wt;
                wt.keep_on_failure();
                return Err(format!(
                    "setup failed in worker {} at {}: {}",
                    i,
                    wt.path().display(),
                    e
                ));
            }
        }
        worktrees.push(wt);
    }
    Ok(worktrees)
}

fn spawn_workers(
    exe: &Path,
    worktrees: &[Worktree],
    partitions: &[Vec<FileEntry>],
    coverage_path: &Path,
    input: &ParallelInput<'_>,
) -> Result<(Vec<Child>, Vec<PathBuf>), String> {
    let mut children = Vec::with_capacity(worktrees.len());
    let mut outputs = Vec::with_capacity(worktrees.len());
    for (i, (wt, partition)) in worktrees.iter().zip(partitions.iter()).enumerate() {
        let output_path = std::env::temp_dir().join(format!(
            "transmute-worker-result-{}-{}.json",
            std::process::id(),
            i
        ));
        let files_arg = format_files_arg(partition);
        info!(
            "spawning worker {} ({} files) in {}",
            i,
            partition.len(),
            wt.path().display()
        );

        let child = Command::new(exe)
            .arg("--files")
            .arg(&files_arg)
            .arg("--coverage")
            .arg(coverage_path)
            .arg("--command")
            .arg(input.command)
            .arg("--formatter")
            .arg("json")
            .arg("--output")
            .arg(&output_path)
            .arg("--max-specs-per-mutation")
            .arg(input.max_specs_per_mutation.to_string())
            .arg("--log-level")
            .arg(input.log_level)
            .arg("--timeout")
            .arg(input.timeout.to_string())
            .arg("--seed")
            .arg(input.seed.to_string())
            .arg("--jobs")
            .arg("1")
            .current_dir(wt.path())
            .spawn()
            .map_err(|e| format!("failed to spawn worker {}: {}", i, e))?;

        locked_active_worker_pids().push(child.id());
        children.push(child);
        outputs.push(output_path);
    }
    Ok((children, outputs))
}

fn wait_for_workers(mut children: Vec<Child>) -> bool {
    let mut any_failed = false;
    for (i, child) in children.iter_mut().enumerate() {
        let pid = child.id();
        match child.wait() {
            Ok(status) => {
                locked_active_worker_pids().retain(|p| *p != pid);
                match status.code() {
                    Some(0) | Some(1) => {
                        info!("worker {} exited with code {:?}", i, status.code());
                    }
                    Some(code) => {
                        any_failed = true;
                        warn!(
                            "worker {} exited with infra code {}; its results may be missing or partial",
                            i, code
                        );
                    }
                    None => {
                        any_failed = true;
                        warn!(
                            "worker {} was terminated by signal; its results may be missing or partial",
                            i
                        );
                    }
                }
            }
            Err(e) => {
                locked_active_worker_pids().retain(|p| *p != pid);
                warn!("worker {} wait failed: {}", i, e);
                any_failed = true;
            }
        }
    }
    any_failed
}

#[derive(Deserialize)]
struct WorkerReport {
    analytics: WorkerAnalytics,
}

#[derive(Deserialize)]
struct WorkerAnalytics {
    files_count: usize,
    mutations: Vec<MutationResult>,
}

fn merge_outputs(paths: &[PathBuf]) -> AnalyticsResult {
    let mut merged = AnalyticsResult::start(0);
    for path in paths {
        if !path.exists() {
            warn!("worker did not produce output at {}", path.display());
            continue;
        }
        let content = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                warn!("could not read worker output {}: {}", path.display(), e);
                continue;
            }
        };
        let report: WorkerReport = match serde_json::from_str(&content) {
            Ok(r) => r,
            Err(e) => {
                warn!("worker output {} was not valid JSON: {}", path.display(), e);
                continue;
            }
        };
        merged.files_count += report.analytics.files_count;
        merged.mutations.extend(report.analytics.mutations);
    }
    merged.mutations.sort_by(|a, b| {
        a.file_path
            .cmp(&b.file_path)
            .then_with(|| a.item.line_number.cmp(&b.item.line_number))
            .then_with(|| a.item.start.cmp(&b.item.start))
            .then_with(|| a.item.end.cmp(&b.item.end))
    });
    merged
}
