//! Talks to `handle_client` over a real TCP socket.

use std::fs;
use std::sync::Arc;

use memorust::aof::Aof;
use memorust::server::{handle_client, serve};
use memorust::store::Store;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::RwLock;
use tokio::time::{Duration, sleep, timeout};

async fn connect(name: &str) -> (TcpStream, String) {
    let path = std::env::temp_dir().join(format!("memorust_{}_{name}.aof", std::process::id()));
    let path = path.to_str().unwrap().to_string();
    let _ = fs::remove_file(&path);

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let store = Arc::new(RwLock::new(Store::new()));
    let aof = Arc::new(Aof::new(&path).unwrap());

    tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let _ = handle_client(stream, store, aof).await;
    });

    (TcpStream::connect(addr).await.unwrap(), path)
}

async fn read_exactly(stream: &mut TcpStream, expected: &str) {
    let mut buffer = vec![0; expected.len()];

    timeout(Duration::from_secs(2), stream.read_exact(&mut buffer))
        .await
        .expect("timed out waiting for a reply")
        .unwrap();

    assert_eq!(String::from_utf8_lossy(&buffer), expected);
}

#[tokio::test]
async fn should_handle_a_frame_split_across_reads() {
    let (mut stream, path) = connect("fragmented").await;

    let frame = "*3\r\n$3\r\nSET\r\n$3\r\nkey\r\n$6\r\nação\r\n".as_bytes();

    // Split inside the multi-byte "ç" (its 2nd byte, "ão" and CRLF follow).
    let split = frame.len() - 6;
    assert!(std::str::from_utf8(&frame[..split]).is_err());
    stream.write_all(&frame[..split]).await.unwrap();
    sleep(Duration::from_millis(50)).await;
    stream.write_all(&frame[split..]).await.unwrap();

    read_exactly(&mut stream, "+OK\r\n").await;

    stream
        .write_all(b"*2\r\n$3\r\nGET\r\n$3\r\nkey\r\n")
        .await
        .unwrap();

    read_exactly(&mut stream, "$6\r\nação\r\n").await;

    let _ = fs::remove_file(path);
}

#[tokio::test]
async fn should_handle_several_commands_in_one_write() {
    let (mut stream, path) = connect("pipelined").await;

    stream
        .write_all(
            b"*3\r\n$3\r\nSET\r\n$1\r\nk\r\n$4\r\na  b\r\n*2\r\n$3\r\nGET\r\n$1\r\nk\r\n*3\r\n$3\r\nSET\r\n$1\r\ne\r\n$0\r\n\r\n",
        )
        .await
        .unwrap();

    read_exactly(&mut stream, "+OK\r\n$4\r\na  b\r\n+OK\r\n").await;

    let _ = fs::remove_file(path);
}

#[tokio::test]
async fn should_close_the_connection_on_a_protocol_error() {
    let (mut stream, path) = connect("protocol_error").await;

    stream.write_all(b"*1\r\n$1\r\n\xc3\xa9\r\n").await.unwrap();

    read_exactly(&mut stream, "-ERR Protocol error: invalid syntax\r\n").await;

    let mut rest = Vec::new();
    timeout(Duration::from_secs(2), stream.read_to_end(&mut rest))
        .await
        .expect("connection should be closed")
        .unwrap();

    assert!(rest.is_empty());

    let _ = fs::remove_file(path);
}

#[tokio::test]
async fn shutdown_flushes_acknowledged_writes() {
    let path = std::env::temp_dir().join(format!("memorust_{}_shutdown.aof", std::process::id()));
    let path = path.to_str().unwrap().to_string();
    let _ = fs::remove_file(&path);

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let server = tokio::spawn(serve(listener, Aof::new(&path).unwrap(), async {
        let _ = stopped.await;
    }));

    let mut stream = TcpStream::connect(addr).await.unwrap();
    stream.write_all(b"SET name Roberto\n").await.unwrap();
    read_exactly(&mut stream, "OK\r\n").await;

    // The periodic flusher ticked at startup and won't again for ~1s, so the
    // write is still only in the buffer.
    assert_eq!(fs::read(&path).unwrap(), b"");

    stop.send(()).unwrap();
    server.await.unwrap().unwrap();

    let records = Aof::new(&path).unwrap().load().unwrap();
    assert_eq!(records, vec![vec!["SET", "name", "Roberto"]]);

    let _ = fs::remove_file(path);
}
