use transmute::analytics;
use transmute::file;
use transmute::formatter;

use std::path::PathBuf;

fn scratch_path(name: &str, ext: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "transmute_test_formatter_{}_{}.{}",
        std::process::id(),
        name,
        ext
    ))
}

fn analytics_with_one_survivor_and_one_kill() -> analytics::AnalyticsResult {
    let mut r = analytics::AnalyticsResult::start(2);
    let survivor = file::MutableItem {
        line_number: 7,
        start: 0,
        end: 2,
        implementation: "x = 42".to_string(),
        content: "42".to_string(),
        replace: "99".to_string(),
    };
    let killed = file::MutableItem {
        line_number: 9,
        start: 0,
        end: 1,
        implementation: "y = 1".to_string(),
        content: "1".to_string(),
        replace: "2".to_string(),
    };
    r.add("a.rb", &survivor, 0, "survived".to_string(), 1);
    r.add("b.rb", &killed, 1, "killed".to_string(), 1);
    r
}

fn analytics_with_low_confidence_and_uncovered() -> analytics::AnalyticsResult {
    let mut r = analytics::AnalyticsResult::start(2);
    let truncated = file::MutableItem {
        line_number: 5,
        start: 0,
        end: 2,
        implementation: "x = 42".to_string(),
        content: "42".to_string(),
        replace: "99".to_string(),
    };
    let uncovered = file::MutableItem {
        line_number: 11,
        start: 0,
        end: 2,
        implementation: "z = 1".to_string(),
        content: "1".to_string(),
        replace: "8".to_string(),
    };
    r.add("a.rb", &truncated, 0, "survived".to_string(), 7);
    r.add("a.rb", &truncated, 0, "survived".to_string(), 7);
    r.add("a.rb", &truncated, 0, "survived".to_string(), 7);
    r.add("b.rb", &uncovered, 0, "".to_string(), 0);
    r
}

#[test]
fn test_json_formatter_writes_failure_count_and_mutations_list() {
    let out = scratch_path("json_count", "json");
    let r = analytics_with_one_survivor_and_one_kill();

    formatter::generate(r, "json", out.to_str().unwrap());

    let content = std::fs::read_to_string(&out).unwrap();
    let json: serde_json::Value = serde_json::from_str(&content).expect("must be valid JSON");
    assert_eq!(
        json["failures"], 1,
        "Exactly one survivor; JSON: {}",
        content
    );
    let mutations = json["analytics"]["mutations"].as_array().unwrap();
    assert_eq!(mutations.len(), 2, "Both mutation runs must be recorded");
    assert_eq!(mutations[0]["exit_code"], 0);
    assert_eq!(mutations[0]["item"]["content"], "42");
    assert_eq!(mutations[0]["specs_total"], 1);
    assert_eq!(mutations[1]["exit_code"], 1);

    std::fs::remove_file(&out).ok();
}

#[test]
fn test_html_formatter_renders_failure_count_and_mutation_details() {
    let out = scratch_path("html", "html");
    let r = analytics_with_one_survivor_and_one_kill();

    formatter::generate(r, "html", out.to_str().unwrap());

    let html = std::fs::read_to_string(&out).unwrap();
    assert!(html.contains("<html>"), "Expected HTML structure");
    assert!(
        html.contains(">1<"),
        "Failure count 1 must appear as the rendered failures value"
    );
    assert!(
        html.contains("a.rb") && html.contains("b.rb"),
        "Both file paths must appear in the HTML report"
    );
    assert!(
        html.contains("x = 42"),
        "Implementation lines must be rendered"
    );

    std::fs::remove_file(&out).ok();
}

#[test]
fn test_unknown_formatter_falls_back_to_json() {
    let out = scratch_path("fallback", "json");
    let r = analytics_with_one_survivor_and_one_kill();

    formatter::generate(r, "yaml", out.to_str().unwrap());

    let content = std::fs::read_to_string(&out).unwrap();
    let json: serde_json::Value =
        serde_json::from_str(&content).expect("Unknown formatter must fall back to valid JSON");
    assert_eq!(json["failures"], 1);

    std::fs::remove_file(&out).ok();
}

#[test]
fn test_json_formatter_emits_uncovered_and_low_confidence_counts() {
    let out = scratch_path("low_conf_count", "json");
    let r = analytics_with_low_confidence_and_uncovered();

    formatter::generate(r, "json", out.to_str().unwrap());

    let content = std::fs::read_to_string(&out).unwrap();
    let json: serde_json::Value = serde_json::from_str(&content).expect("must be valid JSON");
    assert_eq!(json["failures"], 2, "Both survivors are failures");
    assert_eq!(
        json["uncovered_failures"], 1,
        "specs_total=0 mutation must surface as uncovered; JSON: {}",
        content
    );
    assert_eq!(
        json["low_confidence_failures"], 1,
        "Filtered survivor must surface as low-confidence; JSON: {}",
        content
    );

    std::fs::remove_file(&out).ok();
}

#[test]
fn test_html_formatter_marks_low_confidence_and_uncovered_survivors() {
    let out = scratch_path("html_signals", "html");
    let r = analytics_with_low_confidence_and_uncovered();

    formatter::generate(r, "html", out.to_str().unwrap());

    let html = std::fs::read_to_string(&out).unwrap();
    assert!(
        html.contains("low-confidence"),
        "HTML must mark low-confidence survivors"
    );
    assert!(
        html.contains("uncovered"),
        "HTML must mark uncovered survivors"
    );

    std::fs::remove_file(&out).ok();
}

#[test]
fn test_html_formatter_hides_low_confidence_card_when_count_is_zero() {
    let out = scratch_path("html_hides_zero_low", "html");
    let r = analytics_with_one_survivor_and_one_kill();

    formatter::generate(r, "html", out.to_str().unwrap());

    let html = std::fs::read_to_string(&out).unwrap();
    assert!(
        !html.contains("Low-confidence (filtered"),
        "Low-confidence card must be hidden when count is 0"
    );
    assert!(
        !html.contains("Uncovered (no spec at all)"),
        "Uncovered card must be hidden when count is 0"
    );

    std::fs::remove_file(&out).ok();
}

#[test]
fn test_empty_output_path_uses_default_filename() {
    let original_dir = std::env::current_dir().unwrap();
    let scratch_dir = std::env::temp_dir().join(format!(
        "transmute_formatter_default_{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&scratch_dir).unwrap();
    std::env::set_current_dir(&scratch_dir).unwrap();

    let r = analytics_with_one_survivor_and_one_kill();
    formatter::generate(r, "json", "");

    let default = scratch_dir.join("result.json");
    assert!(
        default.exists(),
        "Empty --output must default to result.json"
    );

    std::env::set_current_dir(&original_dir).unwrap();
    std::fs::remove_dir_all(&scratch_dir).ok();
}
