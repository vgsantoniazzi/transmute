use assert_cmd::prelude::*;
use rusqlite::{params, Connection};
use std::path::{Path, PathBuf};
use std::process::Command;
use transmute::coverage::schema;

fn init_repo(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "transmute_parallel_test_{}_{}",
        std::process::id(),
        name
    ));
    if dir.exists() {
        std::fs::remove_dir_all(&dir).ok();
    }
    std::fs::create_dir_all(&dir).unwrap();

    run_git(&dir, &["init", "--quiet"]);
    run_git(&dir, &["config", "user.email", "test@example.com"]);
    run_git(&dir, &["config", "user.name", "Test"]);
    run_git(&dir, &["config", "commit.gpgsign", "false"]);
    dir
}

fn run_git(repo: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(args)
        .current_dir(repo)
        .status()
        .unwrap();
    assert!(status.success(), "git {:?} failed", args);
}

fn insert_one(conn: &Connection, file: &str, line: u32, spec: &str) {
    conn.execute(
        "INSERT OR IGNORE INTO files (path) VALUES (?1)",
        params![file],
    )
    .unwrap();
    conn.execute(
        "INSERT OR IGNORE INTO specs (path) VALUES (?1)",
        params![spec],
    )
    .unwrap();
    let file_id: i64 = conn
        .query_row("SELECT id FROM files WHERE path = ?1", params![file], |r| {
            r.get(0)
        })
        .unwrap();
    let spec_id: i64 = conn
        .query_row("SELECT id FROM specs WHERE path = ?1", params![spec], |r| {
            r.get(0)
        })
        .unwrap();
    conn.execute(
        "INSERT OR IGNORE INTO coverage (file_id, line, spec_id) VALUES (?1, ?2, ?3)",
        params![file_id, line, spec_id],
    )
    .unwrap();
}

fn build_repo_with_coverage(name: &str) -> (PathBuf, PathBuf) {
    let repo = init_repo(name);
    std::fs::write(repo.join("a.rb"), "puts 42\n").unwrap();
    std::fs::write(repo.join("b.rb"), "puts 7\n").unwrap();

    let cov_path = repo.join("transmute.sqlite");
    let conn = Connection::open(&cov_path).unwrap();
    schema::initialize(&conn).unwrap();
    let repo_str = repo.display().to_string();
    conn.execute(
        "INSERT OR REPLACE INTO schema_meta (key, value) VALUES ('cwd', ?1)",
        params![repo_str],
    )
    .unwrap();
    insert_one(&conn, &format!("{}/a.rb", repo_str), 1, "spec_a.rb");
    insert_one(&conn, &format!("{}/b.rb", repo_str), 1, "spec_b.rb");
    drop(conn);

    run_git(&repo, &["add", "."]);
    run_git(&repo, &["commit", "-q", "-m", "src+cov"]);

    (repo, cov_path)
}

#[test]
fn test_jobs_2_runs_workers_and_merges_results() {
    let (repo, cov) = build_repo_with_coverage("happy");
    let output = repo.join("result.json");

    let result = Command::cargo_bin("transmute")
        .unwrap()
        .arg("--files")
        .arg("*.rb")
        .arg("--coverage")
        .arg(&cov)
        .arg("--command")
        .arg("sh -c true")
        .arg("--output")
        .arg(&output)
        .arg("--jobs")
        .arg("2")
        .arg("--log-level")
        .arg("warn")
        .current_dir(&repo)
        .output()
        .unwrap();

    assert!(
        output.exists(),
        "parallel run must write merged report; stderr: {}",
        String::from_utf8_lossy(&result.stderr)
    );

    let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&output).unwrap())
        .expect("merged result must be valid JSON");
    let mutations = json["analytics"]["mutations"].as_array().unwrap();

    let file_paths: std::collections::HashSet<&str> = mutations
        .iter()
        .filter_map(|m| m["file_path"].as_str())
        .collect();
    assert!(
        file_paths.iter().any(|p| p.ends_with("a.rb")),
        "merged report must contain mutations from a.rb; got: {:?}",
        file_paths
    );
    assert!(
        file_paths.iter().any(|p| p.ends_with("b.rb")),
        "merged report must contain mutations from b.rb; got: {:?}",
        file_paths
    );

    let mut keys: Vec<(String, u64)> = mutations
        .iter()
        .map(|m| {
            (
                m["file_path"].as_str().unwrap().to_string(),
                m["item"]["line_number"].as_u64().unwrap(),
            )
        })
        .collect();
    let original = keys.clone();
    keys.sort();
    assert_eq!(
        original, keys,
        "merged mutations must be sorted by (file_path, line) for deterministic output"
    );

    std::fs::remove_dir_all(&repo).ok();
}

