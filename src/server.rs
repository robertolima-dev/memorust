use crate::aof::Aof;

use std::sync::Arc;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::RwLock;
use tokio::time::{self, Duration};

use crate::command::Command;
use crate::command_handler::execute_command;
use crate::executor::Executor;
use crate::resp::{RespParseResult, encode_error, parse_resp, parse_resp_frame};
use crate::store::Store;

pub type SharedStore = Arc<RwLock<Store>>;
// pub type SharedStore = Arc<RwLock<Store>>;
pub type SharedAof = Arc<Aof>;

pub async fn run_server(addr: &str) -> std::io::Result<()> {
    let listener = TcpListener::bind(addr).await?;

    let aof = Arc::new(Aof::new("appendonly.aof")?);
    let mut initial_store = Store::new();

    load_aof_into_store(&aof, &mut initial_store)?;

    let store = Arc::new(RwLock::new(initial_store));

    start_background_cleaner(Arc::clone(&store));
    start_aof_flusher(Arc::clone(&aof));

    println!("Memorust TCP server running on {addr}");

    loop {
        let (stream, socket_addr) = listener.accept().await?;
        let store = Arc::clone(&store);
        let aof = Arc::clone(&aof);

        println!("Client connected: {socket_addr}");

        tokio::spawn(async move {
            if let Err(error) = handle_client(stream, store, aof).await {
                eprintln!("Client error: {error}");
            }
        });
    }
}

async fn handle_client(
    mut stream: TcpStream,
    store: SharedStore,
    aof: SharedAof,
) -> std::io::Result<()> {
    // let mut buffer = [0; 4096];
    let mut read_buffer = [0; 4096];
    let mut connection_buffer: Vec<u8> = Vec::new();

    loop {
        let bytes_read = stream.read(&mut read_buffer).await?;

        if bytes_read == 0 {
            break;
        }

        connection_buffer.extend_from_slice(&read_buffer[..bytes_read]);

        loop {
            if connection_buffer.starts_with(b"*") {
                match parse_resp_frame(&connection_buffer) {
                    RespParseResult::Complete(parts, consumed) => {
                        connection_buffer.drain(..consumed);

                        let response = handle_resp_parts(parts, &store, &aof).await;

                        stream.write_all(response.as_bytes()).await?;
                    }

                    RespParseResult::Incomplete => {
                        break;
                    }

                    RespParseResult::Error(error) => {
                        let response = encode_error(&format!("ERR {error}"));

                        connection_buffer.clear();

                        stream.write_all(response.as_bytes()).await?;

                        break;
                    }
                }
            } else {
                match connection_buffer.iter().position(|&byte| byte == b'\n') {
                    Some(newline_index) => {
                        let line: Vec<u8> = connection_buffer.drain(..=newline_index).collect();
                        let input = String::from_utf8_lossy(&line);
                        let trimmed = input.trim();

                        if !trimmed.is_empty() {
                            let response = handle_plain_text_command(trimmed, &store, &aof).await;

                            stream.write_all(response.as_bytes()).await?;
                        }
                    }

                    None => break,
                }
            }
        }
    }

    Ok(())
}

async fn handle_resp_parts(parts: Vec<String>, store: &SharedStore, aof: &SharedAof) -> String {
    if parts.is_empty() {
        return encode_error("ERR invalid command");
    }

    let raw_command = parts.join(" ");

    let command = match Command::parse(&raw_command) {
        Ok(command) => command,
        Err(error) => return encode_error(&format!("ERR {error}")),
    };

    execute_command(command, store, aof, true).await
}

async fn handle_plain_text_command(input: &str, store: &SharedStore, aof: &SharedAof) -> String {
    if input.eq_ignore_ascii_case("EXIT") || input.eq_ignore_ascii_case("QUIT") {
        return "Bye!\r\n".to_string();
    }

    match Command::parse(input) {
        Ok(command) => execute_command(command, store, aof, false).await,
        Err(error) => format!("ERROR: {error}\r\n"),
    }
}

#[allow(dead_code)]
async fn handle_resp_command(input: &str, store: &SharedStore, aof: &SharedAof) -> String {
    let parts: Vec<String> = match parse_resp(input) {
        Ok(parts) => parts,
        Err(error) => return encode_error(&format!("Err {error}")),
    };

    handle_resp_parts(parts, store, aof).await
}

fn start_background_cleaner(store: SharedStore) {
    tokio::spawn(async move {
        let mut interval = time::interval(Duration::from_secs(5));

        loop {
            interval.tick().await;

            let mut store = store.write().await;
            let removed = store.cleanup_expired_keys();

            if removed > 0 {
                println!("Cleaner removed {removed} expired keys");
            }
        }
    });
}

fn start_aof_flusher(aof: SharedAof) {
    tokio::spawn(async move {
        let mut interval = time::interval(Duration::from_secs(1));

        loop {
            interval.tick().await;

            if let Err(error) = aof.flush() {
                eprintln!("AOF flush error: {error}");
            }
        }
    });
}

fn load_aof_into_store(aof: &Aof, store: &mut Store) -> std::io::Result<()> {
    let commands = aof.load()?;

    for line in commands {
        match Command::parse(&line) {
            Ok(command) => {
                Executor::execute(store, command);
            }
            Err(error) => {
                eprintln!("Skipping invalid AOF line '{line}': {error}");
            }
        }
    }

    Ok(())
}
