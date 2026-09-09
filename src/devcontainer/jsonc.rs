use std::collections::HashMap;

const MAX_DEPTH: usize = 128;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<Value>),
    Object(Vec<(String, Value)>),
}

impl Value {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_u64(&self) -> Option<u64> {
        match self {
            Value::Number(n)
                if n.fract() == 0.0 && *n >= 0.0 && *n < 18_446_744_073_709_551_616.0 =>
            {
                Some(*n as u64)
            }
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[Value]> {
        match self {
            Value::Array(items) => Some(items),
            _ => None,
        }
    }

    pub fn as_object(&self) -> Option<&[(String, Value)]> {
        match self {
            Value::Object(members) => Some(members),
            _ => None,
        }
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Object(members) => members.iter().rev().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn to_string_map(&self) -> Option<HashMap<String, String>> {
        let members = self.as_object()?;
        let mut map = HashMap::new();
        for (k, v) in members {
            map.insert(k.clone(), v.as_str()?.to_string());
        }
        Some(map)
    }

    pub fn to_json_pretty(&self) -> String {
        let mut out = String::new();
        write_value(self, 0, &mut out);
        out
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut out = String::new();
        write_compact(self, &mut out);
        write!(f, "{}", out)
    }
}

fn write_compact(value: &Value, out: &mut String) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(true) => out.push_str("true"),
        Value::Bool(false) => out.push_str("false"),
        Value::Number(n) => write_number(*n, out),
        Value::String(s) => write_string(s, out),
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_compact(item, out);
            }
            out.push(']');
        }
        Value::Object(members) => {
            out.push('{');
            for (i, (key, member)) in members.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_string(key, out);
                out.push(':');
                write_compact(member, out);
            }
            out.push('}');
        }
    }
}

fn write_value(value: &Value, indent: usize, out: &mut String) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(true) => out.push_str("true"),
        Value::Bool(false) => out.push_str("false"),
        Value::Number(n) => write_number(*n, out),
        Value::String(s) => write_string(s, out),
        Value::Array(items) => {
            if items.is_empty() {
                out.push_str("[]");
                return;
            }
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push('\n');
                push_indent(indent + 1, out);
                write_value(item, indent + 1, out);
            }
            out.push('\n');
            push_indent(indent, out);
            out.push(']');
        }
        Value::Object(members) => {
            if members.is_empty() {
                out.push_str("{}");
                return;
            }
            out.push('{');
            for (i, (key, member)) in members.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push('\n');
                push_indent(indent + 1, out);
                write_string(key, out);
                out.push_str(": ");
                write_value(member, indent + 1, out);
            }
            out.push('\n');
            push_indent(indent, out);
            out.push('}');
        }
    }
}

fn push_indent(indent: usize, out: &mut String) {
    for _ in 0..indent {
        out.push_str("  ");
    }
}

fn write_number(n: f64, out: &mut String) {
    if n.fract() == 0.0 && n.abs() < 9_007_199_254_740_992.0 {
        out.push_str(&format!("{}", n as i64));
    } else {
        out.push_str(&format!("{}", n));
    }
}

fn write_string(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{0008}' => out.push_str("\\b"),
            '\u{000C}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

#[derive(Debug, Clone, PartialEq)]
pub enum JsoncError {
    UnexpectedEof,
    UnexpectedChar(usize),
    InvalidNumber(usize),
    InvalidEscape(usize),
    UnclosedString(usize),
    UnclosedComment(usize),
    TrailingCharacters(usize),
    DepthLimitExceeded,
}

impl std::fmt::Display for JsoncError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JsoncError::UnexpectedEof => write!(f, "unexpected end of input"),
            JsoncError::UnexpectedChar(pos) => write!(f, "unexpected character at byte {pos}"),
            JsoncError::InvalidNumber(pos) => write!(f, "invalid number at byte {pos}"),
            JsoncError::InvalidEscape(pos) => write!(f, "invalid escape sequence at byte {pos}"),
            JsoncError::UnclosedString(pos) => write!(f, "unclosed string starting at byte {pos}"),
            JsoncError::UnclosedComment(pos) => {
                write!(f, "unclosed comment starting at byte {pos}")
            }
            JsoncError::TrailingCharacters(pos) => {
                write!(f, "unexpected trailing characters at byte {pos}")
            }
            JsoncError::DepthLimitExceeded => write!(f, "nesting depth limit exceeded"),
        }
    }
}

