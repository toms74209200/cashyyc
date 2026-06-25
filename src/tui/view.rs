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

#[derive(Clone, Copy)]
pub enum Key {
    Char(char),
    Up,
    Down,
    Enter,
    Backspace,
    Space,
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
        10 | 13 => Key::Enter,
        8 | 127 => Key::Backspace,
        32 => Key::Space,
        27 => {
            if tty.read(&mut b).unwrap_or(0) == 0 {
                return Key::Other;
            }
            if b[0] != b'[' {
                return Key::Other;
            }
            if tty.read(&mut b).unwrap_or(0) == 0 {
                return Key::Other;
            }
            match b[0] {
                b'A' => Key::Up,
                b'B' => Key::Down,
                _ => Key::Other,
            }
        }
        c if c.is_ascii_graphic() => Key::Char(c as char),
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

const VIEWPORT: usize = 10;
const SEPARATOR: &str = "──────────────────────────────────────";
const INVERT: &str = "\x1b[7m";
const RESET: &str = "\x1b[0m";
const REDRAW_UP: usize = VIEWPORT + 3;

fn filtered_indices(items: &[String], query: &str) -> Vec<usize> {
    if query.is_empty() {
        return (0..items.len()).collect();
    }
    let mut scored: Vec<(usize, i32)> = items
        .iter()
        .enumerate()
        .filter_map(|(i, name)| crate::search::score(name, query).map(|s| (i, s)))
        .collect();
    scored.sort_by_key(|b| std::cmp::Reverse(b.1));
    scored.into_iter().map(|(i, _)| i).collect()
}

#[allow(clippy::too_many_arguments)]
fn render_select_frame(
    label: &str,
    query: &str,
    cursor: usize,
    scroll: usize,
    items: &[String],
    fi: &[usize],
    selected: Option<&[bool]>,
    up: usize,
) -> String {
    let mut buf = String::new();
    if up > 0 {
        buf.push_str(&format!("\x1b[{up}A"));
    }
    buf.push_str(&format!("{CLEAR_LINE}\r{label}: {query}█\n"));
    buf.push_str(&format!("{CLEAR_LINE}\r{SEPARATOR}\n"));
    for i in 0..VIEWPORT {
        let fi_idx = scroll + i;
        buf.push_str(&format!("{CLEAR_LINE}\r"));
        if fi_idx < fi.len() {
            let orig = fi[fi_idx];
            let name = &items[orig];
            let at_cursor = fi_idx == cursor;
            match selected {
                Some(sel) => {
                    let check = if sel[orig] { "[x]" } else { "[ ]" };
                    if at_cursor {
                        buf.push_str(&format!("{INVERT}> {check} {name}{RESET}\n"));
                    } else {
                        buf.push_str(&format!("  {check} {name}\n"));
                    }
                }
                None => {
                    if at_cursor {
                        buf.push_str(&format!("{INVERT}> {name}{RESET}\n"));
                    } else {
                        buf.push_str(&format!("  {name}\n"));
                    }
                }
            }
        } else {
            buf.push('\n');
        }
    }
    buf.push_str(&format!("{CLEAR_LINE}\r{SEPARATOR}\n"));
    let hint = match selected.map(|s| s.iter().filter(|&&v| v).count()) {
        Some(n) => format!(
            "{}/{} selected:{n}  ↑↓ move  Space toggle  Enter confirm  Ctrl-C cancel",
            fi.len(),
            items.len()
        ),
        None => format!(
            "{}/{}  ↑↓ move  Enter select  Ctrl-C cancel",
            fi.len(),
            items.len()
        ),
    };
    buf.push_str(&format!("{CLEAR_LINE}\r{hint}"));
    buf
}

fn erase_select() -> String {
    let total = VIEWPORT + 4;
    let mut buf = format!("\x1b[{REDRAW_UP}A");
    for _ in 0..total {
        buf.push_str(&format!("{CLEAR_LINE}\r\n"));
    }
    buf.push_str(&format!("\x1b[{total}A"));
    buf
}

pub struct SelectView<'a> {
    label: &'a str,
    items: &'a [String],
    query: String,
    cursor: usize,
    scroll: usize,
}

pub enum SelectResult<'a> {
    Continue(SelectView<'a>, String),
    Done(Option<usize>, String),
}

