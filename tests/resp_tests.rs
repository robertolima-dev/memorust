use memorust::error::MemorustError;
use memorust::resp::{
    MAX_ARRAY_LEN, MAX_FRAME_LEN, RespParseResult, encode_array, encode_bulk_string, encode_error,
    encode_integer, encode_null, encode_simple_string, parse_resp_frame,
};

fn strings(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|part| part.to_string()).collect()
}

fn assert_complete(result: RespParseResult, expected: &[&str], expected_consumed: usize) {
    match result {
        RespParseResult::Complete(parts, consumed) => {
            assert_eq!(parts, strings(expected));
            assert_eq!(consumed, expected_consumed);
        }
        RespParseResult::Incomplete => panic!("expected Complete, got Incomplete"),
        RespParseResult::Error(error) => panic!("expected Complete, got Error({error})"),
    }
}

fn assert_error(result: RespParseResult, expected: MemorustError) {
    match result {
        RespParseResult::Error(error) => assert_eq!(error, expected),
        RespParseResult::Complete(parts, _) => panic!("expected Error, got Complete({parts:?})"),
        RespParseResult::Incomplete => panic!("expected Error, got Incomplete"),
    }
}

#[test]
fn should_parse_resp_set_command() {
    let input = b"*3\r\n$3\r\nSET\r\n$4\r\nname\r\n$7\r\nRoberto\r\n";

    assert_complete(
        parse_resp_frame(input),
        &["SET", "name", "Roberto"],
        input.len(),
    );
}

#[test]
fn should_parse_resp_get_command() {
    let input = b"*2\r\n$3\r\nGET\r\n$4\r\nname\r\n";

    assert_complete(parse_resp_frame(input), &["GET", "name"], input.len());
}

#[test]
fn should_report_every_fragment_of_a_frame_as_incomplete() {
    let input = "*2\r\n$3\r\nGET\r\n$5\r\nnaïv\r\n".as_bytes();

    // Every prefix, including ones that split the multi-byte `ï`.
    for end in 0..input.len() {
        assert!(
            matches!(parse_resp_frame(&input[..end]), RespParseResult::Incomplete),
            "prefix of {end} bytes should be incomplete"
        );
    }

    assert_complete(parse_resp_frame(input), &["GET", "naïv"], input.len());
}

#[test]
fn should_parse_only_the_first_of_several_frames() {
    let first = "*1\r\n$4\r\nPING\r\n";
    let input = format!("{first}*2\r\n$3\r\nGET\r\n$1\r\nk\r\n");

    assert_complete(parse_resp_frame(input.as_bytes()), &["PING"], first.len());
}

#[test]
fn should_keep_values_byte_for_byte() {
    let parts = ["SET", "key", "", "a  b", r#"{"x":1}"#, "x\r\ny", "é"];
    let encoded = encode_array(&parts);

    assert_complete(parse_resp_frame(encoded.as_bytes()), &parts, encoded.len());
}

#[test]
fn should_reject_a_length_that_splits_a_character_without_panicking() {
    // "é" is two bytes; a declared length of 1 cuts it in half.
    let input = "*1\r\n$1\r\né\r\n".as_bytes();

    assert_error(parse_resp_frame(input), MemorustError::InvalidSyntax);
}

#[test]
fn should_reject_invalid_utf8_instead_of_waiting() {
    let input = b"*1\r\n$2\r\n\xff\xfe\r\n";

    assert_error(parse_resp_frame(input), MemorustError::InvalidSyntax);
}

#[test]
fn should_reject_a_missing_crlf_after_a_value() {
    assert_error(
        parse_resp_frame(b"*1\r\n$3\r\nGETxx"),
        MemorustError::InvalidSyntax,
    );
}

#[test]
fn should_reject_malformed_headers() {
    for input in [
        &b"*x\r\n"[..],
        b"*-1\r\n",
        b"*+1\r\n",
        b"*\r\n",
        b"*1\r\n:3\r\n",
        b"*1\r\n$99999999999999999999999\r\n",
    ] {
        assert_error(parse_resp_frame(input), MemorustError::InvalidSyntax);
    }
}

#[test]
fn should_reject_a_header_that_never_ends() {
    let input = format!("*{}", "1".repeat(64));

    assert_error(
        parse_resp_frame(input.as_bytes()),
        MemorustError::InvalidSyntax,
    );
}

#[test]
fn should_reject_oversized_frames_before_buffering_them() {
    let huge_value = format!("*1\r\n${MAX_FRAME_LEN}\r\n");
    assert_error(
        parse_resp_frame(huge_value.as_bytes()),
        MemorustError::FrameTooLarge,
    );

    let huge_length = format!("*1\r\n${}\r\n", usize::MAX);
    assert_error(
        parse_resp_frame(huge_length.as_bytes()),
        MemorustError::FrameTooLarge,
    );

    let too_many = format!("*{}\r\n", MAX_ARRAY_LEN + 1);
    assert_error(
        parse_resp_frame(too_many.as_bytes()),
        MemorustError::FrameTooLarge,
    );
}

#[test]
fn should_encode_simple_string() {
    assert_eq!(encode_simple_string("OK"), "+OK\r\n");
}

#[test]
fn should_encode_bulk_string() {
    assert_eq!(encode_bulk_string("Roberto"), "$7\r\nRoberto\r\n");
}

#[test]
fn should_encode_null() {
    assert_eq!(encode_null(), "$-1\r\n");
}

#[test]
fn should_encode_integer() {
    assert_eq!(encode_integer(1), ":1\r\n");
}

#[test]
fn should_encode_error() {
    assert_eq!(
        encode_error("ERR unknown command"),
        "-ERR unknown command\r\n"
    );
}
