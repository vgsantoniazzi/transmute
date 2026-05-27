#[path = "../src/file/mod.rs"]
mod file;

use std::path::PathBuf;

fn fixture(name: &str, content: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "transmute_test_ruby_{}_{}.rb",
        std::process::id(),
        name
    ));
    std::fs::write(&path, content).unwrap();
    path
}

fn mutations_for(content: &str, name: &str) -> (PathBuf, Vec<file::MutableItem>) {
    let path = fixture(name, content);
    let items = file::File::find_mutations(path.to_str().unwrap().to_string(), 0);
    (path, items)
}

#[test]
fn test_strings_captured_per_literal_not_greedy() {
    let (path, items) = mutations_for(r#"a("foo") + b("bar")"#, "greedy_strings");
    let strings: Vec<_> = items.iter().filter(|m| m.content.starts_with('"')).collect();
    assert_eq!(
        strings.len(),
        2,
        "Expected 2 string literals; got mutations: {:?}",
        items
    );
    std::fs::remove_file(&path).ok();
}

#[test]
fn test_numbers_skip_digits_inside_identifiers() {
    let (path, items) = mutations_for("var_2 = 10", "anchored_numbers");
    let numbers: Vec<_> = items
        .iter()
        .filter(|m| m.content.chars().all(|c| c.is_ascii_digit()))
        .collect();
    assert_eq!(
        numbers.len(),
        1,
        "Only '10' should be mutated; got: {:?}",
        numbers
    );
    assert_eq!(numbers[0].content, "10");
    std::fs::remove_file(&path).ok();
}

#[test]
fn test_mutation_replaces_only_target_occurrence() {
    let (path, items) = mutations_for("puts 12345 || 12345", "position_replace");
    let mutations: Vec<_> = items.iter().filter(|m| m.content == "12345").collect();
    assert!(
        mutations.len() >= 2,
        "Need at least 2 mutations for '12345', got {}",
        mutations.len()
    );

    mutations[0].transmute(path.to_str().unwrap());
    let after = std::fs::read_to_string(&path).unwrap();
    let count = after.matches("12345").count();
    assert_eq!(
        count, 1,
        "Expected exactly one '12345' to remain; file content: {:?}",
        after
    );

    std::fs::remove_file(&path).ok();
}

#[test]
fn test_less_than_mutated_when_class_appears_only_inside_string() {
    let (path, items) = mutations_for(r#"puts "class is X" if a < b"#, "lt_in_string");
    let lt: Vec<_> = items.iter().filter(|m| m.content == "<").collect();
    assert_eq!(
        lt.len(),
        1,
        "Expected '<' to be mutated; got: {:?}",
        items
    );
    std::fs::remove_file(&path).ok();
}

#[test]
fn test_less_than_skipped_for_class_inheritance() {
    let (path, items) = mutations_for("class User < Object", "lt_class");
    let lt: Vec<_> = items.iter().filter(|m| m.content == "<").collect();
    assert!(
        lt.is_empty(),
        "Class inheritance '<' must not be mutated; got: {:?}",
        items
    );
    std::fs::remove_file(&path).ok();
}

#[test]
fn test_less_than_skipped_for_shovel_operator() {
    let (path, items) = mutations_for("[1, 2] << 3", "lt_shovel");
    let lt: Vec<_> = items.iter().filter(|m| m.content == "<").collect();
    assert!(
        lt.is_empty(),
        "'<<' must not produce '<' mutations; got: {:?}",
        items
    );
    std::fs::remove_file(&path).ok();
}

#[test]
fn test_operator_mutation_chooses_diverse_replacement() {
    let mut replacements = std::collections::HashSet::new();
    for i in 0..50 {
        let (path, items) = mutations_for("a > b", &format!("op_diversity_{i}"));
        for item in items.iter().filter(|m| m.content == ">") {
            replacements.insert(item.replace.clone());
        }
        std::fs::remove_file(&path).ok();
    }
    assert!(
        replacements.len() > 1,
        "Operator mutations should be diverse across 50 runs; saw only: {:?}",
        replacements
    );
}

#[test]
fn test_ge_and_le_operators_are_matched_as_pairs() {
    let (path, items) = mutations_for("a >= b && c <= d", "ge_le");
    let contents: Vec<&String> = items.iter().map(|m| &m.content).collect();
    assert!(
        contents.iter().any(|c| c.as_str() == ">="),
        "Expected '>=' as a single token; got: {:?}",
        items
    );
    assert!(
        contents.iter().any(|c| c.as_str() == "<="),
        "Expected '<=' as a single token; got: {:?}",
        items
    );
    std::fs::remove_file(&path).ok();
}
