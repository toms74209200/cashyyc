use anyhow::Result;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::process::Command;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use super::view::{write_frame, BuildLogView, Frame, Key, HIDE_CURSOR, SHOW_CURSOR};

const O_NONBLOCK: i32 = 0o4000;

fn open_output() -> Result<File> {
    Ok(OpenOptions::new().write(true).open("/dev/tty")?)
}

fn open_input() -> Result<File> {
    Ok(OpenOptions::new()
        .read(true)
        .custom_flags(O_NONBLOCK)
        .open("/dev/tty")?)
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
        if let Ok(mut out) = open_output() {
            let _ = write!(out, "{SHOW_CURSOR}");
            let _ = out.flush();
        }
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

    let raw_and_tty = RawTerminal::enter().ok().zip(open_input().ok());

    let (raw, mut tty) = match raw_and_tty {
        Some(pair) => pair,
        None => return fallback_build_log(child),
    };

    let mut out = match open_output() {
        Ok(out) => out,
        Err(_) => return fallback_build_log(child),
    };

    let (log_tx, log_rx) = mpsc::channel::<String>();

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

    let _ = write!(out, "{HIDE_CURSOR}");
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

        match super::view::next_key(&mut tty) {
            Key::CtrlO => {
                let (next, frame) = view.toggle();
                view = next;
                prev_live = write_frame(&mut out, &frame, prev_live);
            }
            Key::CtrlC => {
                let pid = child.id().to_string();
                let _ = Command::new("kill").args(["-INT", &pid]).status();
            }
            Key::Other => {}
        }

        thread::sleep(Duration::from_millis(10));
    }

    let exit_status = child
        .wait()
        .map_err(|e| anyhow::anyhow!("Failed to wait for build: {e}"))?;

    write_frame(&mut out, &Frame::default(), prev_live);

    if !exit_status.success() {
        drop(raw);
        for line in &view.buffer {
            eprintln!("{line}");
        }
    }

    Ok(exit_status)
}
