use memorust::command::Command;
use memorust::error::MemorustError;

#[test]
fn should_parse_set_command() {
    let command = Command::parse("SET name Roberto");

    assert_eq!(
        command,
        Ok(Command::Set {
            key: "name".to_string(),
            value: "Roberto".to_string(),
        })
    );
}

#[test]
fn should_parse_get_command() {
    let command = Command::parse("GET name");

    assert_eq!(
        command,
        Ok(Command::Get {
            key: "name".to_string(),
        })
    );
}

#[test]
fn should_parse_del_command() {
    let command = Command::parse("DEL name");

    assert_eq!(
        command,
        Ok(Command::Del {
            key: "name".to_string(),
        })
    );
}

#[test]
fn should_parse_exists_command() {
    let command = Command::parse("EXISTS name");

    assert_eq!(
        command,
        Ok(Command::Exists {
            key: "name".to_string(),
        })
    );
}

#[test]
fn should_parse_ping_command() {
    let command = Command::parse("PING");

    assert_eq!(command, Ok(Command::Ping),);
}

#[test]
fn should_return_unknown_command_error() {
    let command = Command::parse("INVALID");

    assert_eq!(command, Err(MemorustError::UnknownCommand));
}

#[test]
fn should_return_missing_argument_error() {
    let command = Command::parse("GET");

    assert_eq!(command, Err(MemorustError::MissingArgument));
}

#[test]
fn should_parse_set_command_with_quoted_value() {
    let command = Command::parse("SET bio \"Rust developer\"");

    assert_eq!(
        command,
        Ok(Command::Set {
            key: "bio".to_string(),
            value: "Rust developer".to_string(),
        })
    );
}

#[test]
fn should_parse_setexp_command() {
    let command = Command::parse("SETEX name 60 Roberto");

    assert_eq!(
        command,
        Ok(Command::SetEx {
            key: "name".to_string(),
            seconds: 60,
            value: "Roberto".to_string()
        }),
    );
}

#[test]
fn should_parse_expire_command() {
    let command = Command::parse("EXPIRE name 60");

    assert_eq!(
        command,
        Ok(Command::Expire {
            key: "name".to_string(),
            seconds: 60
        }),
    );
}

#[test]
fn should_parse_ttl_command() {
    let command = Command::parse("TTL name");

    assert_eq!(
        command,
        Ok(Command::Ttl {
            key: "name".to_string()
        }),
    );
}

fn args(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|part| part.to_string()).collect()
}

#[test]
fn from_args_should_keep_values_untouched() {
    for value in ["a  b", r#"{"x":1}"#, "", "line\nbreak"] {
        assert_eq!(
            Command::from_args(&args(&["SET", "key", value])),
            Ok(Command::Set {
                key: "key".to_string(),
                value: value.to_string(),
            }),
        );
    }
}

#[test]
fn from_args_should_keep_keys_with_spaces() {
    assert_eq!(
        Command::from_args(&args(&["GET", "my key"])),
        Ok(Command::Get {
            key: "my key".to_string()
        }),
    );
}

#[test]
fn should_reject_expire_times_that_would_overflow() {
    assert_eq!(
        Command::parse("EXPIRE name 18446744073709551615"),
        Err(MemorustError::InvalidExpireTime)
    );
    assert_eq!(
        Command::parse("SETEX name 18446744073709551615 Roberto"),
        Err(MemorustError::InvalidExpireTime)
    );
    assert_eq!(
        Command::parse("EXPIREAT name 340282366920938463463374607431768211455"),
        Err(MemorustError::InvalidExpireTime)
    );
}
