use crate::error::MemorsError;

pub enum RespParseResult {
    Complete(Vec<String>, usize),
    Incomplete,
    Error(MemorsError),
}

pub fn parse_resp_frame(buffer: &[u8]) -> RespParseResult {
    let input = match std::str::from_utf8(buffer) {
        Ok(value) => value,
        Err(_) => return RespParseResult::Incomplete,
    };

    let mut position = 0;

    let first_line_end = match find_crlf(input, position) {
        Some(pos) => pos,
        None => return RespParseResult::Incomplete,
    };

    let first_line = &input[position..first_line_end];

    if !first_line.starts_with('*') {
        return RespParseResult::Error(MemorsError::InvalidSyntax);
    }

    let count: usize = match first_line[1..].parse() {
        Ok(value) => value,
        Err(_) => return RespParseResult::Error(MemorsError::InvalidSyntax),
    };

    position = first_line_end + 2;

    let mut parts = Vec::new();

    for _ in 0..count {
        let len_line_end = match find_crlf(input, position) {
            Some(pos) => pos,
            None => return RespParseResult::Incomplete,
        };

        let len_line = &input[position..len_line_end];

        if !len_line.starts_with('$') {
            return RespParseResult::Error(MemorsError::InvalidSyntax);
        }

        let len: usize = match len_line[1..].parse() {
            Ok(value) => value,
            Err(_) => return RespParseResult::Error(MemorsError::InvalidSyntax),
        };

        position = len_line_end + 2;

        if input.len() < position + len + 2 {
            return RespParseResult::Incomplete;
        }

        let value = &input[position..position + len];

        parts.push(value.to_string());

        position += len;

        if &input[position..position + 2] != "\r\n" {
            return RespParseResult::Error(MemorsError::InvalidSyntax);
        }

        position += 2;
    }

    RespParseResult::Complete(parts, position)
}

fn find_crlf(input: &str, start: usize) -> Option<usize> {
    input[start..].find("\r\n").map(|pos| start + pos)
}

pub fn parse_resp(input: &str) -> Result<Vec<String>, MemorsError> {
    let mut lines = input.lines();

    let first = lines.next().ok_or(MemorsError::InvalidCommand)?;

    if !first.starts_with('*') {
        return Err(MemorsError::InvalidSyntax);
    }

    let count: usize = first[1..].parse().map_err(|_| MemorsError::InvalidSyntax)?;

    let mut result = Vec::new();

    for _ in 0..count {
        let len_lines = lines.next().ok_or(MemorsError::InvalidSyntax)?;

        if !len_lines.starts_with('$') {
            return Err(MemorsError::InvalidSyntax)?;
        }

        let value = lines.next().ok_or(MemorsError::InvalidSyntax)?;

        result.push(value.to_string());
    }

    Ok(result)
}

pub fn encode_simple_string(value: &str) -> String {
    format!("+{}\r\n", value)
}

pub fn encode_error(value: &str) -> String {
    format!("-{}\r\n", value)
}

pub fn encode_bulk_string(value: &str) -> String {
    format!("${}\r\n{}\r\n", value.len(), value)
}

pub fn encode_null() -> String {
    "$-1\r\n".to_string()
}

pub fn encode_integer(value: i64) -> String {
    format!(":{}\r\n", value)
}
