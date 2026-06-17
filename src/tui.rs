use anyhow::Result;
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Read, Write, stdout};
use std::process::Command;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

const HIDE_CURSOR: &str = "\x1b[?25l";
const SHOW_CURSOR: &str = "\x1b[?25h";
const CLEAR_LINE: &str = "\x1b[2K";
const COLLAPSED_LINES: usize = 3;
const EXPAND_HINT: &str = "Building...  Ctrl-O expand";
const COLLAPSE_HINT: &str = "Building...  Ctrl-O collapse";

#[derive(Debug, Default, PartialEq)]
struct Frame {
    commit: Vec<String>,
    live: Vec<String>,
}

struct BuildLogView {
    buffer: Vec<String>,
    expanded: bool,
    emitted: usize,
    width: usize,
}

impl BuildLogView {
    fn new(width: usize) -> Self {
        Self {
            buffer: Vec::new(),
            expanded: false,
            emitted: 0,
            width,
        }
    }

    fn on_lines(mut self, lines: Vec<String>) -> (Self, Frame) {
        self.buffer.extend(lines.into_iter().map(|line| {
            let mut stripped = String::with_capacity(line.len());
            let mut chars = line.chars();
            while let Some(c) = chars.next() {
                if c != '\x1b' {
                    stripped.push(c);
                    continue;
                }
                match chars.next() {
                    Some('[') => {
                        for c2 in chars.by_ref() {
                            if c2.is_ascii_alphabetic() {
                                break;
                            }
                        }
                    }
                    Some(']') => {
                        for c2 in chars.by_ref() {
                            if c2 == '\x07' || c2 == '\u{9C}' {
                                break;
                            }
                        }
                    }
                    _ => {}
                }
            }
            stripped
        }));
        self.advance()
    }

    fn toggle(mut self) -> (Self, Frame) {
        self.expanded = !self.expanded;
        self.advance()
    }

    fn advance(mut self) -> (Self, Frame) {
        let (commit, live) = if self.expanded {
            let committed = self.buffer[self.emitted..].to_vec();
            self.emitted = self.buffer.len();
            (committed, vec![COLLAPSE_HINT.to_string()])
        } else {
            let mut live = collapsed_window(&self.buffer, self.width);
            live.push(EXPAND_HINT.to_string());
            (Vec::new(), live)
        };
        (self, Frame { commit, live })
    }
}

fn collapsed_window(buffer: &[String], width: usize) -> Vec<String> {
    let start = buffer.len().saturating_sub(COLLAPSED_LINES);
    (0..COLLAPSED_LINES)
        .map(|i| {
            buffer
                .get(start + i)
                .map(|line| line.chars().take(width).collect())
                .unwrap_or_default()
        })
        .collect()
}

struct RawTerminal {
    saved: String,
}

impl RawTerminal {
    fn enter() -> Result<Self> {
        let tty = OpenOptions::new().read(true).open("/dev/tty")?;
        let saved_state = Command::new("stty").arg("-g").stdin(tty).output()?;
        let saved = String::from_utf8_lossy(&saved_state.stdout)
            .trim()
            .to_string();
        let tty = OpenOptions::new().read(true).open("/dev/tty")?;
        Command::new("stty")
            .args(["raw", "-echo"])
            .stdin(tty)
            .status()?;
        Ok(Self { saved })
    }
}

impl Drop for RawTerminal {
    fn drop(&mut self) {
        if let Ok(tty) = OpenOptions::new().read(true).open("/dev/tty") {
            let _ = Command::new("stty").arg(&self.saved).stdin(tty).status();
        }
        print!("{SHOW_CURSOR}");
        let _ = stdout().flush();
    }
}

enum Key {
    CtrlC,
    CtrlO,
    Other,
}

fn next_key(tty: &mut impl Read) -> Key {
    let mut b = [0u8; 1];
    if tty.read(&mut b).unwrap_or(0) == 0 {
        return Key::Other;
    }
    match b[0] {
        3 => Key::CtrlC,
        15 => Key::CtrlO,
        27 => {
            if tty.read(&mut b).unwrap_or(0) > 0 && b[0] == b'[' {
                loop {
                    if tty.read(&mut b).unwrap_or(0) == 0 {
                        break;
                    }
                    if b[0].is_ascii_alphabetic() {
                        break;
                    }
                }
            }
            Key::Other
        }
        _ => Key::Other,
    }
}

