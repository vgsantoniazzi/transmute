use transmute::coverage::proximity_rank;

#[test]
fn test_rank_zero_for_ruby_rails_layout() {
    assert_eq!(
        proximity_rank("./spec/models/user_spec.rb", "app/models/user.rb"),
        0
    );
    assert_eq!(
        proximity_rank(
            "spec/models/billing/invoice_spec.rb",
            "app/models/billing/invoice.rb"
        ),
        0
    );
}

#[test]
fn test_rank_zero_for_plain_ruby_lib_to_spec() {
    assert_eq!(
        proximity_rank("./spec/foo/bar_spec.rb", "lib/foo/bar.rb"),
        0
    );
}

#[test]
fn test_rank_zero_for_python_pytest_layout() {
    assert_eq!(proximity_rank("tests/pkg/test_user.py", "pkg/user.py"), 0);
}

#[test]
fn test_rank_zero_for_javascript_jest_layout() {
    assert_eq!(
        proximity_rank(
            "tests/components/Button.test.js",
            "src/components/Button.js"
        ),
        0
    );
}

#[test]
fn test_rank_zero_for_go_collocated_test() {
    assert_eq!(proximity_rank("pkg/foo_test.go", "pkg/foo.go"), 0);
}

#[test]
fn test_rank_one_for_same_basename_in_different_dir() {
    assert_eq!(
        proximity_rank("./spec/requests/api/v1/user_spec.rb", "app/models/user.rb"),
        1
    );
}

#[test]
fn test_rank_two_for_same_dir_different_basename() {
    assert_eq!(
        proximity_rank(
            "./spec/models/account_insight_spec.rb",
            "app/models/user.rb"
        ),
        2
    );
}

#[test]
fn test_rank_three_for_unrelated_spec() {
    assert_eq!(
        proximity_rank(
            "./spec/initializers/logging_spec.rb",
            "app/models/bulk_contact_rule.rb"
        ),
        3
    );
    assert_eq!(
        proximity_rank(
            "./spec/jobs/billing/sync_paid_invoices_job_spec.rb",
            "app/models/user.rb"
        ),
        3
    );
}

#[test]
fn test_rank_handles_absolute_spec_paths() {
    assert_eq!(
        proximity_rank("/abs/path/spec/models/user_spec.rb", "app/models/user.rb"),
        0
    );
}

#[test]
fn test_rank_handles_source_at_repo_root() {
    assert_eq!(proximity_rank("./spec/foo_spec.rb", "foo.rb"), 0);
}

#[test]
fn test_strip_handles_uppercase_test_marker_case_insensitively() {
    assert_eq!(proximity_rank("tests/Foo.Test.js", "src/Foo.js"), 0);
}

#[test]
fn test_marker_strip_does_not_match_bare_words() {
    assert_eq!(
        proximity_rank("./spec/test_helpers_spec.rb", "app/models/user.rb"),
        3,
        "A spec whose name happens to contain 'test' but isn't a test marker must not falsely match source stem"
    );
}
