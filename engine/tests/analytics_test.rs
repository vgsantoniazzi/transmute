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

    let survivors: Vec<u32> = r
        .mutations
        .iter()
        .filter(|m| m.exit_code == 0)
        .map(|m| m.item.line_number)
        .collect();
    assert_eq!(
        survivors,
        vec![1],
        "Only the line-1 mutation should be recorded as surviving (exit_code 0)"
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

#[test]
fn test_infra_only_run_is_not_counted_as_survivor() {
    let mut r = analytics::AnalyticsResult::start(1);
    let only_infra = item(0, "z");

    r.add("file.rb", &only_infra, 127, "spawn fail".to_string());
    r.add("file.rb", &only_infra, 124, "timeout".to_string());

    assert_eq!(
        r.failures(),
        0,
        "A mutation that only ever saw infra exit codes must be inconclusive, not a survivor"
    );
}

#[test]
fn test_mixed_infra_and_real_run_counts_only_when_real_run_passes() {
    let mut r = analytics::AnalyticsResult::start(1);
    let m = item(0, "z");

    r.add("file.rb", &m, 127, "spawn fail".to_string());
    r.add("file.rb", &m, 0, "real pass".to_string());

    assert_eq!(
        r.failures(),
        1,
        "Once a real run is observed and it passes, the mutation is a survivor"
    );
}
