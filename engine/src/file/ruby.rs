use log::trace;
use rand::seq::SliceRandom;
use random_string::generate;
use regex::Regex;

use crate::file::read_lines;
use crate::file::MutableItem;

static STRING_CHARSET: &str = "abcdefghijklmnopqrstuvwxyz_";
static NUMBER_CHARSET: &str = "1234567890";
static OPERATOR_CHARSET: [&str; 6] = [">", "<", "<=", ">=", "==", "!="];
static EQUALITY_OPERATOR_CHARSET: [&str; 2] = ["==", "!="];

pub fn find_all(file_path: &str, line_number: u32) -> Vec<MutableItem> {
    trace!("searching all mutable items for file {}", file_path);

    let mut mutations: Vec<MutableItem> = Vec::new();
    let mut line_counter: u32 = 0;
    for line_content in read_lines(file_path) {
        line_counter += 1;
        if line_number != 0 && line_number != line_counter {
            continue;
        }
        for (content, replace, start, end) in find_mutable_items(&line_content) {
            mutations.push(MutableItem {
                line_number: line_counter,
                start,
                end,
                implementation: line_content.to_string(),
                content,
                replace,
            });
        }
    }
    mutations
}

fn find_mutable_items(string: &str) -> Vec<(String, String, usize, usize)> {
    let mut items: Vec<(String, String, usize, usize)> = Vec::new();

    for (matched, start) in find_strings(string) {
        let end = start + matched.len();
        let replace = format!(r#""{}""#, generate(6, STRING_CHARSET));
        items.push((matched, replace, start, end));
    }

    for (matched, start) in find_symbols(string) {
        let end = start + matched.len();
        let replace = format!(":{}", generate(6, STRING_CHARSET));
        items.push((matched, replace, start, end));
    }

    for (matched, start) in find_numbers(string) {
        let end = start + matched.len();
        let replace = generate(3, NUMBER_CHARSET);
        items.push((matched, replace, start, end));
    }

    for (matched, start) in find_operators(string) {
        let end = start + matched.len();
        let replace = random_other_operator(&matched, &OPERATOR_CHARSET);
        items.push((matched, replace, start, end));
    }

    for (matched, start) in find_less_than_operators(string) {
        let end = start + matched.len();
        let replace = random_other_operator(&matched, &OPERATOR_CHARSET);
        items.push((matched, replace, start, end));
    }

    for (matched, start) in find_eq_operators(string) {
        let end = start + matched.len();
        let replace = random_other_operator(&matched, &EQUALITY_OPERATOR_CHARSET);
        items.push((matched, replace, start, end));
    }

    dedupe_overlapping(items)
}

fn dedupe_overlapping(
    mut items: Vec<(String, String, usize, usize)>,
) -> Vec<(String, String, usize, usize)> {
    items.sort_by(|a, b| a.2.cmp(&b.2).then(b.3.cmp(&a.3)));
    let mut out: Vec<(String, String, usize, usize)> = Vec::with_capacity(items.len());
    let mut last_end = 0usize;
    for item in items {
        if item.2 >= last_end {
            last_end = item.3;
            out.push(item);
        }
    }
    out
}

fn random_other_operator(current: &str, charset: &[&str]) -> String {
    let candidates: Vec<&&str> = charset.iter().filter(|&&x| x != current).collect();
    match candidates.choose(&mut rand::thread_rng()) {
        Some(s) => s.to_string(),
        None => current.to_string(),
    }
}

// Detects double- and single-quoted Ruby string literals only.
// Does NOT detect: %w[]/%W[] arrays, %q{}/%Q{} forms, <<~/<<- heredocs,
// or interpolation spans inside "#{...}". Mutating those forms reliably
// would need a real Ruby tokenizer.
fn find_strings(string: &str) -> Vec<(String, usize)> {
    let double = Regex::new(r#""(?:[^"\\]|\\.)*""#).unwrap();
    let single = Regex::new(r#"'(?:[^'\\]|\\.)*'"#).unwrap();

    let mut out: Vec<(String, usize)> = Vec::new();
    let mut double_ranges: Vec<(usize, usize)> = Vec::new();
    for m in double.find_iter(string) {
        if m.as_str().len() <= 2 {
            continue;
        }
        trace!("String {} found", m.as_str());
        out.push((m.as_str().to_string(), m.start()));
        double_ranges.push((m.start(), m.end()));
    }
    for m in single.find_iter(string) {
        if m.as_str().len() <= 2 {
            continue;
        }
        let inside = double_ranges
            .iter()
            .any(|(s, e)| m.start() >= *s && m.start() < *e);
        if inside {
            continue;
        }
        trace!("String {} found", m.as_str());
        out.push((m.as_str().to_string(), m.start()));
    }
    out
}

fn find_symbols(string: &str) -> Vec<(String, usize)> {
    let regex = Regex::new(r#"(?:^|[^\w:])(?P<symbol>:\w+)"#).unwrap();
    regex
        .captures_iter(string)
        .filter_map(|cap| {
            let sym = cap.name("symbol")?;
            trace!("Symbol {} found", sym.as_str());
            Some((sym.as_str().to_string(), sym.start()))
        })
        .collect()
}

fn find_numbers(string: &str) -> Vec<(String, usize)> {
    let regex = Regex::new(r#"\d+"#).unwrap();
    let bytes = string.as_bytes();
    regex
        .find_iter(string)
        .filter(|m| {
            let prev = if m.start() > 0 { bytes[m.start() - 1] } else { 0 };
            let next = bytes.get(m.end()).copied().unwrap_or(0);
            let prev_word = prev == b'_' || prev.is_ascii_alphabetic();
            let touches_dot = prev == b'.' || next == b'.';
            let base_prefix = matches!(next, b'x' | b'X' | b'b' | b'B' | b'o' | b'O');
            !(prev_word || touches_dot || base_prefix)
        })
        .map(|m| {
            trace!("Number {} found", m.as_str());
            (m.as_str().to_string(), m.start())
        })
        .collect()
}

fn find_operators(string: &str) -> Vec<(String, usize)> {
    let regex = Regex::new(r#">=|<=|>"#).unwrap();
    regex
        .find_iter(string)
        .map(|m| {
            trace!("Operator {} found", m.as_str());
            (m.as_str().to_string(), m.start())
        })
        .collect()
}

fn find_less_than_operators(string: &str) -> Vec<(String, usize)> {
    let class_inheritance = Regex::new(r"\bclass\s+\w+\s+$").unwrap();
    let bytes = string.as_bytes();
    let mut result = Vec::new();
    for (i, &b) in bytes.iter().enumerate() {
        if b != b'<' {
            continue;
        }
        let prev = if i > 0 { bytes[i - 1] } else { 0 };
        let next = if i + 1 < bytes.len() { bytes[i + 1] } else { 0 };
        if prev == b'<' || next == b'<' {
            continue;
        }
        if next == b'=' {
            continue;
        }
        if class_inheritance.is_match(&string[..i]) {
            continue;
        }
        trace!("Operator < found at {}", i);
        result.push(("<".to_string(), i));
    }
    result
}

fn find_eq_operators(string: &str) -> Vec<(String, usize)> {
    let regex = Regex::new(r#"==|!="#).unwrap();
    regex
        .find_iter(string)
        .map(|m| {
            trace!("Operator {} found", m.as_str());
            (m.as_str().to_string(), m.start())
        })
        .collect()
}