impl std::error::Error for JsoncError {}

pub fn parse(input: &str) -> Result<Value, JsoncError> {
    let bytes = input.as_bytes();
    let mut pos = 0;
    skip_whitespace_and_comments(bytes, &mut pos)?;
    let value = parse_value(bytes, &mut pos, 0)?;
    skip_whitespace_and_comments(bytes, &mut pos)?;
    if pos < bytes.len() {
        return Err(JsoncError::TrailingCharacters(pos));
    }
    Ok(value)
}

fn skip_whitespace_and_comments(bytes: &[u8], pos: &mut usize) -> Result<(), JsoncError> {
    loop {
        while let Some(b) = bytes.get(*pos) {
            if matches!(b, b' ' | b'\t' | b'\n' | b'\r') {
                *pos += 1;
            } else {
                break;
            }
        }
        match (bytes.get(*pos), bytes.get(*pos + 1)) {
            (Some(b'/'), Some(b'/')) => {
                while let Some(b) = bytes.get(*pos) {
                    if *b == b'\n' {
                        break;
                    }
                    *pos += 1;
                }
            }
            (Some(b'/'), Some(b'*')) => {
                let start = *pos;
                *pos += 2;
                loop {
                    match (bytes.get(*pos), bytes.get(*pos + 1)) {
                        (Some(b'*'), Some(b'/')) => {
                            *pos += 2;
                            break;
                        }
                        (Some(_), _) => *pos += 1,
                        (None, _) => return Err(JsoncError::UnclosedComment(start)),
                    }
                }
            }
            _ => return Ok(()),
        }
    }
}

fn parse_value(bytes: &[u8], pos: &mut usize, depth: usize) -> Result<Value, JsoncError> {
    if depth > MAX_DEPTH {
        return Err(JsoncError::DepthLimitExceeded);
    }
    match bytes.get(*pos) {
        None => Err(JsoncError::UnexpectedEof),
        Some(b'{') => parse_object(bytes, pos, depth),
        Some(b'[') => parse_array(bytes, pos, depth),
        Some(b'"') => Ok(Value::String(parse_string(bytes, pos)?)),
        Some(b't') => parse_keyword(bytes, pos, b"true", Value::Bool(true)),
        Some(b'f') => parse_keyword(bytes, pos, b"false", Value::Bool(false)),
        Some(b'n') => parse_keyword(bytes, pos, b"null", Value::Null),
        Some(b'-' | b'0'..=b'9') => parse_number(bytes, pos),
        Some(_) => Err(JsoncError::UnexpectedChar(*pos)),
    }
}

fn parse_keyword(
    bytes: &[u8],
    pos: &mut usize,
    keyword: &[u8],
    value: Value,
) -> Result<Value, JsoncError> {
    if bytes.get(*pos..*pos + keyword.len()) == Some(keyword) {
        *pos += keyword.len();
        Ok(value)
    } else {
        Err(JsoncError::UnexpectedChar(*pos))
    }
}

fn parse_number(bytes: &[u8], pos: &mut usize) -> Result<Value, JsoncError> {
    let start = *pos;
    if bytes.get(*pos) == Some(&b'-') {
        *pos += 1;
    }
    while let Some(b) = bytes.get(*pos) {
        if matches!(b, b'0'..=b'9' | b'.' | b'e' | b'E' | b'+' | b'-') {
            *pos += 1;
        } else {
            break;
        }
    }
    std::str::from_utf8(bytes.get(start..*pos).unwrap_or_default())
        .ok()
        .and_then(|s| s.parse::<f64>().ok())
        .filter(|n| n.is_finite())
        .map(Value::Number)
        .ok_or(JsoncError::InvalidNumber(start))
}

