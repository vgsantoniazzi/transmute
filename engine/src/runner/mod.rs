use log::trace;
use std::process::Command;

pub fn run(command: &str, spec_file: &str) -> (i32, String) {
    let built_command = str::replace(command, "{file}", spec_file);

    let chunks: Vec<&str> = built_command.split_whitespace().collect();

    trace!("Running specs: '{}'", built_command);

    let output = Command::new(&chunks[0])
        .args(&chunks[1..chunks.len()])
        .output()
        .expect("failed run specs");

    let stdout = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| format!("{}\n", line))
        .collect::<String>();

    match output.status.code() {
        Some(code) => return (code, stdout),
        None => return (0, stdout),
    }
}
