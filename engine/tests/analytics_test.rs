use transmute::analytics;
use transmute::file;

fn item(start: usize, replace: &str) -> file::MutableItem {
    file::MutableItem {
        line_number: 1,
        start,
        end: start + 1,
        implementation: "a".to_string(),
        content: "a".to_string(),
        replace: replace.to_string(),
    }
}

#[test]
fn test_failures_callable_via_immutable_binding() {
    let r = analytics::AnalyticsResult::start(0);
    let _: usize = r.failures();
}

#[test]
fn test_failures_does_not_merge_distinct_mutations_with_colliding_replace() {
    let mut r = analytics::AnalyticsResult::start(1);
    let a = file::MutableItem {
        line_number: 1,
        start: 0,
        end: 1,
        implementation: "x".to_string(),
        content: "x".to_string(),
        replace: "AAA".to_string(),
    };
    let b = file::MutableItem {
        line_number: 2,
        start: 0,
        end: 1,
        implementation: "y".to_string(),
        content: "y".to_string(),
        replace: "AAA".to_string(),
    };

    r.add("f.rb", &a, 0, "".to_string());
    r.add("f.rb", &b, 1, "".to_string());

    assert_eq!(
        r.failures(),
        1,
        "Distinct mutations with the same random replace must not merge"
    );
}

#[test]
fn test_failures_counts_groups_where_all_specs_passed() {
    let mut r = analytics::AnalyticsResult::start(1);
    let survived = item(0, "b");
    let killed = item(2, "c");

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
