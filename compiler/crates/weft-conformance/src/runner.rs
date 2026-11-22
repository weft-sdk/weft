//! Talking to a runner: one JSON document per line, a request and its answer.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{json, Value};

/// Anything that can answer a request: a real runner process, or a fake in a test.
pub trait Runner {
    /// Sends `request` (an object with an `op`) and returns the `result`, or the runner's error.
    fn call(&mut self, request: Value) -> Result<Value, String>;
}

/// A runner started as a process.
pub struct Process {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next: u64,
}

impl Process {
    pub fn start(command: &[String], cwd: &std::path::Path) -> Result<Process, String> {
        let (program, args) = command.split_first().ok_or("the runner has no command")?;
        let mut child = Command::new(program)
            .args(args)
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| format!("could not start `{program}`: {e}"))?;
        let stdin = child.stdin.take().ok_or("no stdin")?;
        let stdout = BufReader::new(child.stdout.take().ok_or("no stdout")?);
        Ok(Process {
            child,
            stdin,
            stdout,
            next: 1,
        })
    }
}

impl Runner for Process {
    fn call(&mut self, mut request: Value) -> Result<Value, String> {
        let id = self.next;
        self.next += 1;
        request["id"] = json!(id);
        writeln!(self.stdin, "{request}")
            .map_err(|e| format!("could not write to the runner: {e}"))?;
        self.stdin.flush().map_err(|e| e.to_string())?;
        let mut line = String::new();
        let read = self
            .stdout
            .read_line(&mut line)
            .map_err(|e| format!("could not read from the runner: {e}"))?;
        if read == 0 {
            return Err("the runner closed its output (did it crash?)".into());
        }
        let answer: Value = serde_json::from_str(line.trim())
            .map_err(|e| format!("the runner's answer is not JSON ({e}): {}", line.trim()))?;
        if answer.get("id") != Some(&json!(id)) {
            return Err(format!(
                "the runner answered request {id} with {}",
                line.trim()
            ));
        }
        match answer.get("ok").and_then(Value::as_bool) {
            Some(true) => Ok(answer.get("result").cloned().unwrap_or(Value::Null)),
            Some(false) => Err(answer
                .get("error")
                .and_then(Value::as_str)
                .unwrap_or("the runner failed without saying why")
                .to_string()),
            None => Err(format!("the runner's answer has no `ok`: {}", line.trim())),
        }
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        // Closing stdin asks it to exit; do not leave it behind if it ignores that.
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