fn terminal_width() -> usize {
    let tty = OpenOptions::new().read(true).open("/dev/tty").ok();
    tty.and_then(|tty| Command::new("stty").arg("size").stdin(tty).output().ok())
        .and_then(|o| {
            let s = String::from_utf8_lossy(&o.stdout);
            let mut parts = s.split_whitespace();
            parts.next();
            parts.next()?.parse().ok()
        })
        .unwrap_or(80)
}

fn write_frame(out: &mut impl Write, frame: &Frame, prev_live: usize) -> usize {
    if prev_live > 0 {
        let _ = write!(out, "\x1b[{prev_live}A");
    }
    for line in frame.commit.iter().chain(frame.live.iter()) {
        let _ = write!(out, "{CLEAR_LINE}\r{line}\r\n");
    }
    let written = frame.commit.len() + frame.live.len();
    if prev_live > written {
        let extra = prev_live - written;
        for _ in 0..extra {
            let _ = write!(out, "{CLEAR_LINE}\r\n");
        }
        let _ = write!(out, "\x1b[{extra}A");
    }
    let _ = out.flush();
    frame.live.len()
}

fn fallback_build_log(child: &mut std::process::Child) -> Result<std::process::ExitStatus> {
    let stdout_pipe = child.stdout.take();
    let stderr_pipe = child.stderr.take();

    let (log_tx, log_rx) = mpsc::channel::<String>();
    let tx1 = log_tx.clone();
    let tx2 = log_tx;

    let t1 = stdout_pipe.map(|pipe| {
        thread::spawn(move || {
            for line in BufReader::new(pipe).lines().map_while(Result::ok) {
                let _ = tx1.send(line);
            }
        })
    });

    let t2 = stderr_pipe.map(|pipe| {
        thread::spawn(move || {
            for line in BufReader::new(pipe).lines().map_while(Result::ok) {
                let _ = tx2.send(line);
            }
        })
    });

    let status = child
        .wait()
        .map_err(|e| anyhow::anyhow!("Failed to wait for build: {e}"))?;
    if let Some(t) = t1 {
        let _ = t.join();
    }
    if let Some(t) = t2 {
        let _ = t.join();
    }

    if !status.success() {
        for line in log_rx {
            eprintln!("{line}");
        }
    }

    Ok(status)
}

