use transmute::file;

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
    let strings: Vec<_> = items
        .iter()
        .filter(|m| m.content.starts_with('"'))
        .collect();
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
    assert_eq!(
        mutations.len(),
        2,
        "Input has exactly two '12345' tokens; got mutations: {:?}",
        mutations
    );
    assert_eq!(
        (mutations[0].start, mutations[0].end),
        (5, 10),
        "First mutation must point at byte offsets 5..10"
    );
    assert_eq!(
        (mutations[1].start, mutations[1].end),
        (14, 19),
        "Second mutation must point at byte offsets 14..19"
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
fn test_less_than_inside_string_literal_is_skipped_and_real_one_is_mutated() {
    let (path, items) = mutations_for(
        r#"puts "class User < Object" if a < b"#,
        "lt_class_in_string",
    );
    let lt: Vec<_> = items.iter().filter(|m| m.content == "<").collect();
    assert_eq!(
        lt.len(),
        1,
        "Exactly the real '< b' (outside the string) must be mutated; the '<' inside the string literal must be skipped via comment_start scan; got: {:?}",
        items
    );
    let only = lt[0];
    let real_lt_pos = r#"puts "class User < Object" if a < b"#.find("a < b").unwrap() + 2;
    assert_eq!(
        only.start, real_lt_pos,
        "The mutated '<' must be at the real operator's byte offset, not the one inside the string"
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
fn test_strings_treat_escaped_quotes_as_part_of_literal() {
    let (path, items) = mutations_for(r#"puts "He said \"hi\"""#, "escaped_quotes");
    let strings: Vec<_> = items
        .iter()
        .filter(|m| m.content.starts_with('"'))
        .collect();
    assert_eq!(
        strings.len(),
        1,
        "Escaped quotes should not split the literal; got: {:?}",
        strings
    );
    assert_eq!(
        strings[0].content, r#""He said \"hi\"""#,
        "Captured content should be the full literal, not truncated at the first escaped quote"
    );
    std::fs::remove_file(&path).ok();
}

#[test]
fn test_single_quoted_strings_are_detected() {
    let (path, items) = mutations_for("puts 'hello world'", "single_quoted");
    let strings: Vec<_> = items
        .iter()
        .filter(|m| m.content.starts_with('\''))
        .collect();
    assert_eq!(
        strings.len(),
        1,
        "Expected one single-quoted string mutation; got: {:?}",
        items
    );
    std::fs::remove_file(&path).ok();
}

#[test]
fn test_single_quoted_strings_inside_double_quoted_are_ignored() {
    let (path, items) = mutations_for(r#"puts "outer 'inner' tail""#, "nested_quotes");
    let single: Vec<_> = items
        .iter()
        .filter(|m| m.content.starts_with('\''))
        .collect();
    assert!(
        single.is_empty(),
        "'inner' inside a double-quoted literal should not produce its own mutation; got: {:?}",
        single
    );
    std::fs::remove_file(&path).ok();
}

#[test]
fn test_symbols_inside_array_literal_are_detected() {
    let (path, items) = mutations_for("[:a, :b]", "symbols_in_array");
    let syms: Vec<_> = items
        .iter()
        .filter(|m| m.content.starts_with(':'))
        .collect();
    assert_eq!(
        syms.len(),
        2,
        "Expected two symbol mutations; got: {:?}",
        items
    );
    std::fs::remove_file(&path).ok();
}

#[test]
fn test_module_path_double_colon_is_not_a_symbol() {
    let (path, items) = mutations_for("Module::Foo.call", "module_path");
    let syms: Vec<_> = items
        .iter()
        .filter(|m| m.content.starts_with(':'))
        .collect();
    assert!(
        syms.is_empty(),
        "'::Foo' is a constant path, not a symbol; got: {:?}",
        syms
    );
    std::fs::remove_file(&path).ok();
}

#[test]
fn test_numbers_skips_floats_and_hex_literals() {
    let (path, items) = mutations_for("x = 1.5; y = 0xFF; z = 0b101", "float_hex");
    let numbers: Vec<_> = items
        .iter()
        .filter(|m| m.content.chars().all(|c| c.is_ascii_digit()))
        .collect();
    assert!(
        numbers.is_empty(),
        "Digits inside floats/hex/binary literals must not become number mutations; got: {:?}",
        numbers
    );
    std::fs::remove_file(&path).ok();
}

#[test]
fn test_double_quoted_substring_inside_single_quoted_string_is_not_mutated() {
    let (path, items) = mutations_for(r#"puts 'outer "inner" tail'"#, "double_in_single");
    let doubles: Vec<_> = items
        .iter()
        .filter(|m| m.content.starts_with('"'))
        .collect();
    assert!(
        doubles.is_empty(),
        "Double-quoted substring inside single-quoted literal should be skipped; got: {:?}",
        doubles
    );
    std::fs::remove_file(&path).ok();
}

#[test]
fn test_numbers_skip_leading_digit_of_underscore_separated_literal() {
    let (path, items) = mutations_for("x = 1_000_000", "underscore_literal");
    let numbers: Vec<_> = items
        .iter()
        .filter(|m| m.content.chars().all(|c| c.is_ascii_digit()))
        .collect();
    assert!(
        numbers.is_empty(),
        "1_000_000-style literals must not produce piecewise digit mutations; got: {:?}",
        numbers
    );
    std::fs::remove_file(&path).ok();
}

#[test]
fn test_overlapping_mutations_inside_string_literal_are_deduped() {
    let (path, items) = mutations_for(r#"puts "123""#, "digits_inside_string");
    let mut ranges: Vec<(usize, usize, &String)> =
        items.iter().map(|m| (m.start, m.end, &m.content)).collect();
    ranges.sort_by_key(|(s, _, _)| *s);
    for w in ranges.windows(2) {
        assert!(
            w[0].1 <= w[1].0,
            "Overlapping mutation ranges {:?} and {:?}",
            w[0],
            w[1]
        );
    }
    std::fs::remove_file(&path).ok();
}

#[test]
fn test_no_mutations_emitted_for_trailing_comment_text() {
    let (path, items) = mutations_for("x = 5 # threshold == 5", "trailing_comment");
    let comment_mutations: Vec<_> = items.iter().filter(|m| m.start >= 6).collect();
    assert!(
        comment_mutations.is_empty(),
        "Mutations must not be emitted from comment region; got: {:?}",
        comment_mutations
    );
    std::fs::remove_file(&path).ok();
}

#[test]
fn test_hash_inside_string_is_not_treated_as_comment_start() {
    let (path, items) = mutations_for(r##"puts "# not a comment"; x = 5"##, "hash_in_string");
    let numbers: Vec<&file::MutableItem> = items.iter().filter(|m| m.content == "5").collect();
    assert!(
        !numbers.is_empty(),
        "Code after a string containing '#' must still be scanned; got: {:?}",
        items
    );
    std::fs::remove_file(&path).ok();
}

#[test]
fn test_ge_and_le_operators_are_matched_as_pairs() {
    let (path, items) = mutations_for("a >= b && c <= d", "ge_le");
    let op_mutations: Vec<&file::MutableItem> = items
        .iter()
        .filter(|m| ["<", ">", ">=", "<=", "==", "!="].contains(&m.content.as_str()))
        .collect();
    let contents: Vec<&str> = op_mutations.iter().map(|m| m.content.as_str()).collect();
    assert_eq!(
        contents,
        vec![">=", "<="],
        "Exactly two operator mutations expected — the >= and <= pairs, not their constituent < or > characters; got: {:?}",
        op_mutations
    );
    std::fs::remove_file(&path).ok();
}

#[test]
fn test_triple_equals_is_not_mutated() {
    let (path, items) = mutations_for("case x; when String === y then 1 end", "triple_eq");
    let eq_mutations: Vec<&file::MutableItem> = items
        .iter()
        .filter(|m| m.content == "==" || m.content == "!=" || m.content == "===")
        .collect();
    assert!(
        eq_mutations.is_empty(),
        "'===' must not produce '==' or other equality mutations; got: {:?}",
        eq_mutations
    );
    std::fs::remove_file(&path).ok();
}

#[test]
fn test_bang_double_equals_is_not_mutated_as_neq() {
    let (path, items) = mutations_for("if x !== y then 1 end", "bang_eq_eq");
    let eq_mutations: Vec<&file::MutableItem> = items
        .iter()
        .filter(|m| m.content == "==" || m.content == "!=")
        .collect();
    assert!(
        eq_mutations.is_empty(),
        "'!==' must not produce '!=' or '==' mutations; got: {:?}",
        eq_mutations
    );
    std::fs::remove_file(&path).ok();
}

#[test]
fn test_spaceship_is_not_mutated() {
    let (path, items) = mutations_for("a <=> b", "spaceship");
    let op_mutations: Vec<&file::MutableItem> = items
        .iter()
        .filter(|m| ["<", ">", "<=", ">="].contains(&m.content.as_str()))
        .collect();
    assert!(
        op_mutations.is_empty(),
        "'<=>' must not produce any operator mutations; got: {:?}",
        op_mutations
    );
    std::fs::remove_file(&path).ok();
}

#[test]
fn test_arrow_in_hash_rocket_does_not_produce_gt_mutation() {
    let (path, items) = mutations_for("{ :a => 1 }", "arrow");
    let gt_mutations: Vec<&file::MutableItem> = items.iter().filter(|m| m.content == ">").collect();
    assert!(
        gt_mutations.is_empty(),
        "'=>' must not produce a '>' mutation; got: {:?}",
        gt_mutations
    );
    std::fs::remove_file(&path).ok();
}

#[test]
fn test_lambda_arrow_does_not_produce_gt_mutation() {
    let (path, items) = mutations_for("doubler = ->(x) { x * 2 }", "lambda_arrow");
    let gt_mutations: Vec<&file::MutableItem> = items.iter().filter(|m| m.content == ">").collect();
    assert!(
        gt_mutations.is_empty(),
        "'->' (stabby lambda) must not produce a '>' mutation; got: {:?}",
        gt_mutations
    );
    std::fs::remove_file(&path).ok();
}

#[test]
fn test_right_shift_does_not_produce_gt_mutations() {
    let (path, items) = mutations_for("y = x >> 2", "right_shift");
    let op_mutations: Vec<&file::MutableItem> = items
        .iter()
        .filter(|m| ["<", ">", ">=", "<="].contains(&m.content.as_str()))
        .collect();
    assert!(
        op_mutations.is_empty(),
        "'>>' must not produce any '>' or '>=' operator mutations; got: {:?}",
        op_mutations
    );
    std::fs::remove_file(&path).ok();
}

#[test]
fn test_right_shift_assign_does_not_produce_gt_mutations() {
    let (path, items) = mutations_for("x >>= 4", "right_shift_assign");
    let op_mutations: Vec<&file::MutableItem> = items
        .iter()
        .filter(|m| ["<", ">", ">=", "<="].contains(&m.content.as_str()))
        .collect();
    assert!(
        op_mutations.is_empty(),
        "'>>=' must not produce any operator mutations; got: {:?}",
        op_mutations
    );
    std::fs::remove_file(&path).ok();
}
