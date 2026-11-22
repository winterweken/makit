//! Capture the process's stdout into the Output pane.
//!
//! Task handlers report progress with `println!`, and Python/Blender child
//! processes inherit stdout. While the TUI runs, file descriptor 1 is
//! redirected into a pipe; a reader thread forwards each line as
//! [`Msg::Output`]. Dropping [`StdoutCapture`] restores the original stdout.
//!
//! A task's result travels the same pipe as its output: the worker leaves
//! the result in a [`Mailbox`] and writes an end-of-output marker to stdout
//! ([`announce_done`]). The reader thread turns that marker into
//! [`Msg::TaskFinished`], so the result can never overtake lines the task
//! printed before it finished.

use std::collections::HashMap;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;

use crate::msg::Msg;
use crate::terminal::WORKER_THREAD_PREFIX;

/// Marks the end of a task's output in the captured stream, followed by the
/// task id. Starts with a control character no tool prints; never shown.
pub const DONE_MARKER: &str = "\u{1e}makit-task-done:";

/// The end-of-output marker line for task `id` (without the newline).
pub fn done_line(id: u64) -> String {
    format!("{DONE_MARKER}{id}")
}

/// Write the end-of-output marker for task `id` to stdout.
///
/// This is the one place the TUI prints on purpose: the marker has to go
/// down the same pipe as the task's own output to keep its place in line.
pub fn announce_done(id: u64) {
    println!("{}", done_line(id));
    let _ = io::stdout().flush();
}

/// Results of finished tasks, held until the reader thread reaches the end
/// of each task's output.
#[derive(Debug, Clone, Default)]
pub struct Mailbox(Arc<Mutex<HashMap<u64, Result<String, String>>>>);

impl Mailbox {
    /// Leave the result of task `id` for the reader thread.
    pub fn post(&self, id: u64, result: Result<String, String>) {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(id, result);
    }

    /// Collect the result of task `id`, if it has been posted.
    pub fn take(&self, id: u64) -> Option<Result<String, String>> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&id)
    }
}

/// Active stdout redirection. Restores stdout on drop.
pub struct StdoutCapture {
    _redirect: gag::Redirect<os_pipe::PipeWriter>,
}

impl StdoutCapture {
    /// Redirect stdout and start forwarding lines to `tx`, along with the
    /// results workers leave in `mailbox`.
    pub fn start(tx: Sender<Msg>, mailbox: Mailbox) -> io::Result<Self> {
        io::stdout().flush()?;
        let (reader, writer) = os_pipe::pipe()?;
        let redirect = gag::Redirect::stdout(writer)?;
        thread::Builder::new()
            .name(format!("{WORKER_THREAD_PREFIX}stdout"))
            .spawn(move || forward_lines(reader, &tx, &mailbox))?;
        Ok(Self {
            _redirect: redirect,
        })
    }
}

impl Drop for StdoutCapture {
    fn drop(&mut self) {
        // Push out anything still buffered before the redirect is undone.
        let _ = io::stdout().flush();
    }
}

/// Read `reader` line by line and send each sanitised line to `tx`, until
/// EOF or the receiver goes away. An end-of-output marker for a task whose
/// result is in `mailbox` is sent as that task's [`Msg::TaskFinished`].
pub fn forward_lines<R: Read>(reader: R, tx: &Sender<Msg>, mailbox: &Mailbox) {
    let mut reader = BufReader::new(reader);
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match reader.read_until(b'\n', &mut buf) {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                let text = String::from_utf8_lossy(&buf);
                let delivered = match take_finished(&text, mailbox) {
                    Some((before, id, result)) => {
                        // A child process may have left a line unfinished.
                        (before.is_empty() || tx.send(Msg::Output(sanitize(before))).is_ok())
                            && tx.send(Msg::TaskFinished { id, result }).is_ok()
                    }
                    None => tx.send(Msg::Output(sanitize(&text))).is_ok(),
                };
                if !delivered {
                    break;
                }
            }
        }
    }
}

/// If `line` carries the end-of-output marker of a task whose result is
/// waiting, split it into the output before the marker and that result.
fn take_finished<'a>(
    line: &'a str,
    mailbox: &Mailbox,
) -> Option<(&'a str, u64, Result<String, String>)> {
    let (before, rest) = line.split_once(DONE_MARKER)?;
    let id = rest.trim_end().parse().ok()?;
    let result = mailbox.take(id)?;
    Some((before, id, result))
}