#[test]
fn test_jobs_refuses_dirty_working_tree() {
    let (repo, cov) = build_repo_with_coverage("dirty");
    std::fs::write(repo.join("dirty.txt"), "wip\n").unwrap();

    let output = Command::cargo_bin("transmute")
        .unwrap()
        .arg("--files")
        .arg("*.rb")
        .arg("--coverage")
        .arg(&cov)
        .arg("--command")
        .arg("sh -c true")
        .arg("--jobs")
        .arg("2")
        .arg("--log-level")
        .arg("warn")
        .current_dir(&repo)
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(2), "dirty tree must exit 2");
    assert!(
        stderr.contains("uncommitted changes") && stderr.contains("Commit or stash"),
        "Error must explain the dirty-tree refusal; stderr: {}",
        stderr
    );

    std::fs::remove_dir_all(&repo).ok();
}

#[test]
fn test_jobs_preserves_line_suffix_in_files_pattern() {
    let (repo, cov) = build_repo_with_coverage("line_suffix");
    std::fs::write(repo.join("multi.rb"), "puts 1\nputs 2\nputs 3\n").unwrap();
    let conn = Connection::open(&cov).unwrap();
    insert_one(
        &conn,
        &format!("{}/multi.rb", repo.display()),
        2,
        "spec_multi.rb",
    );
    drop(conn);
    run_git(&repo, &["add", "."]);
    run_git(&repo, &["commit", "-q", "-m", "multi"]);
    let output = repo.join("result.json");

    Command::cargo_bin("transmute")
        .unwrap()
        .arg("--files")
        .arg("multi.rb:2")
        .arg("--coverage")
        .arg(&cov)
        .arg("--command")
        .arg("sh -c true")
        .arg("--output")
        .arg(&output)
        .arg("--jobs")
        .arg("2")
        .arg("--log-level")
        .arg("warn")
        .current_dir(&repo)
        .output()
        .unwrap();

    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&output).unwrap()).unwrap();
    let mutations = json["analytics"]["mutations"].as_array().unwrap();

    let lines: Vec<u64> = mutations
        .iter()
        .filter(|m| {
            m["file_path"]
                .as_str()
                .map(|p| p.ends_with("multi.rb"))
                .unwrap_or(false)
        })
        .map(|m| m["item"]["line_number"].as_u64().unwrap())
        .collect();
    assert!(
        !lines.is_empty(),
        "line-suffix run must produce at least one mutation on multi.rb"
    );
    assert!(
        lines.iter().all(|&n| n == 2),
        "all multi.rb mutations must target line 2; got: {:?}",
        lines
    );

    std::fs::remove_dir_all(&repo).ok();
}

#[test]
fn test_jobs_flags_failure_when_worker_does_not_write_output() {
    let (repo, cov) = build_repo_with_coverage("worker_no_output");
    let output = repo.join("result.json");

    let result = Command::cargo_bin("transmute")
        .unwrap()
        .arg("--files")
        .arg("*.rb")
        .arg("--coverage")
        .arg(&cov)
        .arg("--command")
        .arg("sh -c 'kill -9 $PPID'")
        .arg("--output")
        .arg(&output)
        .arg("--jobs")
        .arg("2")
        .arg("--log-level")
        .arg("warn")
        .current_dir(&repo)
        .output()
        .unwrap();

    assert_eq!(
        result.status.code(),
        Some(2),
        "worker SIGKILL must cause orchestrator to exit 2; stderr: {}",
        String::from_utf8_lossy(&result.stderr)
    );

    std::fs::remove_dir_all(&repo).ok();
}

#[test]
fn test_jobs_refuses_coverage_without_capture_cwd() {
    let repo = init_repo("no_cwd");
    std::fs::write(repo.join("a.rb"), "puts 1\n").unwrap();
    let cov_path = repo.join("transmute.sqlite");
    let conn = Connection::open(&cov_path).unwrap();
    schema::initialize(&conn).unwrap();
    insert_one(&conn, &format!("{}/a.rb", repo.display()), 1, "spec_a.rb");
    drop(conn);
    run_git(&repo, &["add", "."]);
    run_git(&repo, &["commit", "-q", "-m", "no cwd cov"]);

    let output = Command::cargo_bin("transmute")
        .unwrap()
        .arg("--files")
        .arg("*.rb")
        .arg("--coverage")
        .arg(&cov_path)
        .arg("--command")
        .arg("sh -c true")
        .arg("--jobs")
        .arg("2")
        .arg("--log-level")
        .arg("warn")
        .current_dir(&repo)
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(2));
    assert!(
        stderr.contains("transmute-ruby 0.3+") && stderr.contains("schema_meta"),
        "Error must explain that gem upgrade is required; stderr: {}",
        stderr
    );

    std::fs::remove_dir_all(&repo).ok();
}