impl<'a> SelectView<'a> {
    pub fn new(label: &'a str, items: &'a [String]) -> (Self, String) {
        let view = Self {
            label,
            items,
            query: String::new(),
            cursor: 0,
            scroll: 0,
        };
        let fi: Vec<usize> = (0..items.len()).collect();
        let frame = render_select_frame(label, "", 0, 0, items, &fi, None, 0);
        (view, frame)
    }

    pub fn on_key(mut self, key: Key) -> SelectResult<'a> {
        if matches!(key, Key::CtrlC) {
            return SelectResult::Done(None, erase_select());
        }
        match key {
            Key::Backspace => {
                self.query.pop();
                self.cursor = 0;
                self.scroll = 0;
            }
            Key::Char(c) => {
                self.query.push(c);
                self.cursor = 0;
                self.scroll = 0;
            }
            _ => {}
        }
        let fi = filtered_indices(self.items, &self.query);
        if matches!(key, Key::Enter) {
            return SelectResult::Done(fi.get(self.cursor).copied(), erase_select());
        }
        match key {
            Key::Up => self.cursor = self.cursor.saturating_sub(1),
            Key::Down if self.cursor + 1 < fi.len() => {
                self.cursor += 1;
            }
            _ => {}
        }
        if self.cursor < self.scroll {
            self.scroll = self.cursor;
        } else if self.cursor >= self.scroll + VIEWPORT {
            self.scroll = self.cursor + 1 - VIEWPORT;
        }
        let frame = render_select_frame(
            self.label,
            &self.query,
            self.cursor,
            self.scroll,
            self.items,
            &fi,
            None,
            REDRAW_UP,
        );
        SelectResult::Continue(self, frame)
    }
}

pub struct MultiSelectView<'a> {
    label: &'a str,
    items: &'a [String],
    query: String,
    cursor: usize,
    scroll: usize,
    selected: Vec<bool>,
}

pub enum MultiSelectResult<'a> {
    Continue(MultiSelectView<'a>, String),
    Done(Option<Vec<usize>>, String),
}

impl<'a> MultiSelectView<'a> {
    pub fn new(label: &'a str, items: &'a [String]) -> (Self, String) {
        let selected = vec![false; items.len()];
        let fi: Vec<usize> = (0..items.len()).collect();
        let frame = render_select_frame(label, "", 0, 0, items, &fi, Some(&selected), 0);
        let view = Self {
            label,
            items,
            query: String::new(),
            cursor: 0,
            scroll: 0,
            selected,
        };
        (view, frame)
    }

