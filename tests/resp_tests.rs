use memorust::resp::{
    encode_bulk_string, encode_error, encode_integer, encode_null, encode_simple_string, parse_resp,
};

#[test]
fn should_parse_resp_set_command() {
    let input = "*3\r\n$3\r\nSET\r\n$4\r\nname\r\n$7\r\nRoberto\r\n";

    let result = parse_resp(input);

    assert_eq!(
        result,
        Ok(vec![
            "SET".to_string(),
            "name".to_string(),
            "Roberto".to_string(),
        ])
    );
}

#[test]
fn should_parse_resp_get_command() {
    let input = "*2\r\n$3\r\nGET\r\n$4\r\nname\r\n";

    let result = parse_resp(input);

    assert_eq!(result, Ok(vec!["GET".to_string(), "name".to_string()]));
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
