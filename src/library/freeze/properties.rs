//! Reading and writing `java.util.Properties` files, so a `stored.rules` index can be shared
//! with a Java ArchUnit project.

use std::collections::BTreeMap;
use std::fmt::Write as _;

/// Parses a `.properties` file into key/value pairs (`Properties.load`).
pub(crate) fn load(text: &str) -> BTreeMap<String, String> {
    let mut result = BTreeMap::new();
    let mut logical = String::new();
    let mut lines = text.lines().peekable();
    while let Some(raw) = lines.next() {
        let line = raw.trim_start_matches([' ', '\t', '\u{c}']);
        if logical.is_empty() && (line.is_empty() || line.starts_with('#') || line.starts_with('!'))
        {
            continue;
        }
        logical.push_str(line);
        if ends_with_odd_backslashes(&logical) {
            logical.pop();
            if lines.peek().is_some() {
                continue;
            }
        }
        let (key, value) = split_key_value(&logical);
        result.insert(unescape(key), unescape(value));
        logical.clear();
    }
    result
}

fn ends_with_odd_backslashes(s: &str) -> bool {
    s.chars().rev().take_while(|c| *c == '\\').count() % 2 == 1
}

fn split_key_value(line: &str) -> (&str, &str) {
    let bytes = line.as_bytes();
    let mut i = 0;
    let mut key_end = None;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 2,
            b'=' | b':' | b' ' | b'\t' | b'\x0c' => {
                key_end = Some(i);
                break;
            }
            _ => i += 1,
        }
    }
    let Some(key_end) = key_end else {
        return (line, "");
    };
    let mut value_start = key_end;
    while value_start < bytes.len() && matches!(bytes[value_start], b' ' | b'\t' | b'\x0c') {
        value_start += 1;
    }
    if value_start < bytes.len() && matches!(bytes[value_start], b'=' | b':') {
        value_start += 1;
    }
    while value_start < bytes.len() && matches!(bytes[value_start], b' ' | b'\t' | b'\x0c') {
        value_start += 1;
    }
    (&line[..key_end], &line[value_start..])
}

fn unescape(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            result.push(c);
            continue;
        }
        match chars.next() {
            Some('t') => result.push('\t'),
            Some('n') => result.push('\n'),
            Some('r') => result.push('\r'),
            Some('f') => result.push('\u{c}'),
            Some('u') => {
                let hex: String = chars.by_ref().take(4).collect();
                match u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                    Some(decoded) => result.push(decoded),
                    None => {
                        result.push_str("\\u");
                        result.push_str(&hex);
                    }
                }
            }
            Some(other) => result.push(other),
            None => {}
        }
    }
    result
}

/// Serializes key/value pairs (`Properties.store` with an empty comment).
pub(crate) fn store(properties: &BTreeMap<String, String>) -> String {
    let mut out = String::from("#\n");
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default();
    let _ = writeln!(out, "#{seconds}");
    for (key, value) in properties {
        out.push_str(&escape(key, true));
        out.push('=');
        out.push_str(&escape(value, false));
        out.push('\n');
    }
    out
}

fn escape(s: &str, is_key: bool) -> String {
    let mut result = String::with_capacity(s.len());
    for (i, c) in s.chars().enumerate() {
        match c {
            ' ' if is_key || i == 0 => result.push_str("\\ "),
            '\\' => result.push_str("\\\\"),
            '\t' => result.push_str("\\t"),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\u{c}' => result.push_str("\\f"),
            '=' | ':' | '#' | '!' => {
                result.push('\\');
                result.push(c);
            }
            c if !(' '..='~').contains(&c) => {
                let _ = write!(result, "\\u{:04X}", c as u32);
            }
            c => result.push(c),
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_special_characters() {
        let mut properties = BTreeMap::new();
        properties.insert(
            "rule: with = and\nnewline #and ünïcode".to_owned(),
            " value with leading space".to_owned(),
        );
        properties.insert("plain".to_owned(), "abc".to_owned());
        let text = store(&properties);
        assert!(text.contains("rule\\:\\ with\\ \\=\\ and\\nnewline\\ \\#and\\ \\u00FCn\\u00EFcode=\\ value with leading space"));
        assert_eq!(load(&text), properties);
    }

    #[test]
    fn loads_java_style_files() {
        let text = "#comment\n!another\nkey1 = value1\nkey2:value2\nkey3 value3\nlong\\\n    continued=x\\\n  y\n";
        let loaded = load(text);
        assert_eq!(loaded["key1"], "value1");
        assert_eq!(loaded["key2"], "value2");
        assert_eq!(loaded["key3"], "value3");
        assert_eq!(loaded["longcontinued"], "xy");
    }
}
