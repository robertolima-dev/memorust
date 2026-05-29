use std::fs;

use memors::aof::Aof;

#[test]
fn should_append_and_load_commands() {
    let path = "test_appendonly.aof";

    let _ = fs::remove_file(path);

    let aof = Aof::new(path);

    aof.append("SET name Roberto").unwrap();
    aof.append("SET language Rust").unwrap();

    let commands = aof.load().unwrap();

    assert_eq!(
        commands,
        vec![
            "SET name Roberto".to_string(),
            "SET language Rust".to_string(),
        ]
    );

    let _ = fs::remove_file(path);
}

#[test]
fn should_return_empty_when_file_does_not_exist() {
    let path = "missing_appendonly.aof";

    let _ = fs::remove_file(path);

    let aof = Aof::new(path);

    let commands = aof.load().unwrap();

    assert!(commands.is_empty());
}
