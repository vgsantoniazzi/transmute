use log::trace;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use regex::Regex;
use std::sync::{Mutex, OnceLock};

use crate::file::read_lines;
use crate::file::MutableItem;

static RNG: OnceLock<Mutex<StdRng>> = OnceLock::new();

pub fn init_rng(seed: Option<u64>) {
    let rng_value = seed
        .map(StdRng::seed_from_u64)
        .unwrap_or_else(StdRng::from_entropy);
    let _ = RNG.set(Mutex::new(rng_value));
}

fn with_rng<F, R>(f: F) -> R
where
    F: FnOnce(&mut StdRng) -> R,
{
    let cell = RNG.get_or_init(|| Mutex::new(StdRng::from_entropy()));
    let mut guard = match cell.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    f(&mut guard)
}

fn generate(n: usize, charset: &str) -> String {
    let chars: Vec<char> = charset.chars().collect();
    with_rng(|r| (0..n).map(|_| chars[r.gen_range(0..chars.len())]).collect())
}

fn re_double() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r#""(?:[^"\\]|\\.)*""#).unwrap())
}
fn re_single() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r#"'(?:[^'\\]|\\.)*'"#).unwrap())
}
fn re_symbol() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r#"(?:^|[^\w:])(?P<symbol>:\w+)"#).unwrap())
}
fn re_number() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r#"\d+"#).unwrap())
}
fn re_op_general() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r#">=|<=|>"#).unwrap())
}
fn re_op_eq() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r#"==|!="#).unwrap())
}
fn re_class_inheritance() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"\bclass\s+\w+\s+").unwrap())
}

