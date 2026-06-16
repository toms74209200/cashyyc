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

struct RawTerminal {
    saved: String,
}

impl RawTerminal {
    fn enter() -> Result<Self> {
        let tty = OpenOptions::new().read(true).open("/dev/tty")?;
        let out = Command::new("stty").arg("-g").stdin(tty).output()?;
        let saved = String::from_utf8_lossy(&out.stdout).trim().to_string();
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
            let _ = tty.read(&mut b);
            let _ = tty.read(&mut b);
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
            let mut parts = s.trim().split_whitespace();
            parts.next();
            parts.next()?.parse().ok()
        })
        .unwrap_or(80)
}

fn build_status_line(expanded: bool) -> String {
    if expanded {
        "Building...  Ctrl-O collapse\r\n".to_string()
    } else {
        "Building...  Ctrl-O expand\r\n".to_string()
    }
}

fn render_collapsed(log_buffer: &[String], term_width: usize) -> String {
    let mut buf = String::new();
    let start = log_buffer.len().saturating_sub(COLLAPSED_LINES);
    for i in 0..COLLAPSED_LINES {
        buf.push_str(&format!("{CLEAR_LINE}\r"));
        if let Some(line) = log_buffer.get(start + i) {
            let truncated: String = line.chars().take(term_width).collect();
            buf.push_str(truncated.as_str());
        }
        buf.push_str("\r\n");
    }
    buf.push_str(&build_status_line(false));
    buf
}

fn erase_lines(n: usize) {
    if n == 0 {
        return;
    }
    print!("\x1b[{n}A");
    for _ in 0..n {
        print!("{CLEAR_LINE}\r\n");
    }
    print!("\x1b[{n}A");
}

fn fallback_build_log(child: &mut std::process::Child) -> Result<std::process::ExitStatus> {
    let stdout_pipe = child.stdout.take();
    let stderr_pipe = child.stderr.take();

    let (log_tx, log_rx) = mpsc::channel::<String>();
    let tx1 = log_tx.clone();
    let tx2 = log_tx;

    let t1 = stdout_pipe.map(|pipe| {
        thread::spawn(move || {
            for line in BufReader::new(pipe).lines().flatten() {
                let _ = tx1.send(line);
            }
        })
    });

    let t2 = stderr_pipe.map(|pipe| {
        thread::spawn(move || {
            for line in BufReader::new(pipe).lines().flatten() {
                let _ = tx2.send(line);
            }
        })
    });

    let status = child.wait().map_err(|e| anyhow::anyhow!("Failed to wait for build: {e}"))?;
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

    let raw_and_tty = RawTerminal::enter().ok().and_then(|raw| {
        OpenOptions::new()
            .read(true)
            .open("/dev/tty")
            .ok()
            .map(|tty| (raw, tty))
    });

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
        for line in BufReader::new(stdout_pipe).lines() {
            if let Ok(line) = line {
                if tx1.send(line).is_err() {
                    break;
                }
            }
        }
    });

    thread::spawn(move || {
        for line in BufReader::new(stderr_pipe).lines() {
            if let Ok(line) = line {
                if tx2.send(line).is_err() {
                    break;
                }
            }
        }
    });

    thread::spawn(move || loop {
        let key = next_key(&mut tty);
        if key_tx.send(key).is_err() {
            break;
        }
    });

    let mut expanded = false;
    let mut log_buffer: Vec<String> = Vec::new();

    print!("{HIDE_CURSOR}");
    print!("{}", render_collapsed(&log_buffer, term_width));
    let _ = stdout().flush();
    let mut lines_drawn = COLLAPSED_LINES + 1;

    let mut log_done = false;

    loop {
        let mut new_lines: Vec<String> = Vec::new();

        loop {
            match log_rx.try_recv() {
                Ok(line) => {
                    log_buffer.push(line.clone());
                    new_lines.push(line);
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    log_done = true;
                    break;
                }
            }
        }

        if !new_lines.is_empty() {
            if expanded {
                print!("\x1b[1A{CLEAR_LINE}\r");
                for line in &new_lines {
                    print!("{line}\r\n");
                }
                print!("{}", build_status_line(true));
            } else {
                print!("\x1b[{lines_drawn}A");
                print!("{}", render_collapsed(&log_buffer, term_width));
            }
            let _ = stdout().flush();
        }

        if log_done {
            break;
        }

        loop {
            match key_rx.try_recv() {
                Ok(Key::CtrlO) => {
                    expanded = !expanded;
                    erase_lines(lines_drawn);
                    if expanded {
                        for line in &log_buffer {
                            print!("{line}\r\n");
                        }
                        print!("{}", build_status_line(true));
                        lines_drawn = 1;
                    } else {
                        print!("{}", render_collapsed(&log_buffer, term_width));
                        lines_drawn = COLLAPSED_LINES + 1;
                    }
                    let _ = stdout().flush();
                    break;
                }
                Ok(Key::CtrlC) => {
                    let pid = child.id().to_string();
                    let _ = Command::new("kill").args(["-INT", &pid]).status();
                    break;
                }
                Ok(Key::Other) | Err(mpsc::TryRecvError::Empty) | Err(mpsc::TryRecvError::Disconnected) => {
                    break;
                }
            }
        }

        thread::sleep(Duration::from_millis(10));
    }

    let exit_status = child
        .wait()
        .map_err(|e| anyhow::anyhow!("Failed to wait for build: {e}"))?;

    erase_lines(lines_drawn);
    let _ = stdout().flush();

    if !exit_status.success() {
        drop(_raw);
        for line in &log_buffer {
            eprintln!("{line}");
        }
    }

    Ok(exit_status)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn when_build_status_line_collapsed_then_shows_expand_hint() {
        let s = build_status_line(false);
        assert!(s.contains("Building..."));
        assert!(s.contains("Ctrl-O expand"));
    }

    #[test]
    fn when_build_status_line_expanded_then_shows_collapse_hint() {
        let s = build_status_line(true);
        assert!(s.contains("Building..."));
        assert!(s.contains("Ctrl-O collapse"));
    }

    #[test]
    fn when_render_collapsed_with_empty_buffer_then_shows_status_line() {
        let s = render_collapsed(&[], 80);
        assert!(s.contains("Building..."));
        assert!(s.contains("Ctrl-O expand"));
    }

    #[test]
    fn when_render_collapsed_with_lines_then_shows_last_n() {
        let buf: Vec<String> = (1..=5).map(|i| format!("line {i}")).collect();
        let s = render_collapsed(&buf, 80);
        assert!(!s.contains("line 1"));
        assert!(!s.contains("line 2"));
        assert!(s.contains("line 3"));
        assert!(s.contains("line 4"));
        assert!(s.contains("line 5"));
    }

    #[test]
    fn when_render_collapsed_with_long_line_then_truncates_to_term_width() {
        let buf = vec!["x".repeat(200)];
        let s = render_collapsed(&buf, 80);
        assert!(s.contains(&"x".repeat(80)));
        assert!(!s.contains(&"x".repeat(81)));
    }
}
