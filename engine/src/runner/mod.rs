use log::trace;
use std::io::Read;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

pub const INFRA_EXIT_CODES: &[i32] = &[2, 124, 125, 127];

pub fn is_infra_error(exit_code: i32) -> bool {
    INFRA_EXIT_CODES.contains(&exit_code)
}

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

    let stdout = child.stdout.take().expect("stdout was piped");
    let stderr = child.stderr.take().expect("stderr was piped");
    let stdout_reader = thread::spawn(move || drain(stdout));
    let stderr_reader = thread::spawn(move || drain(stderr));

    let start = Instant::now();
    let exit_code = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status.code().unwrap_or(0),
            Ok(None) => {
                if start.elapsed() >= timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = stdout_reader.join();
                    let _ = stderr_reader.join();
                    return (124, format!("transmute: timed out after {:?}\n", timeout));
                }
                thread::sleep(Duration::from_millis(50));
            }
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = stdout_reader.join();
                let _ = stderr_reader.join();
                return (125, format!("transmute: wait failed: {}\n", e));
            }
        }
    };

    let stdout_buf = stdout_reader.join().unwrap_or_default();
    let _ = stderr_reader.join();

    let stdout = stdout_buf
        .lines()
        .map(|line| format!("{}\n", line))
        .collect::<String>();

    (exit_code, stdout)
}

fn drain<R: Read>(mut r: R) -> String {
    let mut buf = String::new();
    let _ = r.read_to_string(&mut buf);
    buf
}
