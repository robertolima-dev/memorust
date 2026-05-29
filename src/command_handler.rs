use crate::command::Command;
use crate::executor::Executor;
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
                Err(error) => encode_failure(resp, &format!("failed to rewrite AOF: {}", error)),
            }
        }

        Command::Info => {
            let mut store = store.write().await;

            let response = format!(
                "memors_version:0.1.0\r\nkeys:{}\r\naof_enabled:1\r\nttl_enabled:1\r\nresp_enabled:1",
                store.key_count()
            );

            if resp {
                encode_bulk_string(&response)
            } else {
                format!("{}\r\n", response)
            }
        }

        Command::FlushAll => {
            {
                let mut store = store.write().await;
                store.flush_all();
            }

            match aof.clear() {
                Ok(_) => encode_ok(resp),
                Err(error) => encode_failure(resp, &format!("failed to clear AOF: {}", error)),
            }
        }

        command => {
            if should_persist(&command) {
                let line = command_to_aof_line(&command);

                if let Err(error) = aof.append(&line) {
                    return encode_failure(resp, &format!("failed to persist command: {}", error));
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
            format!("SET {} {}", key, value)
        }

        Command::SetEx {
            key,
            seconds,
            value,
        } => {
            format!("SETEX {} {} {}", key, seconds, value)
        }

        Command::Del { key } => {
            format!("DEL {}", key)
        }

        Command::Expire { key, seconds } => {
            format!("EXPIRE {} {}", key, seconds)
        }

        _ => String::new(),
    }
}

fn encode_executor_response(result: &str, resp: bool) -> String {
    if !resp {
        return format!("{}\r\n", result);
    }

    match result {
        "OK" => encode_simple_string("OK"),
        "PONG" => encode_simple_string("PONG"),
        "nil" => encode_null(),
        "0" => encode_integer(0),
        "1" => encode_integer(1),
        value => encode_bulk_string(value),
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
        encode_error(&format!("ERR {}", message))
    } else {
        format!("ERROR: {}\r\n", message)
    }
}
