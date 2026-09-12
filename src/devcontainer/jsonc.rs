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

#[derive(Debug, Clone, PartialEq)]
pub struct Document {
    leading: String,
    root: Node,
    trailing: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    Object(ObjectNode),
    Array(ArrayNode),
    Scalar { raw: String, value: Value },
}

#[derive(Debug, Clone, Default, PartialEq)]
struct Trivia {
    prefix: String,
    indent: String,
}

impl From<&str> for Trivia {
    fn from(text: &str) -> Self {
        let prefix = text.trim_end_matches([' ', '\t']);
        Trivia {
            prefix: prefix.to_string(),
            indent: text[prefix.len()..].to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ObjectNode {
    members: Vec<MemberNode>,
    inner: Trivia,
}

#[derive(Debug, Clone, PartialEq)]
struct MemberNode {
    leading: Trivia,
    key: String,
    key_raw: String,
    colon: String,
    value: Node,
    comma: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ArrayNode {
    items: Vec<ItemNode>,
    inner: Trivia,
}

#[derive(Debug, Clone, PartialEq)]
struct ItemNode {
    leading: Trivia,
    value: Node,
    comma: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum InsertError {
    EmptyPath,
    RootNotObject,
    NotAnObject(String),
}

impl std::fmt::Display for InsertError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InsertError::EmptyPath => write!(f, "insert path is empty"),
            InsertError::RootNotObject => write!(f, "document root is not an object"),
            InsertError::NotAnObject(key) => write!(f, "'{key}' is not an object"),
        }
    }
}

impl std::error::Error for InsertError {}

pub fn parse(input: &str) -> Result<Document, JsoncError> {
    let bytes = input.as_bytes();
    let mut pos = 0;
    skip_whitespace_and_comments(bytes, &mut pos)?;
    let leading = input[..pos].to_string();
    let root = parse_value(input, &mut pos, 0)?;
    let start = pos;
    skip_whitespace_and_comments(bytes, &mut pos)?;
    let trailing = input[start..pos].to_string();
    if pos < input.len() {
        return Err(JsoncError::TrailingCharacters(pos));
    }
    Ok(Document {
        leading,
        root,
        trailing,
    })
}

impl Document {
    pub fn value(&self) -> Value {
        self.root.value()
    }

    pub fn to_text(&self) -> String {
        let mut out = String::new();
        out.push_str(&self.leading);
        self.root.write(&mut out);
        out.push_str(&self.trailing);
        out
    }

    pub fn insert(&mut self, path: &[&str], value: &Value) -> Result<(), InsertError> {
        let root = match &mut self.root {
            Node::Object(root) => root,
            _ => return Err(InsertError::RootNotObject),
        };
        let (leading, unit) = match root
            .members
            .iter()
            .find(|member| !member.leading.prefix.is_empty())
        {
            Some(member) => (
                Trivia {
                    prefix: "\n".to_string(),
                    indent: String::new(),
                },
                member.leading.indent.clone(),
            ),
            None => (
                Trivia {
                    prefix: String::new(),
                    indent: String::new(),
                },
                String::new(),
            ),
        };
        insert_into(root, path, value, &leading, &unit)
    }
}

impl Node {
    fn value(&self) -> Value {
        match self {
            Node::Scalar { value, .. } => value.clone(),
            Node::Object(object) => Value::Object(
                object
                    .members
                    .iter()
                    .map(|member| (member.key.clone(), member.value.value()))
                    .collect(),
            ),
            Node::Array(array) => {
                Value::Array(array.items.iter().map(|item| item.value.value()).collect())
            }
        }
    }

    fn write(&self, out: &mut String) {
        match self {
            Node::Scalar { raw, .. } => out.push_str(raw),
            Node::Object(object) => {
                out.push('{');
                for member in &object.members {
                    out.push_str(&member.leading.prefix);
                    out.push_str(&member.leading.indent);
                    out.push_str(&member.key_raw);
                    out.push_str(&member.colon);
                    member.value.write(out);
                    if let Some(comma) = &member.comma {
                        out.push_str(comma);
                    }
                }
                out.push_str(&object.inner.prefix);
                out.push_str(&object.inner.indent);
                out.push('}');
            }
            Node::Array(array) => {
                out.push('[');
                for item in &array.items {
                    out.push_str(&item.leading.prefix);
                    out.push_str(&item.leading.indent);
                    item.value.write(out);
                    if let Some(comma) = &item.comma {
                        out.push_str(comma);
                    }
                }
                out.push_str(&array.inner.prefix);
                out.push_str(&array.inner.indent);
                out.push(']');
            }
        }
    }
}

fn insert_into(
    object: &mut ObjectNode,
    path: &[&str],
    value: &Value,
    leading: &Trivia,
    unit: &str,
) -> Result<(), InsertError> {
    let (key, rest) = path.split_first().ok_or(InsertError::EmptyPath)?;
    let found = object.members.iter().rposition(|member| member.key == *key);
    let member_leading = match found {
        Some(index) => object.members[index].leading.clone(),
        None => {
            let broken = if object.inner.prefix.ends_with('\n') {
                object.inner.prefix.clone()
            } else {
                format!("{}\n", object.inner.prefix)
            };
            let member_leading = match object.members.last() {
                Some(last) if last.leading.prefix.is_empty() => Trivia {
                    prefix: object.inner.prefix.clone(),
                    indent: " ".to_string(),
                },
                Some(last) => Trivia {
                    prefix: broken,
                    indent: last.leading.indent.clone(),
                },
                None if leading.prefix.is_empty() => Trivia {
                    prefix: object.inner.prefix.clone(),
                    indent: String::new(),
                },
                None => Trivia {
                    prefix: broken,
                    indent: format!("{}{unit}", leading.indent),
                },
            };
            object.inner = if member_leading.prefix.is_empty() {
                object.inner.clone()
            } else if object.inner.prefix.is_empty() {
                Trivia {
                    prefix: "\n".to_string(),
                    indent: leading.indent.clone(),
                }
            } else {
                Trivia {
                    prefix: "\n".to_string(),
                    indent: object.inner.indent.clone(),
                }
            };
            member_leading
        }
    };
    match found {
        Some(index) if rest.is_empty() => {
            object.members[index].value = build_node(&[], value, &member_leading, unit);
            Ok(())
        }
        Some(index) => match &mut object.members[index].value {
            Node::Object(inner) => insert_into(inner, rest, value, &member_leading, unit),
            _ => Err(InsertError::NotAnObject((*key).to_string())),
        },
        None => {
            let node = build_node(rest, value, &member_leading, unit);
            let colon = match object.members.last_mut() {
                Some(last) => {
                    last.comma.get_or_insert_with(|| ",".to_string());
                    last.colon.clone()
                }
                None => ": ".to_string(),
            };
            let mut key_raw = String::new();
            write_string(key, &mut key_raw);
            object.members.push(MemberNode {
                leading: member_leading,
                key: (*key).to_string(),
                key_raw,
                colon,
                value: node,
                comma: None,
            });
            Ok(())
        }
    }
}

fn build_node(path: &[&str], value: &Value, leading: &Trivia, unit: &str) -> Node {
    let (child, inner) = if leading.prefix.is_empty() {
        (
            Trivia {
                prefix: String::new(),
                indent: String::new(),
            },
            Trivia {
                prefix: String::new(),
                indent: String::new(),
            },
        )
    } else {
        (
            Trivia {
                prefix: "\n".to_string(),
                indent: format!("{}{unit}", leading.indent),
            },
            Trivia {
                prefix: "\n".to_string(),
                indent: leading.indent.clone(),
            },
        )
    };
    if let Some((key, rest)) = path.split_first() {
        let mut key_raw = String::new();
        write_string(key, &mut key_raw);
        return Node::Object(ObjectNode {
            members: vec![MemberNode {
                leading: child.clone(),
                key: (*key).to_string(),
                key_raw,
                colon: ": ".to_string(),
                value: build_node(rest, value, &child, unit),
                comma: None,
            }],
            inner,
        });
    }
    match value {
        Value::Object(source) if !source.is_empty() => {
            let last = source.len() - 1;
            Node::Object(ObjectNode {
                members: source
                    .iter()
                    .enumerate()
                    .map(|(index, (key, member))| {
                        let mut key_raw = String::new();
                        write_string(key, &mut key_raw);
                        MemberNode {
                            leading: child.clone(),
                            key: key.clone(),
                            key_raw,
                            colon: ": ".to_string(),
                            value: build_node(&[], member, &child, unit),
                            comma: (index < last).then(|| ",".to_string()),
                        }
                    })
                    .collect(),
                inner,
            })
        }
        Value::Array(source) if !source.is_empty() => {
            let last = source.len() - 1;
            Node::Array(ArrayNode {
                items: source
                    .iter()
                    .enumerate()
                    .map(|(index, item)| ItemNode {
                        leading: child.clone(),
                        value: build_node(&[], item, &child, unit),
                        comma: (index < last).then(|| ",".to_string()),
                    })
                    .collect(),
                inner,
            })
        }
        Value::Object(_) => Node::Object(ObjectNode {
            members: vec![],
            inner: Trivia {
                prefix: String::new(),
                indent: String::new(),
            },
        }),
        Value::Array(_) => Node::Array(ArrayNode {
            items: vec![],
            inner: Trivia {
                prefix: String::new(),
                indent: String::new(),
            },
        }),
        scalar => {
            let mut raw = String::new();
            write_compact(scalar, &mut raw);
            Node::Scalar {
                raw,
                value: scalar.clone(),
            }
        }
    }
}

fn parse_value(src: &str, pos: &mut usize, depth: usize) -> Result<Node, JsoncError> {
    if depth > MAX_DEPTH {
        return Err(JsoncError::DepthLimitExceeded);
    }
    let bytes = src.as_bytes();
    let start = *pos;
    let value = match bytes.get(*pos) {
        None => return Err(JsoncError::UnexpectedEof),
        Some(b'{') => return parse_object(src, pos, depth),
        Some(b'[') => return parse_array(src, pos, depth),
        Some(b'"') => Value::String(parse_string(bytes, pos)?),
        Some(b't') => parse_keyword(bytes, pos, b"true", Value::Bool(true))?,
        Some(b'f') => parse_keyword(bytes, pos, b"false", Value::Bool(false))?,
        Some(b'n') => parse_keyword(bytes, pos, b"null", Value::Null)?,
        Some(b'-' | b'0'..=b'9') => parse_number(bytes, pos)?,
        Some(_) => return Err(JsoncError::UnexpectedChar(*pos)),
    };
    Ok(Node::Scalar {
        raw: src[start..*pos].to_string(),
        value,
    })
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

fn parse_array(src: &str, pos: &mut usize, depth: usize) -> Result<Node, JsoncError> {
    let bytes = src.as_bytes();
    *pos += 1;
    let mut items = Vec::new();
    let start = *pos;
    skip_whitespace_and_comments(bytes, pos)?;
    let mut inner = Trivia::from(&src[start..*pos]);
    loop {
        match bytes.get(*pos) {
            None => return Err(JsoncError::UnexpectedEof),
            Some(b']') => {
                *pos += 1;
                return Ok(Node::Array(ArrayNode { items, inner }));
            }
            Some(_) => {
                let leading = std::mem::take(&mut inner);
                let value = parse_value(src, pos, depth + 1)?;
                let after_value = *pos;
                skip_whitespace_and_comments(bytes, pos)?;
                let comma = match bytes.get(*pos) {
                    Some(b',') => {
                        *pos += 1;
                        Some(src[after_value..*pos].to_string())
                    }
                    Some(b']') => None,
                    None => return Err(JsoncError::UnexpectedEof),
                    Some(_) => return Err(JsoncError::UnexpectedChar(*pos)),
                };
                inner = match comma {
                    Some(_) => {
                        let start = *pos;
                        skip_whitespace_and_comments(bytes, pos)?;
                        Trivia::from(&src[start..*pos])
                    }
                    None => Trivia::from(&src[after_value..*pos]),
                };
                items.push(ItemNode {
                    leading,
                    value,
                    comma,
                });
            }
        }
    }
}

fn parse_object(src: &str, pos: &mut usize, depth: usize) -> Result<Node, JsoncError> {
    let bytes = src.as_bytes();
    *pos += 1;
    let mut members = Vec::new();
    let start = *pos;
    skip_whitespace_and_comments(bytes, pos)?;
    let mut inner = Trivia::from(&src[start..*pos]);
    loop {
        match bytes.get(*pos) {
            None => return Err(JsoncError::UnexpectedEof),
            Some(b'}') => {
                *pos += 1;
                return Ok(Node::Object(ObjectNode { members, inner }));
            }
            Some(b'"') => {
                let leading = std::mem::take(&mut inner);
                let key_start = *pos;
                let key = parse_string(bytes, pos)?;
                let key_raw = src[key_start..*pos].to_string();
                let colon_start = *pos;
                skip_whitespace_and_comments(bytes, pos)?;
                match bytes.get(*pos) {
                    Some(b':') => *pos += 1,
                    None => return Err(JsoncError::UnexpectedEof),
                    Some(_) => return Err(JsoncError::UnexpectedChar(*pos)),
                }
                skip_whitespace_and_comments(bytes, pos)?;
                let colon = src[colon_start..*pos].to_string();
                let value = parse_value(src, pos, depth + 1)?;
                let after_value = *pos;
                skip_whitespace_and_comments(bytes, pos)?;
                let comma = match bytes.get(*pos) {
                    Some(b',') => {
                        *pos += 1;
                        Some(src[after_value..*pos].to_string())
                    }
                    Some(b'}') => None,
                    None => return Err(JsoncError::UnexpectedEof),
                    Some(_) => return Err(JsoncError::UnexpectedChar(*pos)),
                };
                inner = match comma {
                    Some(_) => {
                        let start = *pos;
                        skip_whitespace_and_comments(bytes, pos)?;
                        Trivia::from(&src[start..*pos])
                    }
                    None => Trivia::from(&src[after_value..*pos]),
                };
                members.push(MemberNode {
                    leading,
                    key,
                    key_raw,
                    colon,
                    value,
                    comma,
                });
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
        assert_eq!(
            parse("null").map(|document| document.value()),
            Ok(Value::Null)
        );
    }

    #[test]
    fn when_parse_with_true_then_returns_bool_true() {
        assert_eq!(
            parse("true").map(|document| document.value()),
            Ok(Value::Bool(true))
        );
    }

    #[test]
    fn when_parse_with_false_then_returns_bool_false() {
        assert_eq!(
            parse("false").map(|document| document.value()),
            Ok(Value::Bool(false))
        );
    }

    #[test]
    fn when_parse_with_integer_then_returns_number() {
        assert_eq!(
            parse("42").map(|document| document.value()),
            Ok(Value::Number(42.0))
        );
    }

    #[test]
    fn when_parse_with_negative_float_then_returns_number() {
        assert_eq!(
            parse("-3.5").map(|document| document.value()),
            Ok(Value::Number(-3.5))
        );
    }

    #[test]
    fn when_parse_with_exponent_then_returns_number() {
        assert_eq!(
            parse("2e3").map(|document| document.value()),
            Ok(Value::Number(2000.0))
        );
    }

    #[test]
    fn when_parse_with_string_then_returns_string() {
        let name = random_name();
        assert_eq!(
            parse(&format!("\"{}\"", name)).map(|document| document.value()),
            Ok(Value::String(name))
        );
    }

    #[test]
    fn when_parse_with_escaped_characters_then_unescapes() {
        assert_eq!(
            parse(r#""a\n\t\r\b\f\"\\\/b""#).map(|document| document.value()),
            Ok(Value::String("a\n\t\r\u{0008}\u{000C}\"\\/b".to_string()))
        );
    }

    #[test]
    fn when_parse_with_unicode_escape_then_decodes_code_point() {
        assert_eq!(
            parse(r#""\u00e9""#).map(|document| document.value()),
            Ok(Value::String("\u{00e9}".to_string()))
        );
    }

    #[test]
    fn when_parse_with_surrogate_pair_then_decodes_supplementary_character() {
        assert_eq!(
            parse(r#""\ud83d\ude00""#).map(|document| document.value()),
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
        assert_eq!(
            parse("\"日本語\"").map(|document| document.value()),
            Ok(Value::String("日本語".to_string()))
        );
    }

    #[test]
    fn when_parse_with_empty_object_then_returns_empty_object() {
        assert_eq!(
            parse("{}").map(|document| document.value()),
            Ok(Value::Object(vec![]))
        );
    }

    #[test]
    fn when_parse_with_object_then_preserves_member_order() {
        let result = parse(r#"{"b": 1, "a": 2}"#).map(|document| document.value());
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
        let result =
            parse(r#"{"a": [1, {"b": null}], "c": true}"#).map(|document| document.value());
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
            parse(&input).map(|document| document.value()),
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
            parse(input).map(|document| document.value()),
            Ok(Value::Object(vec![("key".to_string(), Value::Number(1.0))]))
        );
    }

    #[test]
    fn when_parse_with_comment_between_members_then_ignores_comment() {
        let input = "[1, // first\n 2 /* second */, 3]";
        assert_eq!(
            parse(input).map(|document| document.value()),
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
            parse("{\"a\": 1,}").map(|document| document.value()),
            Ok(Value::Object(vec![("a".to_string(), Value::Number(1.0))]))
        );
    }

    #[test]
    fn when_parse_with_trailing_comma_in_array_then_accepts() {
        assert_eq!(
            parse("[1, 2,]").map(|document| document.value()),
            Ok(Value::Array(vec![Value::Number(1.0), Value::Number(2.0)]))
        );
    }

    #[test]
    fn when_parse_with_comment_after_trailing_comma_then_accepts() {
        assert_eq!(
            parse("{\"a\": 1, // comment\n}").map(|document| document.value()),
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
        let value = parse(r#"{"a": 1, "a": 2}"#).unwrap().value();
        assert_eq!(value.get("a"), Some(&Value::Number(2.0)));
    }

    #[test]
    fn when_get_with_missing_key_then_returns_none() {
        let value = parse(r#"{"a": 1}"#).unwrap().value();
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
        let value = parse(&format!(r#"{{"k": "{}"}}"#, name)).unwrap().value();
        let map = value.to_string_map().unwrap();
        assert_eq!(map.get("k"), Some(&name));
    }

    #[test]
    fn when_to_string_map_with_non_string_value_then_returns_none() {
        let value = parse(r#"{"k": 1}"#).unwrap().value();
        assert_eq!(value.to_string_map(), None);
    }

    #[test]
    fn when_to_json_pretty_with_nested_object_then_formats_with_two_space_indent() {
        let value = parse(r#"{"a": [1, 2], "b": {"c": null}}"#).unwrap().value();
        assert_eq!(
            value.to_json_pretty(),
            "{\n  \"a\": [\n    1,\n    2\n  ],\n  \"b\": {\n    \"c\": null\n  }\n}"
        );
    }

    #[test]
    fn when_to_json_pretty_with_empty_containers_then_formats_inline() {
        let value = parse(r#"{"a": [], "b": {}}"#).unwrap().value();
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
        let value = parse(&input).unwrap().value();
        assert_eq!(
            parse(&value.to_json_pretty()).map(|document| document.value()),
            Ok(value)
        );
    }

    #[test]
    fn when_display_with_nested_structure_then_returns_compact_json() {
        let value = parse(r#"{"a": [1, "x", false], "b": {"c": null}, "d": []}"#)
            .unwrap()
            .value();
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
        let value = parse(r#"{"b": 1, "a": 2}"#).unwrap().value();
        let out = value.to_json_pretty();
        assert!(out.find("\"b\"").unwrap() < out.find("\"a\"").unwrap());
    }

    #[test]
    fn when_parse_with_comments_and_tabs_then_roundtrips_through_to_text() {
        let input =
            "// header\n{\n\t\"name\": \"java\",\n\n\t// note\n\t\"image\": \"java:17\"\n}\n";
        assert_eq!(parse(input).unwrap().to_text(), input);
    }

    #[test]
    fn when_parse_with_trailing_comma_and_blank_lines_then_roundtrips_through_to_text() {
        let input = "{\n\n  \"a\": [1, 2 , 3,],\n\n  \"b\": {},\n}\n";
        assert_eq!(parse(input).unwrap().to_text(), input);
    }

    #[test]
    fn when_parse_with_escaped_key_then_roundtrips_through_to_text() {
        let input = r#"{"aA\"b": 1}"#;
        assert_eq!(parse(input).unwrap().to_text(), input);
    }

    #[test]
    fn when_insert_with_existing_object_then_keeps_the_comments_and_the_tab_indent() {
        let mut document =
            parse("// header\n{\n\t// note\n\t\"features\": {\n\t\t\"java\": {}\n\t}\n}\n")
                .unwrap();
        let result = document.insert(&["features", "git"], &Value::Object(vec![]));
        assert_eq!(result, Ok(()));
        assert_eq!(
            document.to_text(),
            "// header\n{\n\t// note\n\t\"features\": {\n\t\t\"java\": {},\n\t\t\"git\": {}\n\t}\n}\n"
        );
    }

    #[test]
    fn when_insert_with_trailing_comment_then_leaves_the_comment_on_its_line() {
        let mut document = parse("{\n\t\"image\": \"golang\" // note\n}\n").unwrap();
        let result = document.insert(&["features", "git"], &Value::Object(vec![]));
        assert_eq!(result, Ok(()));
        assert_eq!(
            document.to_text(),
            "{\n\t\"image\": \"golang\", // note\n\t\"features\": {\n\t\t\"git\": {}\n\t}\n}\n"
        );
    }

    #[test]
    fn when_insert_with_missing_key_then_returns_the_created_object() {
        let mut document = parse("{\n  \"name\": \"go\"\n}\n").unwrap();
        let result = document.insert(&["features", "git"], &Value::Object(vec![]));
        assert_eq!(result, Ok(()));
        assert_eq!(
            document.to_text(),
            "{\n  \"name\": \"go\",\n  \"features\": {\n    \"git\": {}\n  }\n}\n"
        );
    }

    #[test]
    fn when_insert_with_empty_object_then_indents_the_member() {
        let mut document = parse("{\n  \"features\": {}\n}\n").unwrap();
        let result = document.insert(&["features", "git"], &Value::Object(vec![]));
        assert_eq!(result, Ok(()));
        assert_eq!(
            document.to_text(),
            "{\n  \"features\": {\n    \"git\": {}\n  }\n}\n"
        );
    }

    #[test]
    fn when_insert_with_single_line_document_then_stays_on_one_line() {
        let mut document = parse(r#"{"image": "golang"}"#).unwrap();
        let result = document.insert(&["features", "git"], &Value::Object(vec![]));
        assert_eq!(result, Ok(()));
        assert_eq!(
            document.to_text(),
            r#"{"image": "golang", "features": {"git": {}}}"#
        );
    }

    #[test]
    fn when_insert_with_trailing_comma_then_does_not_add_another_comma() {
        let mut document = parse("{\n  \"image\": \"golang\",\n}\n").unwrap();
        let result = document.insert(&["features", "git"], &Value::Object(vec![]));
        assert_eq!(result, Ok(()));
        assert_eq!(
            document.to_text(),
            "{\n  \"image\": \"golang\",\n  \"features\": {\n    \"git\": {}\n  }\n}\n"
        );
    }

    #[test]
    fn when_insert_with_existing_key_then_replaces_the_value() {
        let mut document =
            parse("{\n  \"features\": {\n    \"git\": {\"version\": \"1\"}\n  }\n}\n").unwrap();
        let result = document.insert(&["features", "git"], &Value::Object(vec![]));
        assert_eq!(result, Ok(()));
        assert_eq!(
            document.to_text(),
            "{\n  \"features\": {\n    \"git\": {}\n  }\n}\n"
        );
    }

    #[test]
    fn when_insert_with_non_object_on_the_path_then_returns_not_an_object() {
        let mut document = parse(r#"{"features": 1}"#).unwrap();
        assert_eq!(
            document.insert(&["features", "git"], &Value::Object(vec![])),
            Err(InsertError::NotAnObject("features".to_string()))
        );
    }

    #[test]
    fn when_insert_with_non_object_root_then_returns_root_not_object() {
        let mut document = parse("[]").unwrap();
        assert_eq!(
            document.insert(&["features"], &Value::Object(vec![])),
            Err(InsertError::RootNotObject)
        );
    }

    #[test]
    fn when_insert_with_empty_path_then_returns_empty_path() {
        let mut document = parse("{}").unwrap();
        assert_eq!(
            document.insert(&[], &Value::Object(vec![])),
            Err(InsertError::EmptyPath)
        );
    }
}
