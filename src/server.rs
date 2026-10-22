use crate::aof::Aof;

use std::sync::Arc;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::RwLock;
use tokio::time::{self, Duration};

use crate::command::Command;
use crate::command_handler::execute_command;
use crate::executor::Executor;
use crate::resp::{RespParseResult, encode_error, parse_resp_frame};
use crate::store::Store;

pub type SharedStore = Arc<RwLock<Store>>;
pub type SharedAof = Arc<Aof>;

/// Longest inline (plain-text) command accepted, newline included.
///
/// RESP frames are bounded by `resp::MAX_FRAME_LEN`; this bounds the other
/// path, so a client can't grow the connection buffer forever by never
/// sending a newline.
const MAX_INLINE_LEN: usize = 64 * 1024;

pub async fn run_server(addr: &str) -> std::io::Result<()> {
    let listener = TcpListener::bind(addr).await?;
    let aof = Aof::new("appendonly.aof")?;

    println!("Memorust TCP server running on {addr}");

    serve(listener, aof, shutdown_signal()).await
}

/// Runs the server on `listener` until `shutdown` completes, then flushes the
/// AOF so every acknowledged write is on disk before returning.
pub async fn serve(
    listener: TcpListener,
    aof: Aof,
    shutdown: impl Future<Output = ()>,
) -> std::io::Result<()> {
    let aof = Arc::new(aof);
    let mut initial_store = Store::new();

    load_aof_into_store(&aof, &mut initial_store)?;

    let store = Arc::new(RwLock::new(initial_store));

    start_background_cleaner(Arc::clone(&store));
    start_aof_flusher(Arc::clone(&aof));

    tokio::pin!(shutdown);

    loop {
        let (stream, socket_addr) = tokio::select! {
            accepted = listener.accept() => accepted?,
            () = &mut shutdown => break,
        };

        let store = Arc::clone(&store);
        let aof = Arc::clone(&aof);

        println!("Client connected: {socket_addr}");

        tokio::spawn(async move {
            if let Err(error) = handle_client(stream, store, aof).await {
                eprintln!("Client error: {error}");
            }
        });
    }

    println!("Shutting down: flushing AOF");

    // Mutating commands append under the store's write lock, so holding it
    // means no command is halfway through: every write already acknowledged
    // to a client is in the buffer. It is held until we return, so nothing new
    // gets in after the final flush.
    let _store = store.write().await;

    let flusher = Arc::clone(&aof);
    tokio::task::spawn_blocking(move || flusher.flush())
        .await
        .map_err(std::io::Error::other)??;

    Ok(())
}

/// Completes on Ctrl+C, or SIGTERM on Unix (what `kill` and most process
/// managers send).
async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(error) = tokio::signal::ctrl_c().await {
            eprintln!("Failed to listen for Ctrl+C: {error}");
            std::future::pending::<()>().await;
        }
    };

    #[cfg(unix)]
    let terminate = async {
        use tokio::signal::unix::{SignalKind, signal};

        match signal(SignalKind::terminate()) {
            Ok(mut terminate) => {
                terminate.recv().await;
            }
            Err(error) => {
                eprintln!("Failed to listen for SIGTERM: {error}");
                std::future::pending::<()>().await;
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {}
        () = terminate => {}
    }
}

pub async fn handle_client(
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
                        // After a framing error we no longer know where the next
                        // command starts: resyncing could run bytes from inside a
                        // value as commands. Close the connection, like Redis does.
                        let response = encode_error(&format!("ERR Protocol error: {error}"));

                        stream.write_all(response.as_bytes()).await?;

                        return Ok(());
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

                    None if connection_buffer.len() > MAX_INLINE_LEN => {
                        let response = "ERROR: Protocol error: inline command too long\r\n";

                        stream.write_all(response.as_bytes()).await?;

                        return Ok(());
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

    let command = match Command::from_args(&parts) {
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

            // `flush` does a write syscall and an fsync: run it on the blocking
            // pool so it never stalls an async worker serving clients.
            let aof = Arc::clone(&aof);

            match tokio::task::spawn_blocking(move || aof.flush()).await {
                Ok(Ok(())) => {}
                Ok(Err(error)) => eprintln!("AOF flush error: {error}"),
                Err(error) => eprintln!("AOF flush task failed: {error}"),
            }
        }
    });
}

pub fn load_aof_into_store(aof: &Aof, store: &mut Store) -> std::io::Result<()> {
    let records = aof.load()?;

    for record in records {
        match Command::from_args(&record) {
            Ok(command) => {
                Executor::execute(store, command);
            }
            Err(error) => {
                eprintln!("Skipping invalid AOF record {record:?}: {error}");
            }
        }
    }

    Ok(())
}
