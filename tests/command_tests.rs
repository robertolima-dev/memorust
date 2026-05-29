use memors::command::Command;
use memors::error::MemorsError;

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

    assert_eq!(command, Err(MemorsError::UnknownCommand));
}

#[test]
fn should_return_missing_argument_error() {
    let command = Command::parse("GET");

    assert_eq!(command, Err(MemorsError::MissingArgument));
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
