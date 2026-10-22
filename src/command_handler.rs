use std::sync::Arc;

use crate::aof::Record;
use crate::command::Command;
use crate::executor::{Executor, Reply};
use crate::resp::{
    encode_bulk_string, encode_error, encode_integer, encode_null, encode_simple_string,
};
use crate::server::{SharedAof, SharedStore};
use crate::store::Store;

pub async fn execute_command(
    command: Command,
    store: &SharedStore,
    aof: &SharedAof,
    resp: bool,
) -> String {
    match command {
        Command::AofRewrite => {
            // Only the snapshot is taken under the store lock; writing it to disk
            // happens on a blocking thread while other clients keep running.
            // Writes arriving meanwhile are captured by the AOF and appended to
            // the new file when it is swapped in.
            let entries = {
                let mut store = store.write().await;

                if let Err(error) = aof.start_rewrite() {
                    return encode_failure(resp, &format!("failed to rewrite AOF: {error}"));
                }

                store.entries()
            };

            let aof = Arc::clone(aof);

            match tokio::task::spawn_blocking(move || aof.complete_rewrite(entries)).await {
                Ok(Ok(())) => encode_ok(resp),
                Ok(Err(error)) => encode_failure(resp, &format!("failed to rewrite AOF: {error}")),
                Err(error) => encode_failure(resp, &format!("AOF rewrite task failed: {error}")),
            }
        }

        Command::Info => {
            let mut store = store.write().await;

            let response = format!(
                "memorust_version:0.1.0\r\nkeys:{}\r\naof_enabled:1\r\nttl_enabled:1\r\nresp_enabled:1",
                store.key_count()
            );

            if resp {
                encode_bulk_string(&response)
            } else {
                format!("{response}\r\n")
            }
        }

        Command::FlushAll => {
            // Truncate the AOF and wipe memory under the same lock, so no write
            // can land between the two. AOF first: if it fails, memory is untouched
            // and both still agree.
            let mut store = store.write().await;

            if let Err(error) = aof.clear() {
                return encode_failure(resp, &format!("failed to clear AOF: {error}"));
            }

            store.flush_all();

            encode_ok(resp)
        }

        command if command.is_read_only() => {
            // Read-only commands take a shared lock, so they can run concurrently.
            let store = store.read().await;
            let result = Executor::execute_read(&store, &command);

            encode_executor_response(&result, resp)
        }

        command => {
            // Append and apply under the same write lock: the AOF order is then
            // exactly the order mutations hit memory, so replay rebuilds the same
            // state. Appending only fills a buffer, so the lock stays short.
            let mut store = store.write().await;

            let records = aof_records(&command);

            if !records.is_empty() {
                if let Err(error) = aof.append(&records) {
                    return encode_failure(resp, &format!("failed to persist command: {error}"));
                }
            }

            let result = Executor::execute(&mut store, command);

            encode_executor_response(&result, resp)
        }
    }
}

/// AOF records that reproduce `command` on replay.
///
/// Relative TTLs are stored as absolute `EXPIREAT`s so replaying later doesn't
/// extend them. The match is exhaustive on purpose: a new command must decide
/// here whether it is persisted.
fn aof_records(command: &Command) -> Vec<Record> {
    match command {
        Command::Set { key, value } => vec![args(&["SET", key, value])],

        Command::SetEx {
            key,
            seconds,
            value,
        } => {
            let expires_at = Store::expiration_unix_ms_from_seconds(*seconds).to_string();

            vec![
                args(&["SET", key, value]),
                args(&["EXPIREAT", key, &expires_at]),
            ]
        }

        Command::Del { key } => vec![args(&["DEL", key])],

        Command::Expire { key, seconds } => {
            let expires_at = Store::expiration_unix_ms_from_seconds(*seconds).to_string();

            vec![args(&["EXPIREAT", key, &expires_at])]
        }

        Command::ExpireAt {
            key,
            unix_timestamp_ms,
        } => vec![args(&["EXPIREAT", key, &unix_timestamp_ms.to_string()])],

        Command::Get { .. }
        | Command::Exists { .. }
        | Command::Ttl { .. }
        | Command::Ping
        | Command::AofRewrite
        | Command::Info
        | Command::FlushAll => Vec::new(),
    }
}

fn args(parts: &[&str]) -> Record {
    parts.iter().map(|part| part.to_string()).collect()
}

fn encode_executor_response(reply: &Reply, resp: bool) -> String {
    if resp {
        match reply {
            Reply::Ok => encode_simple_string("OK"),
            Reply::Pong => encode_simple_string("PONG"),
            Reply::Nil => encode_null(),
            Reply::Integer(value) => encode_integer(*value),
            Reply::Bulk(value) => encode_bulk_string(value),
        }
    } else {
        match reply {
            Reply::Ok => "OK\r\n".to_string(),
            Reply::Pong => "PONG\r\n".to_string(),
            Reply::Nil => "nil\r\n".to_string(),
            Reply::Integer(value) => format!("{value}\r\n"),
            Reply::Bulk(value) => format!("{value}\r\n"),
        }
    }
}

fn encode_ok(resp: bool) -> String {
    if resp {
        encode_simple_string("OK")
    } else {
        "OK\r\n".to_string()
    }
}

fn encode_failure(resp: bool, message: &str) -> String {
    if resp {
        encode_error(&format!("ERR {message}"))
    } else {
        format!("ERROR: {message}\r\n")
    }
}
