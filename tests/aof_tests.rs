use std::fs;

use memorust::aof::{Aof, Record};

fn temp_path(name: &str) -> String {
    let path = std::env::temp_dir().join(format!("memorust_{}_{name}.aof", std::process::id()));
    let path = path.to_str().unwrap().to_string();

    let _ = fs::remove_file(&path);

    path
}

fn record(parts: &[&str]) -> Record {
    parts.iter().map(|part| part.to_string()).collect()
}

#[test]
fn should_append_and_load_commands() {
    let path = temp_path("append_and_load");

    let aof = Aof::new(&path).unwrap();

    aof.append(&[record(&["SET", "name", "Roberto"])]).unwrap();
    aof.append(&[record(&["SET", "language", "Rust"])]).unwrap();

    // `append` only buffers; durability is provided by `flush` (called
    // periodically by the server's AOF flusher task).
    aof.flush().unwrap();

    let commands = aof.load().unwrap();

    assert_eq!(
        commands,
        vec![
            record(&["SET", "name", "Roberto"]),
            record(&["SET", "language", "Rust"]),
        ]
    );

    let _ = fs::remove_file(&path);
}

#[test]
fn should_return_empty_when_file_does_not_exist() {
    let path = temp_path("missing");

    let aof = Aof::new(&path).unwrap();
    let _ = fs::remove_file(&path);

    let commands = aof.load().unwrap();

    assert!(commands.is_empty());
}

#[test]
fn should_round_trip_values_exactly() {
    let path = temp_path("round_trip");

    let values = [
        "",
        "a  b",
        r#"{"x":1}"#,
        "a\"b",
        "line\nSET injected value",
        "crlf\r\ninside",
        "ação é ✓",
    ];

    let aof = Aof::new(&path).unwrap();

    for (index, value) in values.iter().enumerate() {
        let key = format!("key {index}");
        aof.append(&[record(&["SET", &key, value])]).unwrap();
    }

    aof.flush().unwrap();

    let commands = aof.load().unwrap();

    assert_eq!(commands.len(), values.len());

    for (index, value) in values.iter().enumerate() {
        assert_eq!(
            commands[index],
            record(&["SET", &format!("key {index}"), value])
        );
    }

    let _ = fs::remove_file(&path);
}

#[test]
fn clear_should_discard_buffered_records() {
    let path = temp_path("clear_buffered");

    let aof = Aof::new(&path).unwrap();

    aof.append(&[record(&["SET", "old", "value"])]).unwrap();
    aof.clear().unwrap();
    aof.append(&[record(&["SET", "new", "1"])]).unwrap();
    aof.flush().unwrap();

    assert_eq!(aof.load().unwrap(), vec![record(&["SET", "new", "1"])]);

    let _ = fs::remove_file(&path);
}

#[test]
fn clear_should_truncate_flushed_records() {
    let path = temp_path("clear_flushed");

    let aof = Aof::new(&path).unwrap();

    aof.append(&[record(&["SET", "a_long_key_name", "a long value"])])
        .unwrap();
    aof.flush().unwrap();
    aof.clear().unwrap();
    aof.append(&[record(&["SET", "n", "1"])]).unwrap();
    aof.flush().unwrap();

    assert_eq!(aof.load().unwrap(), vec![record(&["SET", "n", "1"])]);

    let _ = fs::remove_file(&path);
}

#[test]
fn rewrite_should_write_snapshot_with_ttl_and_keep_appending() {
    let path = temp_path("rewrite");

    let aof = Aof::new(&path).unwrap();

    // Buffered before the rewrite: superseded by the snapshot.
    aof.append(&[record(&["SET", "stale", "1"])]).unwrap();

    aof.start_rewrite().unwrap();
    aof.complete_rewrite(vec![
        ("plain".to_string(), "v".to_string(), None),
        (
            "session".to_string(),
            "abc".to_string(),
            Some(4_000_000_000_000),
        ),
    ])
    .unwrap();

    aof.append(&[record(&["DEL", "plain"])]).unwrap();
    aof.flush().unwrap();

    assert_eq!(
        aof.load().unwrap(),
        vec![
            record(&["SET", "plain", "v"]),
            record(&["SET", "session", "abc"]),
            record(&["EXPIREAT", "session", "4000000000000"]),
            record(&["DEL", "plain"]),
        ]
    );

    let _ = fs::remove_file(&path);
}