/// Make a captured line safe to draw: drop the line ending, expand tabs,
/// strip ANSI escape sequences and other control characters.
pub fn sanitize(raw: &str) -> String {
    let raw = raw.trim_end_matches(['\n', '\r']);
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\t' => out.push_str("    "),
            '\u{1b}' => {
                // CSI: ESC [ params… final-byte(@..~); otherwise skip one char.
                if chars.peek() == Some(&'[') {
                    chars.next();
                    for c in chars.by_ref() {
                        if ('@'..='~').contains(&c) {
                            break;
                        }
                    }
                } else {
                    chars.next();
                }
            }
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn sanitize_strips_line_endings_and_escapes() {
        assert_eq!(sanitize("hello\r\n"), "hello");
        assert_eq!(sanitize("a\tb"), "a    b");
        assert_eq!(sanitize("\u{1b}[1;32mOK\u{1b}[0m done"), "OK done");
        assert_eq!(sanitize("bell\u{7}!"), "bell!");
        assert_eq!(sanitize("wall ◆ 12.5 m²"), "wall ◆ 12.5 m²");
    }

    #[test]
    fn forwards_each_line_until_eof() {
        let (tx, rx) = mpsc::channel();
        let input: &[u8] = b"first\nsecond\r\nno newline";
        forward_lines(input, &tx, &Mailbox::default());
        drop(tx);
        let lines: Vec<String> = rx
            .iter()
            .map(|m| match m {
                Msg::Output(line) => line,
                other => panic!("unexpected {other:?}"),
            })
            .collect();
        assert_eq!(lines, vec!["first", "second", "no newline"]);
    }

    #[test]
    fn invalid_utf8_is_replaced_not_dropped() {
        let (tx, rx) = mpsc::channel();
        let input: &[u8] = b"bad \xff byte\n";
        forward_lines(input, &tx, &Mailbox::default());
        drop(tx);
        match rx.recv().unwrap() {
            Msg::Output(line) => assert_eq!(line, "bad \u{fffd} byte"),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn stops_when_receiver_is_gone() {
        let (tx, rx) = mpsc::channel();
        drop(rx);
        let input: &[u8] = b"a\nb\nc\n";
        forward_lines(input, &tx, &Mailbox::default()); // must return, not loop forever
    }

    /// What the reader thread sent, as short strings.
    fn forwarded(input: &str, mailbox: &Mailbox) -> Vec<String> {
        let (tx, rx) = mpsc::channel();
        forward_lines(input.as_bytes(), &tx, mailbox);
        drop(tx);
        rx.iter()
            .map(|m| match m {
                Msg::Output(line) => format!("out:{line}"),
                Msg::TaskFinished { id, result } => format!("done:{id}:{result:?}"),
                other => panic!("unexpected {other:?}"),
            })
            .collect()
    }

    #[test]
    fn a_task_result_arrives_after_the_output_written_before_it() {
        let mailbox = Mailbox::default();
        mailbox.post(3, Ok("Success: x".into()));
        let input = format!("line a\nline b\n{}\nlater\n", done_line(3));
        assert_eq!(
            forwarded(&input, &mailbox),
            vec![
                "out:line a",
                "out:line b",
                "done:3:Ok(\"Success: x\")",
                "out:later"
            ]
        );
        // The result is handed over exactly once.
        assert!(mailbox.take(3).is_none());
    }

    #[test]
    fn a_marker_glued_to_unterminated_output_still_finishes_the_task() {
        let mailbox = Mailbox::default();
        mailbox.post(4, Err("Error: x: boom".into()));
        // A child process left a line open before the worker's marker.
        let input = format!("partial{}\n", done_line(4));
        assert_eq!(
            forwarded(&input, &mailbox),
            vec!["out:partial", "done:4:Err(\"Error: x: boom\")"]
        );
    }

    #[test]
    fn a_marker_nobody_posted_is_ordinary_output() {
        let mailbox = Mailbox::default();
        let input = format!("{}\n{DONE_MARKER}not-a-number\n", done_line(9));
        assert_eq!(
            forwarded(&input, &mailbox),
            vec!["out:makit-task-done:9", "out:makit-task-done:not-a-number"]
        );
    }

    #[test]
    fn mailbox_is_shared_between_clones() {
        let mailbox = Mailbox::default();
        mailbox.clone().post(1, Ok("a".into()));
        assert_eq!(mailbox.take(1), Some(Ok("a".to_string())));
        assert_eq!(mailbox.take(1), None);
    }
}
