use log::trace;
use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

pub const INFRA_EXIT_CODES: &[i32] = &[2, 124, 125, 127];

pub fn is_infra_error(exit_code: i32) -> bool {
    INFRA_EXIT_CODES.contains(&exit_code)
}

static ACTIVE_CHILD_PID: OnceLock<Mutex<Option<u32>>> = OnceLock::new();

fn locked_active_child_pid() -> std::sync::MutexGuard<'static, Option<u32>> {
    let cell = ACTIVE_CHILD_PID.get_or_init(|| Mutex::new(None));
    match cell.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    }
}

pub fn kill_active_child() {
    let pid = locked_active_child_pid().take();
    if let Some(pid) = pid {
        let _ = Command::new("kill")
            .arg("-TERM")
            .arg(pid.to_string())
            .status();
    }
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

    *locked_active_child_pid() = Some(child.id());

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
                    *locked_active_child_pid() = None;
                    let _ = stdout_reader.join();
                    let _ = stderr_reader.join();
                    return (124, format!("transmute: timed out after {:?}\n", timeout));
                }
                thread::sleep(Duration::from_millis(50));
            }
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                *locked_active_child_pid() = None;
                let _ = stdout_reader.join();
                let _ = stderr_reader.join();
                return (125, format!("transmute: wait failed: {}\n", e));
            }
        }
    };

    *locked_active_child_pid() = None;

    let stdout_buf = stdout_reader.join().unwrap_or_default();
    let stderr_buf = stderr_reader.join().unwrap_or_default();

    let stdout = if stderr_buf.trim().is_empty() {
        stdout_buf
    } else {
        format!("{}\n--- stderr ---\n{}", stdout_buf, stderr_buf)
    };

    (exit_code, stdout)
}

fn drain<R: Read>(mut r: R) -> String {
    let mut buf: Vec<u8> = Vec::new();
    let _ = r.read_to_end(&mut buf);
    String::from_utf8_lossy(&buf).into_owned()
}