    pub fn on_key(mut self, key: Key) -> MultiSelectResult<'a> {
        if matches!(key, Key::CtrlC) {
            return MultiSelectResult::Done(None, erase_select());
        }
        match key {
            Key::Backspace => {
                self.query.pop();
                self.cursor = 0;
                self.scroll = 0;
            }
            Key::Char(c) => {
                self.query.push(c);
                self.cursor = 0;
                self.scroll = 0;
            }
            _ => {}
        }
        let fi = filtered_indices(self.items, &self.query);
        if matches!(key, Key::Enter) {
            let result = (0..self.items.len())
                .filter(|&i| self.selected[i])
                .collect();
            return MultiSelectResult::Done(Some(result), erase_select());
        }
        match key {
            Key::Space => {
                if let Some(&orig) = fi.get(self.cursor) {
                    self.selected[orig] = !self.selected[orig];
                }
            }
            Key::Up => self.cursor = self.cursor.saturating_sub(1),
            Key::Down if self.cursor + 1 < fi.len() => {
                self.cursor += 1;
            }
            _ => {}
        }
        if self.cursor < self.scroll {
            self.scroll = self.cursor;
        } else if self.cursor >= self.scroll + VIEWPORT {
            self.scroll = self.cursor + 1 - VIEWPORT;
        }
        let frame = render_select_frame(
            self.label,
            &self.query,
            self.cursor,
            self.scroll,
            self.items,
            &fi,
            Some(&self.selected),
            REDRAW_UP,
        );
        MultiSelectResult::Continue(self, frame)
    }
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
    fn when_next_key_with_regular_char_then_returns_char() {
        let mut input = Cursor::new(vec![b'a']);
        assert!(matches!(next_key(&mut input), Key::Char('a')));
    }

    #[test]
    fn when_next_key_with_empty_then_returns_other() {
        let mut input = Cursor::new(vec![]);
        assert!(matches!(next_key(&mut input), Key::Other));
    }

    #[test]
    fn when_next_key_with_escape_sequence_up_then_returns_up() {
        let mut input = Cursor::new(vec![27, b'[', b'A']);
        assert!(matches!(next_key(&mut input), Key::Up));
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

    fn items(names: &[&str]) -> Vec<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn when_filtered_indices_with_empty_query_then_returns_all_in_order() {
        let names = items(&["go", "rust", "python"]);
        assert_eq!(filtered_indices(&names, ""), vec![0, 1, 2]);
    }

    #[test]
    fn when_filtered_indices_excludes_non_matching() {
        let names = items(&["go", "rust", "python"]);
        assert_eq!(filtered_indices(&names, "go"), vec![0]);
    }

    #[test]
    fn when_filtered_indices_ranks_better_match_first() {
        let names = items(&["axc", "acb"]);
        assert_eq!(filtered_indices(&names, "ac"), vec![1, 0]);
    }

    #[test]
    fn when_on_lines_with_osc_st_terminator_then_strips() {
        let view = BuildLogView::new(80);
        let (_view, frame) = view.on_lines(vec!["\x1b]0;title\u{9C}text".to_string()]);
        assert!(frame.live.iter().any(|l| l.contains("text")));
        assert!(!frame.live.iter().any(|l| l.contains("title")));
    }

    #[test]
    fn when_next_key_with_enter_then_returns_enter() {
        let mut input = Cursor::new(vec![13u8]);
        assert!(matches!(next_key(&mut input), Key::Enter));
    }

    #[test]
    fn when_next_key_with_newline_then_returns_enter() {
        let mut input = Cursor::new(vec![10u8]);
        assert!(matches!(next_key(&mut input), Key::Enter));
    }

    #[test]
    fn when_next_key_with_backspace_then_returns_backspace() {
        let mut input = Cursor::new(vec![127u8]);
        assert!(matches!(next_key(&mut input), Key::Backspace));
    }

    #[test]
    fn when_next_key_with_bs_then_returns_backspace() {
        let mut input = Cursor::new(vec![8u8]);
        assert!(matches!(next_key(&mut input), Key::Backspace));
    }

    #[test]
    fn when_next_key_with_space_then_returns_space() {
        let mut input = Cursor::new(vec![32u8]);
        assert!(matches!(next_key(&mut input), Key::Space));
    }

    #[test]
    fn when_next_key_with_down_arrow_then_returns_down() {
        let mut input = Cursor::new(vec![27, b'[', b'B']);
        assert!(matches!(next_key(&mut input), Key::Down));
    }

    #[test]
    fn when_next_key_with_unknown_escape_then_returns_other() {
        let mut input = Cursor::new(vec![27, b'[', b'C']);
        assert!(matches!(next_key(&mut input), Key::Other));
    }

    #[test]
    fn when_next_key_with_escape_no_bracket_then_returns_other() {
        let mut input = Cursor::new(vec![27, b'O']);
        assert!(matches!(next_key(&mut input), Key::Other));
    }

    #[test]
    fn when_next_key_with_escape_bracket_eof_then_returns_other() {
        let mut input = Cursor::new(vec![27, b'[']);
        assert!(matches!(next_key(&mut input), Key::Other));
    }

    #[test]
    fn when_next_key_with_control_char_then_returns_other() {
        let mut input = Cursor::new(vec![1u8]);
        assert!(matches!(next_key(&mut input), Key::Other));
    }

    #[test]
    fn when_select_new_then_frame_contains_label_and_highlights_first() {
        let it = items(&["go", "rust"]);
        let (_, frame) = SelectView::new("Template", &it);
        assert!(frame.contains("Template: █"));
        assert!(frame.contains(&format!("{INVERT}> go{RESET}")));
    }

    #[test]
    fn when_select_ctrl_c_then_done_none() {
        let it = items(&["go", "rust"]);
        let (view, _) = SelectView::new("Template", &it);
        let SelectResult::Done(result, _) = view.on_key(Key::CtrlC) else {
            panic!()
        };
        assert!(result.is_none());
    }

    #[test]
    fn when_select_enter_then_done_with_first_index() {
        let it = items(&["go", "rust"]);
        let (view, _) = SelectView::new("Template", &it);
        let SelectResult::Done(result, _) = view.on_key(Key::Enter) else {
            panic!()
        };
        assert_eq!(result, Some(0));
    }

    #[test]
    fn when_select_down_enter_then_selects_second() {
        let it = items(&["go", "rust"]);
        let (view, _) = SelectView::new("Template", &it);
        let SelectResult::Continue(view, _) = view.on_key(Key::Down) else {
            panic!()
        };
        let SelectResult::Done(result, _) = view.on_key(Key::Enter) else {
            panic!()
        };
        assert_eq!(result, Some(1));
    }

    #[test]
    fn when_select_up_at_top_then_stays() {
        let it = items(&["go", "rust"]);
        let (view, _) = SelectView::new("Template", &it);
        let SelectResult::Continue(view, _) = view.on_key(Key::Up) else {
            panic!()
        };
        let SelectResult::Done(result, _) = view.on_key(Key::Enter) else {
            panic!()
        };
        assert_eq!(result, Some(0));
    }

    #[test]
    fn when_select_down_at_bottom_then_stays() {
        let it = items(&["go", "rust"]);
        let (view, _) = SelectView::new("Template", &it);
        let SelectResult::Continue(view, _) = view.on_key(Key::Down) else {
            panic!()
        };
        let SelectResult::Continue(view, _) = view.on_key(Key::Down) else {
            panic!()
        };
        let SelectResult::Done(result, _) = view.on_key(Key::Enter) else {
            panic!()
        };
        assert_eq!(result, Some(1));
    }

    #[test]
    fn when_select_char_then_filters_and_enter_selects_original_index() {
        let it = items(&["go", "rust", "go-postgres"]);
        let (view, _) = SelectView::new("Template", &it);
        let SelectResult::Continue(view, frame) = view.on_key(Key::Char('r')) else {
            panic!()
        };
        assert!(frame.contains("rust"));
        let SelectResult::Done(result, _) = view.on_key(Key::Enter) else {
            panic!()
        };
        assert_eq!(result, Some(1));
    }

    #[test]
    fn when_select_backspace_then_unfilters() {
        let it = items(&["go", "rust"]);
        let (view, _) = SelectView::new("Template", &it);
        let SelectResult::Continue(view, _) = view.on_key(Key::Char('z')) else {
            panic!()
        };
        let SelectResult::Continue(_, frame) = view.on_key(Key::Backspace) else {
            panic!()
        };
        assert!(frame.contains("go"));
        assert!(frame.contains("rust"));
    }

    #[test]
    fn when_select_case_insensitive_filter() {
        let it = items(&["Go", "RUST"]);
        let (view, _) = SelectView::new("Template", &it);
        let SelectResult::Continue(_, frame) = view.on_key(Key::Char('g')) else {
            panic!()
        };
        assert!(frame.contains("Go"));
    }

    #[test]
    fn when_select_enter_on_empty_filtered_then_done_none() {
        let it = items(&["go", "rust"]);
        let (view, _) = SelectView::new("Template", &it);
        let SelectResult::Continue(view, _) = view.on_key(Key::Char('z')) else {
            panic!()
        };
        let SelectResult::Continue(view, _) = view.on_key(Key::Char('z')) else {
            panic!()
        };
        let SelectResult::Done(result, _) = view.on_key(Key::Enter) else {
            panic!()
        };
        assert!(result.is_none());
    }

    #[test]
    fn when_select_scroll_past_viewport_then_frame_shows_scrolled_item() {
        let it: Vec<String> = (0..20).map(|i| format!("item-{i}")).collect();
        let (mut view, _) = SelectView::new("Template", &it);
        for _ in 0..12 {
            let SelectResult::Continue(v, _) = view.on_key(Key::Down) else {
                panic!()
            };
            view = v;
        }
        let SelectResult::Continue(_, frame) = view.on_key(Key::Down) else {
            panic!()
        };
        assert!(frame.contains("item-13"));
    }

    #[test]
    fn when_select_done_then_returns_erase_string() {
        let it = items(&["go"]);
        let (view, _) = SelectView::new("Template", &it);
        let SelectResult::Done(_, erase) = view.on_key(Key::CtrlC) else {
            panic!()
        };
        assert!(!erase.is_empty());
    }

    #[test]
    fn when_multi_select_ctrl_c_then_done_none() {
        let it = items(&["git", "go"]);
        let (view, _) = MultiSelectView::new("Features", &it);
        let MultiSelectResult::Done(result, _) = view.on_key(Key::CtrlC) else {
            panic!()
        };
        assert!(result.is_none());
    }

    #[test]
    fn when_multi_select_enter_with_no_selection_then_done_empty() {
        let it = items(&["git", "go"]);
        let (view, _) = MultiSelectView::new("Features", &it);
        let MultiSelectResult::Done(result, _) = view.on_key(Key::Enter) else {
            panic!()
        };
        assert_eq!(result, Some(vec![]));
    }

    #[test]
    fn when_multi_select_space_enter_then_returns_toggled() {
        let it = items(&["git", "go"]);
        let (view, _) = MultiSelectView::new("Features", &it);
        let MultiSelectResult::Continue(view, _) = view.on_key(Key::Space) else {
            panic!()
        };
        let MultiSelectResult::Done(result, _) = view.on_key(Key::Enter) else {
            panic!()
        };
        assert_eq!(result, Some(vec![0]));
    }

    #[test]
    fn when_multi_select_space_toggles_off() {
        let it = items(&["git", "go"]);
        let (view, _) = MultiSelectView::new("Features", &it);
        let MultiSelectResult::Continue(view, _) = view.on_key(Key::Space) else {
            panic!()
        };
        let MultiSelectResult::Continue(view, _) = view.on_key(Key::Space) else {
            panic!()
        };
        let MultiSelectResult::Done(result, _) = view.on_key(Key::Enter) else {
            panic!()
        };
        assert_eq!(result, Some(vec![]));
    }

    #[test]
    fn when_multi_select_frame_shows_checkboxes() {
        let it = items(&["git", "go"]);
        let (view, frame) = MultiSelectView::new("Features", &it);
        assert!(frame.contains("[ ] git"));
        let MultiSelectResult::Continue(_, frame) = view.on_key(Key::Space) else {
            panic!()
        };
        assert!(frame.contains("[x] git"));
    }

    #[test]
    fn when_multi_select_down_space_then_toggles_second() {
        let it = items(&["git", "go"]);
        let (view, _) = MultiSelectView::new("Features", &it);
        let MultiSelectResult::Continue(view, _) = view.on_key(Key::Down) else {
            panic!()
        };
        let MultiSelectResult::Continue(view, _) = view.on_key(Key::Space) else {
            panic!()
        };
        let MultiSelectResult::Done(result, _) = view.on_key(Key::Enter) else {
            panic!()
        };
        assert_eq!(result, Some(vec![1]));
    }

    #[test]
    fn when_multi_select_char_filters() {
        let it = items(&["git", "go", "rust"]);
        let (view, _) = MultiSelectView::new("Features", &it);
        let MultiSelectResult::Continue(_, frame) = view.on_key(Key::Char('g')) else {
            panic!()
        };
        assert!(frame.contains("git"));
        assert!(frame.contains("go"));
    }

    #[test]
    fn when_multi_select_up_down_space_navigation() {
        let it = items(&["git", "go", "rust"]);
        let (view, _) = MultiSelectView::new("Features", &it);
        let MultiSelectResult::Continue(view, _) = view.on_key(Key::Down) else {
            panic!()
        };
        let MultiSelectResult::Continue(view, _) = view.on_key(Key::Down) else {
            panic!()
        };
        let MultiSelectResult::Continue(view, _) = view.on_key(Key::Up) else {
            panic!()
        };
        let MultiSelectResult::Continue(view, _) = view.on_key(Key::Space) else {
            panic!()
        };
        let MultiSelectResult::Done(result, _) = view.on_key(Key::Enter) else {
            panic!()
        };
        assert_eq!(result, Some(vec![1]));
    }

    #[test]
    fn when_multi_select_backspace_then_unfilters() {
        let it = items(&["git", "rust"]);
        let (view, _) = MultiSelectView::new("Features", &it);
        let MultiSelectResult::Continue(view, _) = view.on_key(Key::Char('z')) else {
            panic!()
        };
        let MultiSelectResult::Continue(_, frame) = view.on_key(Key::Backspace) else {
            panic!()
        };
        assert!(frame.contains("git"));
        assert!(frame.contains("rust"));
    }
}
