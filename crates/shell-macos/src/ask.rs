//! Ask (FR-34): hand a typed question to an assistant command-line tool that
//! is already installed (claude, codex, grok) and show what it prints.
//!
//! The question is passed as one argument, never through a shell. The tool
//! runs in an empty scratch directory with no terminal attached, so it cannot
//! prompt and is not pointed at any of the user's files. Nothing it prints is
//! ever executed: the answer is shown and can be copied.

use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const INSTRUCTION: &str = "Answer the question below briefly, as plain text with no Markdown formatting, \
in at most twelve short lines. If the answer is a shell command, put the command alone on the first line \
and explain it in one or two lines after. Do not run tools or modify anything; just answer.\n\nQuestion: ";

pub const DEADLINE: Duration = Duration::from_secs(90);

pub struct Request {
    child: Arc<Mutex<Option<Child>>>,
    result: Receiver<Result<String, String>>,
    pub started: Instant,
}

impl Request {
    /// The answer, if the tool has finished.
    pub fn poll(&self) -> Option<Result<String, String>> {
        self.result.try_recv().ok()
    }

    pub fn cancel(&self) {
        if let Some(mut child) = self.child.lock().ok().and_then(|mut c| c.take()) {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl Drop for Request {
    fn drop(&mut self) {
        self.cancel();
    }
}

/// Start `command` (program followed by arguments; `{question}` marks where
/// the question goes) and return a handle to poll from the main thread.
pub fn start(command: &[String], question: &str) -> Result<Request, String> {
    let (program, args) = command.split_first().ok_or("no command configured")?;
    let prompt = format!("{INSTRUCTION}{question}");
    let args: Vec<String> = args.iter().map(|a| a.replace("{question}", &prompt)).collect();

    let scratch = std::env::temp_dir().join("rusty-spotlight-ask");
    std::fs::create_dir_all(&scratch).map_err(|e| e.to_string())?;
    // A login item starts with a bare PATH; add the usual install locations.
    let home = std::env::var("HOME").unwrap_or_default();
    let path = format!(
        "{home}/.local/bin:/opt/homebrew/bin:/usr/local/bin:{}",
        std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".into())
    );
    let program_path = if program.starts_with("~/") { format!("{home}/{}", &program[2..]) } else { program.clone() };

    let mut child = Command::new(&program_path)
        .args(&args)
        .current_dir(&scratch)
        .env("PATH", path)
        .env("NO_COLOR", "1")
        .env("TERM", "dumb")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("could not start {program}: {e}"))?;

    let mut stdout = child.stdout.take().ok_or("no output pipe")?;
    let mut stderr = child.stderr.take().ok_or("no error pipe")?;
    let child = Arc::new(Mutex::new(Some(child)));
    let (tx, rx) = channel();

    let for_thread = child.clone();
    std::thread::spawn(move || {
        // Drain stderr on its own thread so a chatty tool cannot block on it.
        let errors = std::thread::spawn(move || {
            let mut text = String::new();
            let _ = stderr.read_to_string(&mut text);
            text
        });
        let mut output = String::new();
        let _ = stdout.read_to_string(&mut output);
        let errors = errors.join().unwrap_or_default();
        let status = for_thread.lock().ok().and_then(|mut c| c.take()).map(|mut c| c.wait());
        let answer = clean(&output);
        let result = match status {
            None => return, // cancelled: nobody is waiting
            Some(Ok(s)) if s.success() && !answer.is_empty() => Ok(answer),
            Some(Ok(s)) => {
                let detail = clean(&errors);
                let last = detail.lines().last().unwrap_or("no output").to_string();
                Err(if answer.is_empty() { format!("{last} (exit {})", s.code().unwrap_or(-1)) } else { answer })
            }
            Some(Err(e)) => Err(e.to_string()),
        };
        let _ = tx.send(result);
    });

    Ok(Request { child, result: rx, started: Instant::now() })
}

/// Strip terminal colour codes and surrounding blank lines.
fn clean(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            if chars.peek() == Some(&'[') {
                chars.next();
                for d in chars.by_ref() {
                    if d.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
            continue;
        }
        if c != '\r' {
            out.push(c);
        }
    }
    out.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_colour_codes() {
        assert_eq!(clean("\u{1b}[1mls -la\u{1b}[0m\r\n\n"), "ls -la");
    }

    #[test]
    fn question_is_one_argument_and_is_never_run_by_a_shell() {
        // `echo` prints its arguments; a shell would have expanded `$(…)`.
        let request = start(&["/bin/echo".into(), "{question}".into()], "what is $(whoami); rm -rf x").unwrap();
        let answer = loop {
            if let Some(r) = request.poll() {
                break r.unwrap();
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        assert!(answer.ends_with("Question: what is $(whoami); rm -rf x"));
    }

    #[test]
    fn a_missing_tool_is_reported() {
        assert!(start(&["/nonexistent/tool".into()], "q").is_err());
    }
}
