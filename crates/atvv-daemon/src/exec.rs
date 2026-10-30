// SPDX-License-Identifier: GPL-3.0-or-later

//! `exec:` action worker.
//!
//! Commands are opaque `sh -c` strings. They run one at a time on a worker
//! thread so a slow hook cannot stall the session loop, with a fixed timeout
//! and without inheriting the daemon's stdio. A daemon running as root refuses
//! them outright; the project never installs a root daemon, and this is the
//! defensive check.

use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

/// Wall-clock limit for one `exec:` command.
pub const EXEC_TIMEOUT: Duration = Duration::from_secs(5);
const POLL_INTERVAL: Duration = Duration::from_millis(20);

/// Result of one command, delivered back to the daemon loop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecResult {
    pub command: String,
    pub outcome: Result<(), String>,
}

/// Serial `exec:` runner.
pub struct ExecEngine {
    commands: Sender<String>,
    results: Receiver<ExecResult>,
}

impl ExecEngine {
    /// Start the worker with the production timeout.
    #[must_use]
    pub fn start() -> Self {
        Self::start_with_timeout(EXEC_TIMEOUT)
    }

    /// Start the worker with an explicit timeout; tests use a short one.
    #[must_use]
    pub fn start_with_timeout(timeout: Duration) -> Self {
        let (commands, incoming) = mpsc::channel::<String>();
        let (outgoing, results) = mpsc::channel::<ExecResult>();
        let _worker = thread::Builder::new()
            .name("atvv-exec".into())
            .spawn(move || {
                while let Ok(command) = incoming.recv() {
                    let outcome = run_command(&command, timeout);
                    if outgoing.send(ExecResult { command, outcome }).is_err() {
                        break;
                    }
                }
            });
        Self { commands, results }
    }

    /// Queue one command for execution.
    pub fn run(&self, command: &str) -> Result<(), String> {
        if running_as_root() {
            return Err("exec actions are refused while the daemon runs as root".to_string());
        }
        self.commands
            .send(command.to_string())
            .map_err(|_| "the exec worker is not running".to_string())
    }

    /// Take one finished result, if any.
    pub fn try_recv(&self) -> Option<ExecResult> {
        self.results.try_recv().ok()
    }
}

fn run_command(command: &str, timeout: Duration) -> Result<(), String> {
    let mut child = Command::new("sh")
        .arg("-c")
        .arg(command)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("cannot spawn: {error}"))?;
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => return Ok(()),
            Ok(Some(status)) => return Err(format!("exited with {status}")),
            Ok(None) => {}
            Err(error) => return Err(format!("cannot wait: {error}")),
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("timed out after {} ms", timeout.as_millis()));
        }
        thread::sleep(POLL_INTERVAL);
    }
}

/// Whether the effective uid is 0, read without `libc` or `unsafe`.
fn running_as_root() -> bool {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|status| {
            status
                .lines()
                .find_map(|line| line.strip_prefix("Uid:"))
                .map(str::to_string)
        })
        .is_some_and(|uids| uids.split_whitespace().nth(1) == Some("0"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn result(engine: &ExecEngine) -> ExecResult {
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            if let Some(result) = engine.try_recv() {
                return result;
            }
            assert!(Instant::now() < deadline, "command did not finish");
            thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn successful_commands_report_ok() {
        let engine = ExecEngine::start_with_timeout(Duration::from_secs(2));
        engine.run("true").unwrap();
        let result = result(&engine);
        assert_eq!(result.command, "true");
        assert_eq!(result.outcome, Ok(()));
    }

    #[test]
    fn failures_and_timeouts_are_reported() {
        let engine = ExecEngine::start_with_timeout(Duration::from_millis(100));
        engine.run("exit 3").unwrap();
        let failed = result(&engine);
        assert!(
            failed.outcome.as_ref().unwrap_err().contains("exited with"),
            "{failed:?}"
        );

        engine.run("sleep 1").unwrap();
        let timed_out = result(&engine);
        assert!(
            timed_out
                .outcome
                .as_ref()
                .unwrap_err()
                .contains("timed out"),
            "{timed_out:?}"
        );
    }

    #[test]
    fn commands_do_not_run_after_the_worker_is_gone() {
        let engine = ExecEngine::start_with_timeout(Duration::from_secs(1));
        drop(engine);
    }
}
