use log::trace;
use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub fn run(command: &str, spec_file: &str, timeout: Duration) -> (i32, String) {
    let built_command = str::replace(command, "{file}", spec_file);
    trace!("Running specs: '{}'", built_command);

    let mut child = Command::new("sh")
        .arg("-c")
        .arg(&built_command)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn shell");

    let start = Instant::now();
    let exit_code = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status.code().unwrap_or(0),
            Ok(None) => {
                if start.elapsed() >= timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return (
                        124,
                        format!("transmute: timed out after {:?}\n", timeout),
                    );
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
