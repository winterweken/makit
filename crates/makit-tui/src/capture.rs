//! Capture the process's stdout into the Output pane.
//!
//! Task handlers report progress with `println!`, and Python/Blender child
//! processes inherit stdout. While the TUI runs, file descriptor 1 is
//! redirected into a pipe; a reader thread forwards each line as
//! [`Msg::Output`]. Dropping [`StdoutCapture`] restores the original stdout.

use std::io::{self, BufRead, BufReader, Read, Write};
use std::sync::mpsc::Sender;
use std::thread;

use crate::msg::Msg;
use crate::terminal::WORKER_THREAD_PREFIX;

/// Active stdout redirection. Restores stdout on drop.
pub struct StdoutCapture {
    _redirect: gag::Redirect<os_pipe::PipeWriter>,
}

impl StdoutCapture {
    /// Redirect stdout and start forwarding lines to `tx`.
    pub fn start(tx: Sender<Msg>) -> io::Result<Self> {
        io::stdout().flush()?;
        let (reader, writer) = os_pipe::pipe()?;
        let redirect = gag::Redirect::stdout(writer)?;
        thread::Builder::new()
            .name(format!("{WORKER_THREAD_PREFIX}stdout"))
            .spawn(move || forward_lines(reader, &tx))?;
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
/// EOF or the receiver goes away.
pub fn forward_lines<R: Read>(reader: R, tx: &Sender<Msg>) {
    let mut reader = BufReader::new(reader);
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match reader.read_until(b'\n', &mut buf) {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                let line = sanitize(&String::from_utf8_lossy(&buf));
                if tx.send(Msg::Output(line)).is_err() {
                    break;
                }
            }
        }
    }
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
        forward_lines(input, &tx);
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
        forward_lines(input, &tx);
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
        forward_lines(input, &tx); // must return, not loop forever
    }
}
