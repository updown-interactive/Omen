//! LSP base protocol framing; all IO runs on the language-service threads.
use omen::TextSnapshot;
use serde_json::{Value, json};
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

pub fn read_message(input: &mut impl BufRead) -> io::Result<Value> {
    let mut length = None;
    loop {
        let mut header = String::new();
        if input.read_line(&mut header)? == 0 {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        if header.trim().is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':')
            && name.eq_ignore_ascii_case("content-length")
        {
            length = value.trim().parse::<usize>().ok();
        }
    }
    let length = length
        .filter(|length| *length <= 32 * 1024 * 1024)
        .ok_or_else(|| io::Error::other("Invalid LSP Content-Length"))?;
    let mut body = vec![0; length];
    input.read_exact(&mut body)?;
    serde_json::from_slice(&body).map_err(io::Error::other)
}
pub fn write_message(output: &mut impl Write, message: &Value) -> io::Result<()> {
    let body = serde_json::to_vec(message)?;
    write!(output, "Content-Length: {}\r\n\r\n", body.len())?;
    output.write_all(&body)?;
    output.flush()
}
pub fn uri(path: &Path) -> String {
    let path = path.to_string_lossy().replace('\\', "/");
    let mut uri = if path.starts_with('/') {
        "file://".to_string()
    } else {
        "file:///".to_string()
    };
    for byte in path.bytes() {
        if byte.is_ascii_alphanumeric() || b"/-_.~:".contains(&byte) {
            uri.push(char::from(byte));
        } else {
            uri.push_str(&format!("%{byte:02X}"));
        }
    }
    uri
}
pub fn path(uri: &str) -> Option<PathBuf> {
    let encoded = uri.strip_prefix("file://")?;
    let encoded = if cfg!(windows) {
        encoded.strip_prefix('/').unwrap_or(encoded)
    } else {
        encoded
    };
    let mut bytes = Vec::new();
    let mut source = encoded.as_bytes().iter().copied();
    while let Some(byte) = source.next() {
        if byte == b'%' {
            let a = source.next()?;
            let b = source.next()?;
            bytes.push(u8::from_str_radix(std::str::from_utf8(&[a, b]).ok()?, 16).ok()?);
        } else {
            bytes.push(byte);
        }
    }
    String::from_utf8(bytes).ok().map(PathBuf::from)
}
pub fn position(text: &TextSnapshot, offset: usize) -> Value {
    let row = text.byte_to_line(offset);
    json!({"line":row,"character":text.byte_to_utf16(offset)-text.byte_to_utf16(text.line_start(row))})
}
pub fn offset(text: &TextSnapshot, position: &Value) -> usize {
    let row = position["line"].as_u64().unwrap_or(0) as usize;
    let row = row.min(text.line_count().saturating_sub(1));
    let start = text.line_start(row);
    let end = start + text.line(row).len();
    text.utf16_to_byte(
        text.byte_to_utf16(start) + position["character"].as_u64().unwrap_or(0) as usize,
    )
    .min(end)
}
pub fn range(text: &TextSnapshot, value: &Value) -> std::ops::Range<usize> {
    let start = offset(text, &value["start"]);
    start..offset(text, &value["end"]).max(start)
}
pub fn edits(text: &TextSnapshot, values: &Value) -> TextSnapshot {
    let mut edits = values
        .as_array()
        .into_iter()
        .flatten()
        .map(|edit| {
            (
                range(text, &edit["range"]),
                edit["newText"].as_str().unwrap_or_default(),
            )
        })
        .collect::<Vec<_>>();
    edits.sort_by_key(|edit| std::cmp::Reverse(edit.0.start));
    let mut result = text.clone();
    for (range, inserted) in edits {
        result.replace(range, inserted);
    }
    result
}

/// Validate server-provided edits before applying any changes to a buffer.
pub fn checked_edits(text: &TextSnapshot, values: &Value) -> io::Result<TextSnapshot> {
    let values = values
        .as_array()
        .ok_or_else(|| io::Error::other("Invalid language-server text edits"))?;
    let mut ranges = Vec::new();
    for value in values {
        if !value["newText"].is_string() {
            return Err(io::Error::other("Missing replacement text"));
        }
        for key in ["start", "end"] {
            let position = &value["range"][key];
            let row = position["line"]
                .as_u64()
                .ok_or_else(|| io::Error::other("Invalid edit line"))?
                as usize;
            let column = position["character"]
                .as_u64()
                .ok_or_else(|| io::Error::other("Invalid edit column"))?
                as usize;
            if row >= text.line_count() || column > text.line(row).encode_utf16().count() {
                return Err(io::Error::other(
                    "Language edit is outside the current document",
                ));
            }
        }
        let start = offset(text, &value["range"]["start"]);
        let end = offset(text, &value["range"]["end"]);
        if end < start {
            return Err(io::Error::other("Invalid edit range"));
        }
        ranges.push(start..end);
    }
    ranges.sort_by_key(|range| range.start);
    if ranges.windows(2).any(|pair| pair[0].end > pair[1].start) {
        return Err(io::Error::other(
            "Overlapping language edits are not supported",
        ));
    }
    Ok(edits(text, &Value::Array(values.clone())))
}