pub fn build_log(child: &mut std::process::Child) -> Result<std::process::ExitStatus> {
    let term_width = terminal_width();

    let raw_and_tty = RawTerminal::enter()
        .ok()
        .zip(OpenOptions::new().read(true).open("/dev/tty").ok());

    let (_raw, mut tty) = match raw_and_tty {
        Some(pair) => pair,
        None => return fallback_build_log(child),
    };

    let (log_tx, log_rx) = mpsc::channel::<String>();
    let (key_tx, key_rx) = mpsc::channel::<Key>();

    let stdout_pipe = match child.stdout.take() {
        Some(p) => p,
        None => return fallback_build_log(child),
    };
    let stderr_pipe = match child.stderr.take() {
        Some(p) => p,
        None => return fallback_build_log(child),
    };

    let tx1 = log_tx.clone();
    let tx2 = log_tx;

    thread::spawn(move || {
        for line in BufReader::new(stdout_pipe).lines().map_while(Result::ok) {
            if tx1.send(line).is_err() {
                break;
            }
        }
    });

    thread::spawn(move || {
        for line in BufReader::new(stderr_pipe).lines().map_while(Result::ok) {
            if tx2.send(line).is_err() {
                break;
            }
        }
    });

    thread::spawn(move || {
        loop {
            let key = next_key(&mut tty);
            if key_tx.send(key).is_err() {
                break;
            }
        }
    });

    let mut out = stdout();
    print!("{HIDE_CURSOR}");
    let (mut view, frame) = BuildLogView::new(term_width).advance();
    let mut prev_live = write_frame(&mut out, &frame, 0);

    let mut log_done = false;

    loop {
        let mut new_lines: Vec<String> = Vec::new();

        loop {
            match log_rx.try_recv() {
                Ok(line) => new_lines.push(line),
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    log_done = true;
                    break;
                }
            }
        }

        if !new_lines.is_empty() {
            let (next, frame) = view.on_lines(new_lines);
            view = next;
            prev_live = write_frame(&mut out, &frame, prev_live);
        }

        if log_done {
            break;
        }

        match key_rx.try_recv() {
            Ok(Key::CtrlO) => {
                let (next, frame) = view.toggle();
                view = next;
                prev_live = write_frame(&mut out, &frame, prev_live);
            }
            Ok(Key::CtrlC) => {
                let pid = child.id().to_string();
                let _ = Command::new("kill").args(["-INT", &pid]).status();
            }
            Ok(Key::Other) | Err(_) => {}
        }

        thread::sleep(Duration::from_millis(10));
    }

    let exit_status = child
        .wait()
        .map_err(|e| anyhow::anyhow!("Failed to wait for build: {e}"))?;

    write_frame(&mut out, &Frame::default(), prev_live);

    if !exit_status.success() {
        drop(_raw);
        for line in &view.buffer {
            eprintln!("{line}");
        }
    }

    Ok(exit_status)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn when_collapsed_window_with_lines_then_returns_last_n() {
        let buf: Vec<String> = (1..=5).map(|i| format!("line {i}")).collect();
        let w = collapsed_window(&buf, 80);
        assert_eq!(w, vec!["line 3", "line 4", "line 5"]);
    }

    #[test]
    fn when_collapsed_window_with_fewer_lines_then_returns_padded_empty() {
        let buf = vec!["a".to_string(), "b".to_string()];
        let w = collapsed_window(&buf, 80);
        assert_eq!(w, vec!["a", "b", ""]);
    }

    #[test]
    fn when_collapsed_window_with_long_line_then_returns_truncated() {
        let buf = vec!["x".repeat(200)];
        let w = collapsed_window(&buf, 80);
        assert_eq!(w[0], "x".repeat(80));
    }

    #[test]
    fn when_on_lines_with_collapsed_then_returns_window_without_commit() {
        let view = BuildLogView::new(80);
        let (_view, frame) = view.on_lines(vec![
            "a".to_string(),
            "b".to_string(),
            "c".to_string(),
            "d".to_string(),
        ]);
        assert!(frame.commit.is_empty());
        assert_eq!(
            frame.live,
            vec![
                "b".to_string(),
                "c".to_string(),
                "d".to_string(),
                EXPAND_HINT.to_string()
            ]
        );
    }

    #[test]
    fn when_toggle_with_buffered_lines_then_returns_committed_lines() {
        let view = BuildLogView::new(80);
        let (view, _) = view.on_lines(vec!["a".to_string(), "b".to_string()]);
        let (_view, frame) = view.toggle();
        assert_eq!(frame.commit, vec!["a".to_string(), "b".to_string()]);
        assert_eq!(frame.live, vec![COLLAPSE_HINT.to_string()]);
    }

    #[test]
    fn when_on_lines_with_expanded_then_returns_appended_lines() {
        let view = BuildLogView::new(80);
        let (view, _) = view.on_lines(vec!["a".to_string(), "b".to_string()]);
        let (view, _) = view.toggle();
        let (_view, frame) = view.on_lines(vec!["c".to_string()]);
        assert_eq!(frame.commit, vec!["c".to_string()]);
        assert_eq!(frame.live, vec![COLLAPSE_HINT.to_string()]);
    }

    #[test]
    fn when_toggle_with_expanded_then_returns_no_recommit() {
        let view = BuildLogView::new(80);
        let (view, _) = view.on_lines(vec!["a".to_string(), "b".to_string()]);
        let (view, _) = view.toggle();
        let (view, frame) = view.toggle();
        assert!(frame.commit.is_empty());
        let (_view, next) = view.on_lines(vec!["c".to_string()]);
        assert!(next.commit.is_empty());
    }

    #[test]
    fn when_toggle_with_reexpansion_then_returns_only_new_lines() {
        let view = BuildLogView::new(80);
        let (view, _) = view.on_lines(vec!["a".to_string(), "b".to_string()]);
        let (view, _) = view.toggle();
        let (view, _) = view.toggle();
        let (view, _) = view.on_lines(vec!["c".to_string()]);
        let (_view, frame) = view.toggle();
        assert_eq!(frame.commit, vec!["c".to_string()]);
    }
}