fn parse_string(bytes: &[u8], pos: &mut usize) -> Result<String, JsoncError> {
    let start = *pos;
    *pos += 1;
    let mut result = String::new();
    loop {
        match bytes.get(*pos) {
            None => return Err(JsoncError::UnclosedString(start)),
            Some(b'"') => {
                *pos += 1;
                return Ok(result);
            }
            Some(b'\\') => {
                *pos += 1;
                match bytes.get(*pos) {
                    Some(b'"') => result.push('"'),
                    Some(b'\\') => result.push('\\'),
                    Some(b'/') => result.push('/'),
                    Some(b'b') => result.push('\u{0008}'),
                    Some(b'f') => result.push('\u{000C}'),
                    Some(b'n') => result.push('\n'),
                    Some(b'r') => result.push('\r'),
                    Some(b't') => result.push('\t'),
                    Some(b'u') => {
                        let escape_start = *pos - 1;
                        let first = parse_hex4(bytes, escape_start, *pos + 1)?;
                        *pos += 4;
                        let code = if (0xD800..0xDC00).contains(&first) {
                            if bytes.get(*pos + 1..*pos + 3) != Some(b"\\u") {
                                return Err(JsoncError::InvalidEscape(escape_start));
                            }
                            let second = parse_hex4(bytes, escape_start, *pos + 3)?;
                            *pos += 6;
                            if !(0xDC00..0xE000).contains(&second) {
                                return Err(JsoncError::InvalidEscape(escape_start));
                            }
                            0x10000 + ((first - 0xD800) << 10) + (second - 0xDC00)
                        } else {
                            first
                        };
                        match char::from_u32(code) {
                            Some(c) => result.push(c),
                            None => return Err(JsoncError::InvalidEscape(escape_start)),
                        }
                    }
                    _ => return Err(JsoncError::InvalidEscape(*pos - 1)),
                }
                *pos += 1;
            }
            Some(&b) if b < 0x20 => return Err(JsoncError::UnexpectedChar(*pos)),
            Some(_) => {
                let char_start = *pos;
                *pos += 1;
                while let Some(&b) = bytes.get(*pos) {
                    if b & 0xC0 == 0x80 {
                        *pos += 1;
                    } else {
                        break;
                    }
                }
                match std::str::from_utf8(bytes.get(char_start..*pos).unwrap_or_default()) {
                    Ok(s) => result.push_str(s),
                    Err(_) => return Err(JsoncError::UnexpectedChar(char_start)),
                }
            }
        }
    }
}

fn parse_hex4(bytes: &[u8], escape_start: usize, at: usize) -> Result<u32, JsoncError> {
    bytes
        .get(at..at + 4)
        .and_then(|hex| std::str::from_utf8(hex).ok())
        .and_then(|hex| u32::from_str_radix(hex, 16).ok())
        .ok_or(JsoncError::InvalidEscape(escape_start))
}

fn parse_array(bytes: &[u8], pos: &mut usize, depth: usize) -> Result<Value, JsoncError> {
    *pos += 1;
    let mut items = Vec::new();
    loop {
        skip_whitespace_and_comments(bytes, pos)?;
        match bytes.get(*pos) {
            None => return Err(JsoncError::UnexpectedEof),
            Some(b']') => {
                *pos += 1;
                return Ok(Value::Array(items));
            }
            Some(_) => {
                items.push(parse_value(bytes, pos, depth + 1)?);
                skip_whitespace_and_comments(bytes, pos)?;
                match bytes.get(*pos) {
                    Some(b',') => *pos += 1,
                    Some(b']') => {}
                    None => return Err(JsoncError::UnexpectedEof),
                    Some(_) => return Err(JsoncError::UnexpectedChar(*pos)),
                }
            }
        }
    }
}

