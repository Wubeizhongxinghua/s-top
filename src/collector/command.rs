use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread;
use std::time::{Duration, Instant};

use serde::Serialize;

#[derive(Debug, Clone, Default)]
pub struct CancelFlag {
    inner: Arc<AtomicBool>,
}

impl CancelFlag {
    pub fn cancel(&self) {
        self.inner.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.inner.load(Ordering::SeqCst)
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub enum CommandStatus {
    Success,
    Failed,
    TimedOut,
    Cancelled,
    SpawnError,
}

#[derive(Debug, Clone, Serialize)]
pub struct CommandCapture {
    pub program: String,
    pub args: Vec<String>,
    pub status: CommandStatus,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u128,
}

impl CommandCapture {
    pub fn ok(&self) -> bool {
        self.status == CommandStatus::Success
    }

    pub fn short_error(&self) -> Option<String> {
        if self.ok() || self.status == CommandStatus::Cancelled {
            return None;
        }

        if self.status == CommandStatus::TimedOut {
            return Some(if self.stdout.trim().is_empty() {
                format!("{} timed out", self.program)
            } else {
                format!("{} timed out; using partial data", self.program)
            });
        }

        let detail = self.stderr.trim();
        Some(if detail.is_empty() {
            format!("{} failed", self.program)
        } else {
            format!(
                "{} failed: {}",
                self.program,
                detail.lines().next().unwrap_or(detail)
            )
        })
    }
}

#[derive(Debug, Clone)]
pub struct CommandRunner {
    timeout: Duration,
    cancel: CancelFlag,
}

impl CommandRunner {
    #[allow(dead_code)]
    pub fn new(timeout: Duration) -> Self {
        Self {
            timeout,
            cancel: CancelFlag::default(),
        }
    }

    pub fn with_cancel(timeout: Duration, cancel: CancelFlag) -> Self {
        Self { timeout, cancel }
    }

    pub fn run(&self, program: &str, args: &[String]) -> CommandCapture {
        let started = Instant::now();
        let mut command = Command::new(program);
        command
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(error) => {
                return CommandCapture {
                    program: program.to_string(),
                    args: args.to_vec(),
                    status: CommandStatus::SpawnError,
                    exit_code: None,
                    stdout: String::new(),
                    stderr: error.to_string(),
                    duration_ms: started.elapsed().as_millis(),
                };
            }
        };

        let stdout_reader = child.stdout.take().map(spawn_reader);
        let stderr_reader = child.stderr.take().map(spawn_reader);

        loop {
            if self.cancel.is_cancelled() {
                let _ = child.kill();
                let _ = child.wait();
                let (stdout, stderr) = finish_readers(stdout_reader, stderr_reader);
                return CommandCapture {
                    program: program.to_string(),
                    args: args.to_vec(),
                    status: CommandStatus::Cancelled,
                    exit_code: None,
                    stdout,
                    stderr,
                    duration_ms: started.elapsed().as_millis(),
                };
            }

            match child.try_wait() {
                Ok(Some(status)) => {
                    let (stdout, stderr) = finish_readers(stdout_reader, stderr_reader);
                    return CommandCapture {
                        program: program.to_string(),
                        args: args.to_vec(),
                        status: if status.success() {
                            CommandStatus::Success
                        } else {
                            CommandStatus::Failed
                        },
                        exit_code: status.code(),
                        stdout,
                        stderr,
                        duration_ms: started.elapsed().as_millis(),
                    };
                }
                Ok(None) if started.elapsed() >= self.timeout => {
                    let _ = child.kill();
                    let _ = child.wait();
                    let (stdout, stderr) = finish_readers(stdout_reader, stderr_reader);
                    return CommandCapture {
                        program: program.to_string(),
                        args: args.to_vec(),
                        status: CommandStatus::TimedOut,
                        exit_code: None,
                        stdout,
                        stderr: if stderr.trim().is_empty() {
                            format!("{} timed out after {:?}", program, self.timeout)
                        } else {
                            stderr
                        },
                        duration_ms: started.elapsed().as_millis(),
                    };
                }
                Ok(None) => thread::sleep(Duration::from_millis(10)),
                Err(error) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    let (stdout, stderr) = finish_readers(stdout_reader, stderr_reader);
                    return CommandCapture {
                        program: program.to_string(),
                        args: args.to_vec(),
                        status: CommandStatus::Failed,
                        exit_code: None,
                        stdout,
                        stderr: if stderr.trim().is_empty() {
                            error.to_string()
                        } else {
                            format!("{}; {}", error, stderr)
                        },
                        duration_ms: started.elapsed().as_millis(),
                    };
                }
            }
        }
    }
}

fn spawn_reader<T>(mut stream: T) -> thread::JoinHandle<Vec<u8>>
where
    T: Read + Send + 'static,
{
    thread::spawn(move || {
        let mut buffer = Vec::new();
        let _ = stream.read_to_end(&mut buffer);
        buffer
    })
}

fn finish_readers(
    stdout_reader: Option<thread::JoinHandle<Vec<u8>>>,
    stderr_reader: Option<thread::JoinHandle<Vec<u8>>>,
) -> (String, String) {
    let stdout = stdout_reader
        .and_then(|reader| reader.join().ok())
        .map(|bytes| String::from_utf8_lossy(&bytes).to_string())
        .unwrap_or_default();
    let stderr = stderr_reader
        .and_then(|reader| reader.join().ok())
        .map(|bytes| String::from_utf8_lossy(&bytes).to_string())
        .unwrap_or_default();
    (stdout, stderr)
}
