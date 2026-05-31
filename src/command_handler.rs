use crate::command::Command;
use crate::executor::{Executor, Reply};
use crate::resp::{
    encode_bulk_string, encode_error, encode_integer, encode_null, encode_simple_string,
};
use crate::server::{SharedAof, SharedStore};

pub async fn execute_command(
    command: Command,
    store: &SharedStore,
    aof: &SharedAof,
    resp: bool,
) -> String {
    match command {
        Command::AofRewrite => {
            let mut store = store.write().await;
            let entries = store.entries();

            match aof.rewrite(entries) {
                Ok(_) => encode_ok(resp),
                Err(error) => encode_failure(resp, &format!("failed to rewrite AOF: {error}")),
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
            {
                let mut store = store.write().await;
                store.flush_all();
            }

            match aof.clear() {
                Ok(_) => encode_ok(resp),
                Err(error) => encode_failure(resp, &format!("failed to clear AOF: {error}")),
            }
        }

        command if command.is_read_only() => {
            // Read-only commands take a shared lock, so they can run concurrently.
            let store = store.read().await;
            let result = Executor::execute_read(&store, &command);

            encode_executor_response(&result, resp)
        }

        command => {
            if should_persist(&command) {
                let line = command_to_aof_line(&command);

                if let Err(error) = aof.append(&line) {
                    return encode_failure(resp, &format!("failed to persist command: {error}"));
                }
            }

            let mut store = store.write().await;
            let result = Executor::execute(&mut store, command);

            encode_executor_response(&result, resp)
        }
    }
}

fn should_persist(command: &Command) -> bool {
    matches!(
        command,
        Command::Set { .. } | Command::SetEx { .. } | Command::Del { .. } | Command::Expire { .. }
    )
}

fn command_to_aof_line(command: &Command) -> String {
    match command {
        Command::Set { key, value } => {
            format!("SET {key} {value}")
        }

        Command::SetEx {
            key,
            seconds,
            value,
        } => {
            format!("SETEX {key} {seconds} {value}")
        }

        Command::Del { key } => {
            format!("DEL {key}")
        }

        Command::Expire { key, seconds } => {
            format!("EXPIRE {key} {seconds}")
        }

        _ => String::new(),
    }
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
