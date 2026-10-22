use crate::error::MemorustError;

pub enum RespParseResult {
    Complete(Vec<String>, usize),
    Incomplete,
    Error(MemorustError),
}

/// Upper bound for a whole RESP frame (header lines + every bulk string).
///
/// Every `Incomplete` state is bounded by this, so a client can't make the
/// server buffer an arbitrary amount of data for a frame it never finishes.
pub const MAX_FRAME_LEN: usize = 64 * 1024 * 1024;

/// Upper bound for the number of elements in a RESP array.
pub const MAX_ARRAY_LEN: usize = 1024;

/// Upper bound for a `*<n>` / `$<n>` header line, CRLF excluded.
const MAX_HEADER_LEN: usize = 32;

/// Parses one RESP array of bulk strings from the start of `buffer`.
///
/// Framing is done on raw bytes: lengths are byte counts, so the input is
/// never sliced as `&str` (which would panic on a non-char boundary). Each
/// bulk string must be valid UTF-8, because the store keeps `String`s.
pub fn parse_resp_frame(buffer: &[u8]) -> RespParseResult {
    match parse_frame(buffer) {
        Ok(Some((parts, consumed))) => RespParseResult::Complete(parts, consumed),
        Ok(None) => RespParseResult::Incomplete,
        Err(error) => RespParseResult::Error(error),
    }
}

/// `Ok(None)` means the frame is valid so far but needs more bytes.
fn parse_frame(buffer: &[u8]) -> Result<Option<(Vec<String>, usize)>, MemorustError> {
    let Some((count, mut position)) = read_header(buffer, 0, b'*')? else {
        return Ok(None);
    };

    if count > MAX_ARRAY_LEN {
        return Err(MemorustError::FrameTooLarge);
    }

    let mut parts = Vec::with_capacity(count);

    for _ in 0..count {
        let Some((len, value_start)) = read_header(buffer, position, b'$')? else {
            return Ok(None);
        };

        // `len` is checked against MAX_FRAME_LEN first, so the additions below
        // can't overflow.
        if len > MAX_FRAME_LEN {
            return Err(MemorustError::FrameTooLarge);
        }

        let value_end = value_start + len;
        let frame_end = value_end + 2;

        if frame_end > MAX_FRAME_LEN {
            return Err(MemorustError::FrameTooLarge);
        }

        if buffer.len() < frame_end {
            return Ok(None);
        }

        if &buffer[value_end..frame_end] != b"\r\n" {
            return Err(MemorustError::InvalidSyntax);
        }

        let value = std::str::from_utf8(&buffer[value_start..value_end])
            .map_err(|_| MemorustError::InvalidSyntax)?;

        parts.push(value.to_string());

        position = frame_end;
    }

    Ok(Some((parts, position)))
}

/// Reads a `<prefix><digits>\r\n` line starting at `start`.
///
/// Returns the parsed number and the position right after the CRLF, or
/// `Ok(None)` when the line isn't complete yet.
fn read_header(
    buffer: &[u8],
    start: usize,
    prefix: u8,
) -> Result<Option<(usize, usize)>, MemorustError> {
    let rest = &buffer[start..];

    let Some(line_len) = rest.windows(2).position(|window| window == b"\r\n") else {
        if rest.len() > MAX_HEADER_LEN + 1 {
            return Err(MemorustError::InvalidSyntax);
        }

        if let Some(&first) = rest.first() {
            if first != prefix {
                return Err(MemorustError::InvalidSyntax);
            }
        }

        return Ok(None);
    };

    let line = &rest[..line_len];

    if line.len() > MAX_HEADER_LEN || line.first() != Some(&prefix) {
        return Err(MemorustError::InvalidSyntax);
    }

    let digits = &line[1..];

    if digits.is_empty() || !digits.iter().all(u8::is_ascii_digit) {
        return Err(MemorustError::InvalidSyntax);
    }

    // ASCII digits only, so this is valid UTF-8; an overflow is a parse error.
    let value = std::str::from_utf8(digits)
        .ok()
        .and_then(|digits| digits.parse().ok())
        .ok_or(MemorustError::InvalidSyntax)?;

    Ok(Some((value, start + line_len + 2)))
}

/// Encodes `parts` as a RESP array of bulk strings.
///
/// Used as the AOF record format: lengths are explicit, so values can contain
/// spaces, quotes or newlines and still round-trip exactly.
pub fn encode_array<S: AsRef<str>>(parts: &[S]) -> String {
    let mut encoded = format!("*{}\r\n", parts.len());

    for part in parts {
        encoded.push_str(&encode_bulk_string(part.as_ref()));
    }

    encoded
}

pub fn encode_simple_string(value: &str) -> String {
    format!("+{value}\r\n")
}

pub fn encode_error(value: &str) -> String {
    format!("-{value}\r\n")
}

pub fn encode_bulk_string(value: &str) -> String {
    format!("${}\r\n{}\r\n", value.len(), value)
}

pub fn encode_null() -> String {
    "$-1\r\n".to_string()
}

pub fn encode_integer(value: i64) -> String {
    format!(":{value}\r\n")
}
