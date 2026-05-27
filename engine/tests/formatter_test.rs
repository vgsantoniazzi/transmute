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
    r.add("a.rb", &survivor, 0, "survived".to_string(), true);
    r.add("b.rb", &killed, 1, "killed".to_string(), true);
    r
}

fn analytics_with_one_low_confidence_survivor() -> analytics::AnalyticsResult {
    let mut r = analytics::AnalyticsResult::start(1);
    let survivor = file::MutableItem {
        line_number: 5,
        start: 0,
        end: 2,
        implementation: "x = 42".to_string(),
        content: "42".to_string(),
        replace: "99".to_string(),
    };
    r.add("a.rb", &survivor, 0, "survived".to_string(), false);
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
        "Exactly one survivor; killed mutation must not be counted; JSON: {}",
        content
    );
    let mutations = json["analytics"]["mutations"].as_array().unwrap();
    assert_eq!(mutations.len(), 2, "Both mutation runs must be recorded");
    assert_eq!(mutations[0]["exit_code"], 0);
    assert_eq!(mutations[0]["item"]["content"], "42");
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
fn test_json_formatter_emits_low_confidence_failures_field() {
    let out = scratch_path("low_conf_count", "json");
    let r = analytics_with_one_low_confidence_survivor();

    formatter::generate(r, "json", out.to_str().unwrap());

    let content = std::fs::read_to_string(&out).unwrap();
    let json: serde_json::Value = serde_json::from_str(&content).expect("must be valid JSON");
    assert_eq!(json["failures"], 1, "Survivor must count as failure");
    assert_eq!(
        json["low_confidence_failures"], 1,
        "Survivor with coverage_complete=false must count as low-confidence; JSON: {}",
        content
    );
    let mutations = json["analytics"]["mutations"].as_array().unwrap();
    assert_eq!(
        mutations[0]["coverage_complete"], false,
        "Per-mutation coverage_complete flag must be emitted"
    );

    std::fs::remove_file(&out).ok();
}

#[test]
fn test_html_formatter_shows_low_confidence_marker_for_filtered_survivors() {
    let out = scratch_path("html_low_conf", "html");
    let r = analytics_with_one_low_confidence_survivor();

    formatter::generate(r, "html", out.to_str().unwrap());

    let html = std::fs::read_to_string(&out).unwrap();
    assert!(
        html.contains("low-confidence"),
        "HTML must mark low-confidence survivors so users can spot them; got: {}",
        &html[..html.len().min(800)]
    );
    assert!(
        html.contains("Low-confidence (subset of Failures)"),
        "HTML must include a low-confidence count card"
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
