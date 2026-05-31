use memorust::error::MemorustError;
use memorust::tokenizer::tokenize;

#[test]
fn should_tokenize_simple_command() {
    let tokens = tokenize("SET name Roberto");

    assert_eq!(
        tokens,
        Ok(vec![
            "SET".to_string(),
            "name".to_string(),
            "Roberto".to_string()
        ])
    );
}

#[test]
fn should_tokenize_json_as_raw_string() {
    let tokens = tokenize("SET user {\"id\":123,\"name\":\"Roberto\"}");

    assert_eq!(
        tokens,
        Ok(vec![
            "SET".to_string(),
            "user".to_string(),
            "{id:123,name:Roberto}".to_string()
        ])
    )
}

#[test]
fn should_return_invalid_syntax_when_quotes_are_not_closed() {
    let tokens = tokenize("SET bio \"Rust developer");

    assert_eq!(tokens, Err(MemorustError::InvalidSyntax));
}
