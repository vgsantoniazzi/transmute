use log::trace;
use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub fn run(command: &str, spec_file: &str, timeout: Duration) -> (i32, String) {
    let parts = match shlex::split(command) {
        Some(p) if !p.is_empty() => p,
        _ => {
            return (
                2,
                format!("transmute: cannot parse --command: {}\n", command),
            )
        }
    };
    let argv: Vec<String> = parts
        .into_iter()
        .map(|p| {
            if p == "{file}" {
                spec_file.to_string()
            } else {
                p
            }
        })
        .collect();

    trace!("Running specs: {:?}", argv);

    let mut child = match Command::new(&argv[0])
        .args(&argv[1..])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            return (
                127,
                format!("transmute: failed to spawn '{}': {}\n", argv[0], e),
            );
        }
    };

    let start = Instant::now();
    let exit_code = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status.code().unwrap_or(0),
            Ok(None) => {
                if start.elapsed() >= timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return (124, format!("transmute: timed out after {:?}\n", timeout));
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => panic!("wait failed: {}", e),
        }
    };

    let mut buf = String::new();
    if let Some(mut out) = child.stdout.take() {
        let _ = out.read_to_string(&mut buf);
    }
    let stdout: String = buf
        .lines()
        .map(|line| format!("{}\n", line))
        .collect::<String>();

    (exit_code, stdout)
}
