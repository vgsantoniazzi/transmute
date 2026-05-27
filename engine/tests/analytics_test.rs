#[path = "../src/analytics/mod.rs"]
mod analytics;
#[path = "../src/file/mod.rs"]
mod file;

fn item(replace: &str) -> file::MutableItem {
    file::MutableItem {
        line_number: 1,
        start: 0,
        end: 1,
        implementation: "a".to_string(),
        content: "a".to_string(),
        replace: replace.to_string(),
    }
}

#[test]
fn test_failures_callable_via_immutable_reference() {
    let r = analytics::AnalyticsResult::start(0);
    let _ = (&r).failures();
}

#[test]
fn test_failures_counts_groups_where_all_specs_passed() {
    let mut r = analytics::AnalyticsResult::start(1);
    let survived = item("b");
    let killed = item("c");

    r.add("file.rb", &survived, 0, "out".to_string());
    r.add("file.rb", &survived, 0, "out".to_string());
    r.add("file.rb", &killed, 1, "out".to_string());
    r.add("file.rb", &killed, 0, "out".to_string());

    assert_eq!(
        r.failures(),
        1,
        "Only the all-passed group should count as a surviving mutation"
    );
}