fn parse_object(bytes: &[u8], pos: &mut usize, depth: usize) -> Result<Value, JsoncError> {
    *pos += 1;
    let mut members = Vec::new();
    loop {
        skip_whitespace_and_comments(bytes, pos)?;
        match bytes.get(*pos) {
            None => return Err(JsoncError::UnexpectedEof),
            Some(b'}') => {
                *pos += 1;
                return Ok(Value::Object(members));
            }
            Some(b'"') => {
                let key = parse_string(bytes, pos)?;
                skip_whitespace_and_comments(bytes, pos)?;
                match bytes.get(*pos) {
                    Some(b':') => *pos += 1,
                    None => return Err(JsoncError::UnexpectedEof),
                    Some(_) => return Err(JsoncError::UnexpectedChar(*pos)),
                }
                skip_whitespace_and_comments(bytes, pos)?;
                members.push((key, parse_value(bytes, pos, depth + 1)?));
                skip_whitespace_and_comments(bytes, pos)?;
                match bytes.get(*pos) {
                    Some(b',') => *pos += 1,
                    Some(b'}') => {}
                    None => return Err(JsoncError::UnexpectedEof),
                    Some(_) => return Err(JsoncError::UnexpectedChar(*pos)),
                }
            }
            Some(_) => return Err(JsoncError::UnexpectedChar(*pos)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use random_string::{CharacterType, generate_random_string};
    use std::fs::File;

    fn urandom() -> File {
        File::open("/dev/urandom").unwrap()
    }

    fn random_name() -> String {
        generate_random_string(8, &[CharacterType::Lowercase], "", &mut urandom())
    }

    #[test]
    fn when_parse_with_null_then_returns_null() {
        assert_eq!(parse("null"), Ok(Value::Null));
    }

    #[test]
    fn when_parse_with_true_then_returns_bool_true() {
        assert_eq!(parse("true"), Ok(Value::Bool(true)));
    }

    #[test]
    fn when_parse_with_false_then_returns_bool_false() {
        assert_eq!(parse("false"), Ok(Value::Bool(false)));
    }

    #[test]
    fn when_parse_with_integer_then_returns_number() {
        assert_eq!(parse("42"), Ok(Value::Number(42.0)));
    }

    #[test]
    fn when_parse_with_negative_float_then_returns_number() {
        assert_eq!(parse("-3.5"), Ok(Value::Number(-3.5)));
    }

    #[test]
    fn when_parse_with_exponent_then_returns_number() {
        assert_eq!(parse("2e3"), Ok(Value::Number(2000.0)));
    }

    #[test]
    fn when_parse_with_string_then_returns_string() {
        let name = random_name();
        assert_eq!(parse(&format!("\"{}\"", name)), Ok(Value::String(name)));
    }

    #[test]
    fn when_parse_with_escaped_characters_then_unescapes() {
        assert_eq!(
            parse(r#""a\n\t\r\b\f\"\\\/b""#),
            Ok(Value::String("a\n\t\r\u{0008}\u{000C}\"\\/b".to_string()))
        );
    }

    #[test]
    fn when_parse_with_unicode_escape_then_decodes_code_point() {
        assert_eq!(
            parse(r#""\u00e9""#),
            Ok(Value::String("\u{00e9}".to_string()))
        );
    }

    #[test]
    fn when_parse_with_surrogate_pair_then_decodes_supplementary_character() {
        assert_eq!(
            parse(r#""\ud83d\ude00""#),
            Ok(Value::String("\u{1F600}".to_string()))
        );
    }

    #[test]
    fn when_parse_with_lone_high_surrogate_then_returns_error() {
        assert!(matches!(
            parse(r#""\ud83dx""#),
            Err(JsoncError::InvalidEscape(_))
        ));
    }

    #[test]
    fn when_parse_with_high_surrogate_not_followed_by_low_surrogate_then_returns_error() {
        assert!(matches!(
            parse(r#""\ud83d\u0041""#),
            Err(JsoncError::InvalidEscape(_))
        ));
    }

    #[test]
    fn when_parse_with_lone_low_surrogate_then_returns_error() {
        assert!(matches!(
            parse(r#""\udc00""#),
            Err(JsoncError::InvalidEscape(_))
        ));
    }

    #[test]
    fn when_parse_with_unknown_escape_then_returns_error() {
        assert!(matches!(
            parse(r#""\x""#),
            Err(JsoncError::InvalidEscape(_))
        ));
    }

    #[test]
    fn when_parse_with_multibyte_utf8_then_preserves_characters() {
        assert_eq!(parse("\"日本語\""), Ok(Value::String("日本語".to_string())));
    }

    #[test]
    fn when_parse_with_empty_object_then_returns_empty_object() {
        assert_eq!(parse("{}"), Ok(Value::Object(vec![])));
    }

    #[test]
    fn when_parse_with_object_then_preserves_member_order() {
        let result = parse(r#"{"b": 1, "a": 2}"#);
        assert_eq!(
            result,
            Ok(Value::Object(vec![
                ("b".to_string(), Value::Number(1.0)),
                ("a".to_string(), Value::Number(2.0)),
            ]))
        );
    }

    #[test]
    fn when_parse_with_nested_structures_then_returns_tree() {
        let result = parse(r#"{"a": [1, {"b": null}], "c": true}"#);
        assert_eq!(
            result,
            Ok(Value::Object(vec![
                (
                    "a".to_string(),
                    Value::Array(vec![
                        Value::Number(1.0),
                        Value::Object(vec![("b".to_string(), Value::Null)]),
                    ])
                ),
                ("c".to_string(), Value::Bool(true)),
            ]))
        );
    }

    #[test]
    fn when_parse_with_line_comment_then_ignores_comment() {
        let name = random_name();
        let input = format!("{{\n  // comment\n  \"key\": \"{}\"\n}}", name);
        assert_eq!(
            parse(&input),
            Ok(Value::Object(vec![(
                "key".to_string(),
                Value::String(name)
            )]))
        );
    }

    #[test]
    fn when_parse_with_block_comment_then_ignores_comment() {
        let input = "{ /* multi\nline */ \"key\": 1 }";
        assert_eq!(
            parse(input),
            Ok(Value::Object(vec![("key".to_string(), Value::Number(1.0))]))
        );
    }

    #[test]
    fn when_parse_with_comment_between_members_then_ignores_comment() {
        let input = "[1, // first\n 2 /* second */, 3]";
        assert_eq!(
            parse(input),
            Ok(Value::Array(vec![
                Value::Number(1.0),
                Value::Number(2.0),
                Value::Number(3.0),
            ]))
        );
    }

    #[test]
    fn when_parse_with_trailing_comma_in_object_then_accepts() {
        assert_eq!(
            parse("{\"a\": 1,}"),
            Ok(Value::Object(vec![("a".to_string(), Value::Number(1.0))]))
        );
    }

    #[test]
    fn when_parse_with_trailing_comma_in_array_then_accepts() {
        assert_eq!(
            parse("[1, 2,]"),
            Ok(Value::Array(vec![Value::Number(1.0), Value::Number(2.0)]))
        );
    }

    #[test]
    fn when_parse_with_comment_after_trailing_comma_then_accepts() {
        assert_eq!(
            parse("{\"a\": 1, // comment\n}"),
            Ok(Value::Object(vec![("a".to_string(), Value::Number(1.0))]))
        );
    }

    #[test]
    fn when_parse_with_empty_input_then_returns_error() {
        assert_eq!(parse(""), Err(JsoncError::UnexpectedEof));
    }

    #[test]
    fn when_parse_with_unexpected_character_then_returns_error() {
        assert!(matches!(parse("@"), Err(JsoncError::UnexpectedChar(0))));
    }

    #[test]
    fn when_parse_with_unclosed_string_then_returns_error() {
        assert!(matches!(parse("\"abc"), Err(JsoncError::UnclosedString(_))));
    }

    #[test]
    fn when_parse_with_unclosed_block_comment_then_returns_error() {
        assert!(matches!(
            parse("/* abc"),
            Err(JsoncError::UnclosedComment(_))
        ));
    }

    #[test]
    fn when_parse_with_trailing_characters_then_returns_error() {
        assert!(matches!(
            parse("1 2"),
            Err(JsoncError::TrailingCharacters(_))
        ));
    }

    #[test]
    fn when_parse_with_missing_colon_then_returns_error() {
        assert!(matches!(
            parse("{\"a\" 1}"),
            Err(JsoncError::UnexpectedChar(_))
        ));
    }

    #[test]
    fn when_parse_with_unterminated_array_after_comma_then_returns_error() {
        assert_eq!(parse("[1,"), Err(JsoncError::UnexpectedEof));
    }

    #[test]
    fn when_parse_with_unterminated_array_after_item_then_returns_error() {
        assert_eq!(parse("[1"), Err(JsoncError::UnexpectedEof));
    }

    #[test]
    fn when_parse_with_missing_comma_in_array_then_returns_error() {
        assert!(matches!(parse("[1 2]"), Err(JsoncError::UnexpectedChar(_))));
    }

    #[test]
    fn when_parse_with_unterminated_object_then_returns_error() {
        assert_eq!(parse("{"), Err(JsoncError::UnexpectedEof));
    }

    #[test]
    fn when_parse_with_unterminated_object_after_key_then_returns_error() {
        assert_eq!(parse(r#"{"a""#), Err(JsoncError::UnexpectedEof));
    }

    #[test]
    fn when_parse_with_unterminated_object_after_member_then_returns_error() {
        assert_eq!(parse(r#"{"a": 1"#), Err(JsoncError::UnexpectedEof));
    }

    #[test]
    fn when_parse_with_missing_comma_in_object_then_returns_error() {
        assert!(matches!(
            parse(r#"{"a": 1 "b": 2}"#),
            Err(JsoncError::UnexpectedChar(_))
        ));
    }

    #[test]
    fn when_parse_with_excessive_nesting_then_returns_depth_error() {
        let deep = "[".repeat(200) + &"]".repeat(200);
        assert_eq!(parse(&deep), Err(JsoncError::DepthLimitExceeded));
    }

    #[test]
    fn when_parse_with_invalid_number_then_returns_error() {
        assert!(matches!(parse("1.2.3"), Err(JsoncError::InvalidNumber(_))));
    }

    #[test]
    fn when_get_with_duplicate_keys_then_returns_last_value() {
        let value = parse(r#"{"a": 1, "a": 2}"#).unwrap();
        assert_eq!(value.get("a"), Some(&Value::Number(2.0)));
    }

    #[test]
    fn when_get_with_missing_key_then_returns_none() {
        let value = parse(r#"{"a": 1}"#).unwrap();
        assert_eq!(value.get("b"), None);
    }

    #[test]
    fn when_as_u64_with_fractional_number_then_returns_none() {
        assert_eq!(Value::Number(1.5).as_u64(), None);
    }

    #[test]
    fn when_as_object_with_array_then_returns_none() {
        assert_eq!(Value::Array(vec![Value::Number(1.0)]).as_object(), None);
    }

    #[test]
    fn when_to_string_map_with_string_values_then_returns_map() {
        let name = random_name();
        let value = parse(&format!(r#"{{"k": "{}"}}"#, name)).unwrap();
        let map = value.to_string_map().unwrap();
        assert_eq!(map.get("k"), Some(&name));
    }

    #[test]
    fn when_to_string_map_with_non_string_value_then_returns_none() {
        let value = parse(r#"{"k": 1}"#).unwrap();
        assert_eq!(value.to_string_map(), None);
    }

    #[test]
    fn when_to_json_pretty_with_nested_object_then_formats_with_two_space_indent() {
        let value = parse(r#"{"a": [1, 2], "b": {"c": null}}"#).unwrap();
        assert_eq!(
            value.to_json_pretty(),
            "{\n  \"a\": [\n    1,\n    2\n  ],\n  \"b\": {\n    \"c\": null\n  }\n}"
        );
    }

    #[test]
    fn when_to_json_pretty_with_empty_containers_then_formats_inline() {
        let value = parse(r#"{"a": [], "b": {}}"#).unwrap();
        assert_eq!(value.to_json_pretty(), "{\n  \"a\": [],\n  \"b\": {}\n}");
    }

    #[test]
    fn when_to_json_pretty_with_integer_number_then_omits_fraction() {
        assert_eq!(Value::Number(42.0).to_json_pretty(), "42");
    }

    #[test]
    fn when_to_json_pretty_with_float_number_then_keeps_fraction() {
        assert_eq!(Value::Number(1.5).to_json_pretty(), "1.5");
    }

    #[test]
    fn when_to_json_pretty_with_special_characters_then_escapes_them() {
        assert_eq!(
            Value::String("a\"b\\c\nd\re\tf\u{0008}g\u{000C}h".to_string()).to_json_pretty(),
            r#""a\"b\\c\nd\re\tf\bg\fh""#
        );
    }

    #[test]
    fn when_to_json_pretty_then_roundtrips_through_parse() {
        let name = random_name();
        let input = format!(r#"{{"key": "{}", "list": [true, false, null, 1.5]}}"#, name);
        let value = parse(&input).unwrap();
        assert_eq!(parse(&value.to_json_pretty()), Ok(value));
    }

    #[test]
    fn when_display_with_nested_structure_then_returns_compact_json() {
        let value = parse(r#"{"a": [1, "x", false], "b": {"c": null}, "d": []}"#).unwrap();
        assert_eq!(
            value.to_string(),
            r#"{"a":[1,"x",false],"b":{"c":null},"d":[]}"#
        );
    }

    #[test]
    fn when_display_with_number_then_returns_bare_number() {
        assert_eq!(Value::Number(20.0).to_string(), "20");
    }

    #[test]
    fn when_display_with_bool_then_returns_bare_bool() {
        assert_eq!(Value::Bool(true).to_string(), "true");
    }

    #[test]
    fn when_to_json_pretty_then_preserves_member_order() {
        let value = parse(r#"{"b": 1, "a": 2}"#).unwrap();
        let out = value.to_json_pretty();
        assert!(out.find("\"b\"").unwrap() < out.find("\"a\"").unwrap());
    }
}
