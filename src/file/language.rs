use log::trace;
use std::fs::File;
use std::io::{self, BufRead};
use std::path::Path;
use regex::Regex;

use crate::file::{MutableItem};

pub fn find_mutations(file_path: String) -> Vec<MutableItem> {
    let signature: Vec<&str> = file_path.split(".").collect();
    match signature[signature.len() - 1] {
        _ => find_all(&file_path),
    }
}

pub fn find_all(file_path: &str) -> Vec<MutableItem> {
    trace!("searching all mutable items for file {}", file_path);

    let mut mutations: Vec<MutableItem> = Vec::new();

    let mut line_counter = 0;
    if let Ok(lines) = read_lines(file_path) {
        for line in lines {
            if let Ok(string) = line {
                line_counter += 1;
                let result = find_mutable_items(&string);
                for item in result {
                    mutations.push(MutableItem {
                        line_number: line_counter,
                        content: item.to_string(),
                    })
                }
            }
        }
    }
    return mutations;
}

fn find_mutable_items(string: &str) -> Vec<String> {
    let mut items: Vec<String> = Vec::new();
    for result in find_strings(string) {
        items.push(result);
    }
    return items;
}

fn find_strings(string: &str) -> Vec<String> {
    let mut result: Vec<String> = Vec::new();
    let regex = Regex::new(r#"("[\w\s]+")"#).unwrap();
    for cap in regex.captures_iter(string) {
        trace!("String {} found", &cap[0]);
        result.push(cap[0].to_string())
    }
    return result;
}

fn read_lines<P>(file_path: P) -> io::Result<io::Lines<io::BufReader<File>>>
where
    P: AsRef<Path>,
{
    let file = File::open(file_path)?;
    Ok(io::BufReader::new(file).lines())
}
