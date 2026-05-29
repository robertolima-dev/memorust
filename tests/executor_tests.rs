use memors::command::Command;
use memors::executor::{Executor, Reply};
use memors::store::Store;

#[test]
fn should_execute_set_command() {
    let mut store = Store::new();

    let result = Executor::execute(
        &mut store,
        Command::Set {
            key: "name".to_string(),
            value: "Roberto".to_string(),
        },
    );

    assert_eq!(result, Reply::Ok);
}

#[test]
fn should_execute_get_command() {
    let mut store = Store::new();

    store.set("name".to_string(), "Roberto".to_string());

    let result = Executor::execute(
        &mut store,
        Command::Get {
            key: "name".to_string(),
        },
    );

    assert_eq!(result, Reply::Bulk("Roberto".to_string()));
}

#[test]
fn should_return_nil_when_key_does_not_exist() {
    let mut store = Store::new();

    let result = Executor::execute(
        &mut store,
        Command::Get {
            key: "name".to_string(),
        },
    );

    assert_eq!(result, Reply::Nil);
}

#[test]
fn should_execute_ping_command() {
    let mut store = Store::new();

    let result = Executor::execute(&mut store, Command::Ping);

    assert_eq!(result, Reply::Pong);
}
