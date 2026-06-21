use std::io::{Read, Write};

pub const HIDE_CURSOR: &str = "\x1b[?25l";
pub const SHOW_CURSOR: &str = "\x1b[?25h";
const CLEAR_LINE: &str = "\x1b[2K";
const COLLAPSED_LINES: usize = 3;
const EXPAND_HINT: &str = "Building...  Ctrl-O expand";
const COLLAPSE_HINT: &str = "Building...  Ctrl-O collapse";

#[derive(Debug, Default, PartialEq)]
pub struct Frame {
    pub commit: Vec<String>,
    pub live: Vec<String>,
}

pub struct BuildLogView {
    pub buffer: Vec<String>,
    expanded: bool,
    emitted: usize,
    width: usize,
}

impl BuildLogView {
    pub fn new(width: usize) -> Self {
        Self {
            buffer: Vec::new(),
            expanded: false,
            emitted: 0,
            width,
        }
    }

    pub fn on_lines(mut self, lines: Vec<String>) -> (Self, Frame) {
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

    pub fn toggle(mut self) -> (Self, Frame) {
        self.expanded = !self.expanded;
        self.advance()
    }

    pub fn advance(mut self) -> (Self, Frame) {
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

pub enum Key {
    CtrlC,
    CtrlO,
    Other,
}

pub fn next_key(tty: &mut impl Read) -> Key {
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

pub fn write_frame(out: &mut impl Write, frame: &Frame, prev_live: usize) -> usize {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

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

    #[test]
    fn when_on_lines_with_ansi_csi_then_strips_escape_sequences() {
        let view = BuildLogView::new(80);
        let (_view, frame) = view.on_lines(vec!["\x1b[31mred\x1b[0m normal".to_string()]);
        assert!(frame.live.iter().any(|l| l.contains("red normal")));
    }

    #[test]
    fn when_on_lines_with_ansi_osc_then_strips_osc_sequences() {
        let view = BuildLogView::new(80);
        let (_view, frame) = view.on_lines(vec!["\x1b]0;title\x07text".to_string()]);
        assert!(frame.live.iter().any(|l| l.contains("text")));
        assert!(!frame.live.iter().any(|l| l.contains("title")));
    }

    #[test]
    fn when_on_lines_with_no_escapes_then_preserves_text() {
        let view = BuildLogView::new(80);
        let (_view, frame) = view.on_lines(vec!["plain text".to_string()]);
        assert!(frame.live.iter().any(|l| l == "plain text"));
    }

    #[test]
    fn when_collapsed_window_with_empty_buffer_then_returns_empty_strings() {
        let buf: Vec<String> = vec![];
        let w = collapsed_window(&buf, 80);
        assert_eq!(w, vec!["", "", ""]);
    }

    #[test]
    fn when_new_view_advance_then_returns_empty_window_with_hint() {
        let view = BuildLogView::new(80);
        let (_view, frame) = view.on_lines(vec![]);
        assert!(frame.commit.is_empty());
        assert_eq!(frame.live.len(), COLLAPSED_LINES + 1);
        assert_eq!(frame.live.last().unwrap(), EXPAND_HINT);
    }

    #[test]
    fn when_next_key_with_ctrl_c_then_returns_ctrl_c() {
        let mut input = Cursor::new(vec![3u8]);
        assert!(matches!(next_key(&mut input), Key::CtrlC));
    }

    #[test]
    fn when_next_key_with_ctrl_o_then_returns_ctrl_o() {
        let mut input = Cursor::new(vec![15u8]);
        assert!(matches!(next_key(&mut input), Key::CtrlO));
    }

    #[test]
    fn when_next_key_with_regular_char_then_returns_other() {
        let mut input = Cursor::new(vec![b'a']);
        assert!(matches!(next_key(&mut input), Key::Other));
    }

    #[test]
    fn when_next_key_with_empty_then_returns_other() {
        let mut input = Cursor::new(vec![]);
        assert!(matches!(next_key(&mut input), Key::Other));
    }

    #[test]
    fn when_next_key_with_escape_sequence_then_consumes_and_returns_other() {
        let mut input = Cursor::new(vec![27, b'[', b'A']);
        assert!(matches!(next_key(&mut input), Key::Other));
        assert_eq!(input.position(), 3);
    }

    #[test]
    fn when_next_key_with_escape_only_then_returns_other() {
        let mut input = Cursor::new(vec![27]);
        assert!(matches!(next_key(&mut input), Key::Other));
    }

    #[test]
    fn when_write_frame_with_commit_and_live_then_writes_all_lines() {
        let frame = Frame {
            commit: vec!["committed".to_string()],
            live: vec!["live".to_string()],
        };
        let mut buf = Vec::new();
        let live_count = write_frame(&mut buf, &frame, 0);
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("committed"));
        assert!(output.contains("live"));
        assert_eq!(live_count, 1);
    }

    #[test]
    fn when_write_frame_with_prev_live_then_moves_cursor_up() {
        let frame = Frame {
            commit: vec![],
            live: vec!["line".to_string()],
        };
        let mut buf = Vec::new();
        write_frame(&mut buf, &frame, 3);
        let output = String::from_utf8(buf).unwrap();
        assert!(output.starts_with("\x1b[3A"));
    }

    #[test]
    fn when_write_frame_with_fewer_lines_than_prev_then_clears_extra() {
        let frame = Frame {
            commit: vec![],
            live: vec!["one".to_string()],
        };
        let mut buf = Vec::new();
        write_frame(&mut buf, &frame, 4);
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("\x1b[3A"));
    }

    #[test]
    fn when_write_frame_with_empty_frame_then_returns_zero() {
        let frame = Frame::default();
        let mut buf = Vec::new();
        let live_count = write_frame(&mut buf, &frame, 0);
        assert_eq!(live_count, 0);
    }

    #[test]
    fn when_write_frame_then_returns_live_line_count() {
        let frame = Frame {
            commit: vec!["a".to_string(), "b".to_string()],
            live: vec!["c".to_string(), "d".to_string(), "e".to_string()],
        };
        let mut buf = Vec::new();
        let live_count = write_frame(&mut buf, &frame, 0);
        assert_eq!(live_count, 3);
    }
}