static STRING_CHARSET: &str = "abcdefghijklmnopqrstuvwxyz_";
static NUMBER_CHARSET: &str = "1234567890";
static NUMBER_LEADING_CHARSET: &str = "123456789";
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
    let scan = &string[..comment_start(string)];
    let mut items: Vec<(String, String, usize, usize)> = Vec::new();

    for (matched, start) in find_strings(scan) {
        let end = start + matched.len();
        let replace = format!(r#""{}""#, generate(6, STRING_CHARSET));
        items.push((matched, replace, start, end));
    }

    for (matched, start) in find_symbols(scan) {
        let end = start + matched.len();
        let replace = format!(":{}", generate(6, STRING_CHARSET));
        items.push((matched, replace, start, end));
    }

    for (matched, start) in find_numbers(scan) {
        let end = start + matched.len();
        let mut replace = format!(
            "{}{}",
            generate(1, NUMBER_LEADING_CHARSET),
            generate(2, NUMBER_CHARSET)
        );
        let mut attempts = 0;
        while replace == matched && attempts < 8 {
            replace = format!(
                "{}{}",
                generate(1, NUMBER_LEADING_CHARSET),
                generate(2, NUMBER_CHARSET)
            );
            attempts += 1;
        }
        items.push((matched, replace, start, end));
    }

    for (matched, start) in find_operators(scan) {
        let end = start + matched.len();
        let replace = random_other_operator(&matched, &OPERATOR_CHARSET);
        items.push((matched, replace, start, end));
    }

    for (matched, start) in find_less_than_operators(scan) {
        let end = start + matched.len();
        let replace = random_other_operator(&matched, &OPERATOR_CHARSET);
        items.push((matched, replace, start, end));
    }

    for (matched, start) in find_eq_operators(scan) {
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

fn comment_start(string: &str) -> usize {
    let double = re_double();
    let single = re_single();
    let mut ranges: Vec<(usize, usize)> = Vec::new();
    for m in double.find_iter(string) {
        ranges.push((m.start(), m.end()));
    }
    for m in single.find_iter(string) {
        ranges.push((m.start(), m.end()));
    }
    for (i, b) in string.bytes().enumerate() {
        if b != b'#' {
            continue;
        }
        if ranges.iter().any(|(s, e)| i >= *s && i < *e) {
            continue;
        }
        return i;
    }
    string.len()
}

fn random_other_operator(current: &str, charset: &[&str]) -> String {
    let candidates: Vec<&&str> = charset.iter().filter(|&&x| x != current).collect();
    if candidates.is_empty() {
        return current.to_string();
    }
    with_rng(|r| {
        let idx = r.gen_range(0..candidates.len());
        candidates[idx].to_string()
    })
}

// Detects double- and single-quoted Ruby string literals only.
// Does NOT detect: %w[]/%W[] arrays, %q{}/%Q{} forms, <<~/<<- heredocs,
// or interpolation spans inside "#{...}". Mutating those forms reliably
// would need a real Ruby tokenizer.
fn find_strings(string: &str) -> Vec<(String, usize)> {
    let double = re_double();
    let single = re_single();

    let single_ranges: Vec<(usize, usize)> = single
        .find_iter(string)
        .filter(|m| m.as_str().len() > 2)
        .map(|m| (m.start(), m.end()))
        .collect();
    let double_ranges: Vec<(usize, usize)> = double
        .find_iter(string)
        .filter(|m| m.as_str().len() > 2)
        .map(|m| (m.start(), m.end()))
        .collect();

    let mut out: Vec<(String, usize)> = Vec::new();
    for m in double.find_iter(string) {
        if m.as_str().len() <= 2 {
            continue;
        }
        let inside_single = single_ranges
            .iter()
            .any(|(s, e)| m.start() >= *s && m.start() < *e);
        if inside_single {
            continue;
        }
        trace!("String {} found", m.as_str());
        out.push((m.as_str().to_string(), m.start()));
    }
    for m in single.find_iter(string) {
        if m.as_str().len() <= 2 {
            continue;
        }
        let inside_double = double_ranges
            .iter()
            .any(|(s, e)| m.start() >= *s && m.start() < *e);
        if inside_double {
            continue;
        }
        trace!("String {} found", m.as_str());
        out.push((m.as_str().to_string(), m.start()));
    }
    out
}

fn find_symbols(string: &str) -> Vec<(String, usize)> {
    re_symbol()
        .captures_iter(string)
        .filter_map(|cap| {
            let sym = cap.name("symbol")?;
            trace!("Symbol {} found", sym.as_str());
            Some((sym.as_str().to_string(), sym.start()))
        })
        .collect()
}

fn find_numbers(string: &str) -> Vec<(String, usize)> {
    let bytes = string.as_bytes();
    re_number()
        .find_iter(string)
        .filter(|m| {
            let prev = if m.start() > 0 {
                bytes[m.start() - 1]
            } else {
                0
            };
            let next = bytes.get(m.end()).copied().unwrap_or(0);
            let next_next = bytes.get(m.end() + 1).copied().unwrap_or(0);
            let prev_word = prev == b'_' || prev.is_ascii_alphabetic();
            let touches_dot = prev == b'.' || next == b'.';
            let base_prefix = matches!(next, b'x' | b'X' | b'b' | b'B' | b'o' | b'O');
            let underscore_digit = next == b'_' && next_next.is_ascii_digit();
            !(prev_word || touches_dot || base_prefix || underscore_digit)
        })
        .map(|m| {
            trace!("Number {} found", m.as_str());
            (m.as_str().to_string(), m.start())
        })
        .collect()
}

fn find_operators(string: &str) -> Vec<(String, usize)> {
    let bytes = string.as_bytes();
    re_op_general()
        .find_iter(string)
        .filter(|m| !is_part_of_spaceship_or_arrow(bytes, m.start(), m.as_str().len()))
        .map(|m| {
            trace!("Operator {} found", m.as_str());
            (m.as_str().to_string(), m.start())
        })
        .collect()
}

fn find_less_than_operators(string: &str) -> Vec<(String, usize)> {
    let bytes = string.as_bytes();
    let inheritance_pos: Option<usize> = re_class_inheritance().find(string).map(|m| m.end());
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
        if inheritance_pos == Some(i) {
            continue;
        }
        trace!("Operator < found at {}", i);
        result.push(("<".to_string(), i));
    }
    result
}

fn is_part_of_spaceship_or_arrow(bytes: &[u8], pos: usize, len: usize) -> bool {
    let prev = if pos > 0 { bytes[pos - 1] } else { 0 };
    let next = bytes.get(pos + len).copied().unwrap_or(0);
    prev == b'<' || next == b'>'
}

fn is_part_of_triple_equals(bytes: &[u8], pos: usize, len: usize) -> bool {
    let prev = if pos > 0 { bytes[pos - 1] } else { 0 };
    let next = bytes.get(pos + len).copied().unwrap_or(0);
    prev == b'=' || next == b'='
}

fn find_eq_operators(string: &str) -> Vec<(String, usize)> {
    let bytes = string.as_bytes();
    re_op_eq()
        .find_iter(string)
        .filter(|m| !is_part_of_triple_equals(bytes, m.start(), m.as_str().len()))
        .map(|m| {
            trace!("Operator {} found", m.as_str());
            (m.as_str().to_string(), m.start())
        })
        .collect()
}