#[test]
fn should_load_legacy_plain_text_lines() {
    let path = temp_path("legacy");

    fs::write(&path, "SET name Roberto\n\nDEL city\n").unwrap();

    let aof = Aof::new(&path).unwrap();
    aof.append(&[record(&["SET", "x", "a b"])]).unwrap();
    aof.flush().unwrap();

    assert_eq!(
        aof.load().unwrap(),
        vec![
            record(&["SET", "name", "Roberto"]),
            record(&["DEL", "city"]),
            record(&["SET", "x", "a b"]),
        ]
    );

    let _ = fs::remove_file(&path);
}

#[test]
fn should_drop_and_truncate_an_incomplete_last_record() {
    let path = temp_path("truncated_tail");

    let complete = "*3\r\n$3\r\nSET\r\n$1\r\na\r\n$1\r\n1\r\n";
    fs::write(&path, format!("{complete}*3\r\n$3\r\nSET\r\n$1\r\nb")).unwrap();

    let aof = Aof::new(&path).unwrap();

    assert_eq!(aof.load().unwrap(), vec![record(&["SET", "a", "1"])]);
    assert_eq!(fs::read_to_string(&path).unwrap(), complete);

    // New records land right after the last complete one.
    aof.append(&[record(&["SET", "c", "3"])]).unwrap();
    aof.flush().unwrap();

    assert_eq!(
        aof.load().unwrap(),
        vec![record(&["SET", "a", "1"]), record(&["SET", "c", "3"])]
    );

    let _ = fs::remove_file(&path);
}

#[test]
fn should_fail_on_corruption_in_the_middle() {
    let path = temp_path("corrupted");

    fs::write(
        &path,
        "*2\r\n$3\r\nDEL\r\n$1\r\nxyz\r\n*2\r\n$3\r\nDEL\r\n$1\r\ny\r\n",
    )
    .unwrap();

    let aof = Aof::new(&path).unwrap();
    let error = aof.load().unwrap_err();

    assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);

    let _ = fs::remove_file(&path);
}

#[test]
fn records_appended_during_a_rewrite_follow_the_snapshot() {
    let path = temp_path("rewrite_pending");

    let aof = Aof::new(&path).unwrap();

    aof.append(&[record(&["SET", "a", "1"])]).unwrap();
    aof.start_rewrite().unwrap();

    // Arrive while the snapshot is being written.
    aof.append(&[record(&["SET", "b", "2"])]).unwrap();
    aof.append(&[record(&["DEL", "a"])]).unwrap();

    aof.complete_rewrite(vec![("a".to_string(), "1".to_string(), None)])
        .unwrap();

    aof.append(&[record(&["SET", "c", "3"])]).unwrap();
    aof.flush().unwrap();

    assert_eq!(
        aof.load().unwrap(),
        vec![
            record(&["SET", "a", "1"]),
            record(&["SET", "b", "2"]),
            record(&["DEL", "a"]),
            record(&["SET", "c", "3"]),
        ]
    );

    let _ = fs::remove_file(&path);
}

#[test]
fn flushall_during_a_rewrite_aborts_it() {
    let path = temp_path("rewrite_flushall");

    let aof = Aof::new(&path).unwrap();

    aof.append(&[record(&["SET", "a", "1"])]).unwrap();
    aof.start_rewrite().unwrap();
    aof.clear().unwrap();
    aof.append(&[record(&["SET", "b", "2"])]).unwrap();

    let error = aof
        .complete_rewrite(vec![("a".to_string(), "1".to_string(), None)])
        .unwrap_err();

    assert!(error.to_string().contains("FLUSHALL"));

    aof.flush().unwrap();

    // The live file is untouched by the aborted rewrite.
    assert_eq!(aof.load().unwrap(), vec![record(&["SET", "b", "2"])]);
    assert!(!std::path::Path::new(&format!("{path}.tmp")).exists());

    // And a new rewrite can start.
    aof.start_rewrite().unwrap();
    aof.complete_rewrite(Vec::new()).unwrap();

    let _ = fs::remove_file(&path);
}

#[test]
fn only_one_rewrite_runs_at_a_time() {
    let path = temp_path("rewrite_twice");

    let aof = Aof::new(&path).unwrap();

    assert!(aof.complete_rewrite(Vec::new()).is_err());

    aof.start_rewrite().unwrap();
    assert!(aof.start_rewrite().is_err());

    aof.complete_rewrite(Vec::new()).unwrap();
    aof.start_rewrite().unwrap();

    let _ = fs::remove_file(&path);
}
