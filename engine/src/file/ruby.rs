use log::trace;
use random_string::generate;
use regex::Regex;

use crate::file::read_lines;
use crate::file::MutableItem;

static STRING_CHARSET: &str = "abcdefghijklmnopqrstuvwxyz_";
static NUMBER_CHARSET: &str = "1234567890";
static OPERATOR_CHARSET: [&str; 6] = [">", "<", "<=", ">=", "==", "!="];
static EQUALITY_OPERATOR_CHARSET: [&str; 2] = ["==", "!="];

pub fn find_all(file_path: &str, line_number: u16) -> Vec<MutableItem> {
    trace!("searching all mutable items for file {}", file_path);

    let mut mutations: Vec<MutableItem> = Vec::new();

    let mut line_counter: u16 = 0;
    for line_content in read_lines(file_path) {
        line_counter += 1;

        if line_number != 0 && line_number != line_counter {
            continue;
        }

        let result = find_mutable_items(&line_content);
        for item in result {
            let (content, replace) = item;
            mutations.push(MutableItem {
                line_number: line_counter,
                implementation: line_content.to_string(),
                content: content.to_string(),
                replace: replace.to_string(),
            })
        }
    }
    return mutations;
}

fn find_mutable_items(string: &str) -> Vec<(String, String)> {
    let mut items: Vec<(String, String)> = Vec::new();
    for result in find_strings(string) {
        items.push((result, format!(r#""{}""#, generate(6, STRING_CHARSET))));
    }

    for result in find_symbols(string) {
        items.push((result, format!(r#":{}"#, generate(6, STRING_CHARSET))));
    }

    for result in find_numbers(string) {
        items.push((result, format!(r#"{}"#, generate(3, NUMBER_CHARSET))));
    }

    for result in find_operators(string) {
        items.push((
            result.to_string(),
            format!(
                r#"{}"#,
                OPERATOR_CHARSET.iter().find(|&&x| x != result).unwrap()
            ),
        ));
    }

    for result in find_less_than_operators(string) {
        items.push((
            result.to_string(),
            format!(
                r#"{}"#,
                OPERATOR_CHARSET.iter().find(|&&x| x != result).unwrap()
            ),
        ));
    }


    for result in find_eq_operators(string) {
        items.push((
            result.to_string(),
            format!(
                r#"{}"#,
                EQUALITY_OPERATOR_CHARSET.iter().find(|&&x| x != result).unwrap()
            ),
        ));
    }

    return items;
}

fn find_strings(string: &str) -> Vec<String> {
    let mut result: Vec<String> = Vec::new();
    let regex = Regex::new(r#"("[\w\s].+")"#).unwrap();
    for cap in regex.captures_iter(string) {
        trace!("String {} found", &cap[0]);
        result.push(cap[0].to_string())
    }
    return result;
}

fn find_symbols(string: &str) -> Vec<String> {
    let mut result: Vec<String> = Vec::new();
    let regex = Regex::new(r#"([( ])(?P<symbol>:\w+)"#).unwrap();
    for cap in regex.captures_iter(string) {
        trace!("Symbol {} found", &cap["symbol"]);
        result.push(cap["symbol"].to_string())
    }
    return result;
}

fn find_numbers(string: &str) -> Vec<String> {
    let mut result: Vec<String> = Vec::new();
    let regex = Regex::new(r#"(\d+)"#).unwrap();
    for cap in regex.captures_iter(string) {
        trace!("Number {} found", &cap[0]);
        result.push(cap[0].to_string())
    }
    return result;
}

fn find_operators(string: &str) -> Vec<String> {
    let mut result: Vec<String> = Vec::new();
    let regex = Regex::new(r#"(>|>=|<=)"#).unwrap();
    for cap in regex.captures_iter(string) {
        if !string.contains("class") {
            trace!("Operator {} found", &cap[0]);
            result.push(cap[0].to_string())
        }
    }
    return result;
}

fn find_less_than_operators(string: &str) -> Vec<String> {
    let mut result: Vec<String> = Vec::new();
    let regex = Regex::new(r#"(<)"#).unwrap();
    for cap in regex.captures_iter(string) {
        if !string.contains("class") {
            trace!("Operator {} found", &cap[0]);
            result.push(cap[0].to_string())
        }
    }
    return result;
}

fn find_eq_operators(string: &str) -> Vec<String> {
    let mut result: Vec<String> = Vec::new();
    let regex = Regex::new(r#"(==|!=)"#).unwrap();
    for cap in regex.captures_iter(string) {
        trace!("Operator {} found", &cap[0]);
        result.push(cap[0].to_string())
    }
    return result;
}
