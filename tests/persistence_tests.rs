//! End-to-end persistence: commands go through `execute_command` (locking +
//! AOF), then the AOF is replayed into a fresh store and compared.

use std::fs;
use std::sync::Arc;

use memorust::aof::Aof;
use memorust::command::Command;
use memorust::command_handler::execute_command;
use memorust::server::{SharedAof, SharedStore, load_aof_into_store};
use memorust::store::Store;
use tokio::sync::RwLock;

struct Server {
    path: String,
    store: SharedStore,
    aof: SharedAof,
}

impl Server {
    fn start(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("memorust_{}_{name}.aof", std::process::id()));
        let path = path.to_str().unwrap().to_string();

        let _ = fs::remove_file(&path);

        Self {
            store: Arc::new(RwLock::new(Store::new())),
            aof: Arc::new(Aof::new(&path).unwrap()),
            path,
        }
    }

    async fn run(&self, args: &[&str]) -> String {
        let args: Vec<String> = args.iter().map(|arg| arg.to_string()).collect();
        let command = Command::from_args(&args).unwrap();

        execute_command(command, &self.store, &self.aof, false).await
    }

    /// Simulates a restart: flush what is buffered and replay the file.
    fn restart(&self) -> Store {
        self.aof.flush().unwrap();

        let mut store = Store::new();
        load_aof_into_store(&Aof::new(&self.path).unwrap(), &mut store).unwrap();

        store
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

#[tokio::test]
async fn values_survive_a_restart_exactly() {
    let server = Server::start("exact_values");

    server.run(&["SET", "json", r#"{"x":1}"#]).await;
    server.run(&["SET", "spaces", "a  b"]).await;
    server.run(&["SET", "empty", ""]).await;
    server.run(&["SET", "evil", "x\nSET other value"]).await;

    let store = server.restart();

    assert_eq!(store.get("json"), Some(&r#"{"x":1}"#.to_string()));
    assert_eq!(store.get("spaces"), Some(&"a  b".to_string()));
    assert_eq!(store.get("empty"), Some(&String::new()));
    assert_eq!(store.get("evil"), Some(&"x\nSET other value".to_string()));
    assert_eq!(store.get("other"), None);
}

#[tokio::test]
async fn flushall_before_the_periodic_flush_stays_empty() {
    let server = Server::start("flushall");

    server.run(&["SET", "gone", "1"]).await;
    server.run(&["FLUSHALL"]).await;
    server.run(&["SET", "kept", "2"]).await;

    let store = server.restart();

    assert_eq!(store.get("gone"), None);
    assert_eq!(store.get("kept"), Some(&"2".to_string()));
    assert_eq!(store.len(), 1);
}

#[tokio::test]
async fn ttls_survive_a_rewrite_and_a_restart() {
    let server = Server::start("rewrite_ttl");

    server.run(&["SETEX", "session", "3600", "abc"]).await;
    server.run(&["SET", "plain", "v"]).await;
    server.run(&["AOFREWRITE"]).await;

    let store = server.restart();

    assert_eq!(store.get("session"), Some(&"abc".to_string()));
    assert!((3590..=3600).contains(&store.ttl("session")));
    assert_eq!(store.ttl("plain"), -1);
}

#[tokio::test]
async fn expireat_is_persisted() {
    let server = Server::start("expireat");

    let expires_at = Store::expiration_unix_ms_from_seconds(3600).to_string();

    server.run(&["SET", "token", "abc"]).await;
    server.run(&["EXPIREAT", "token", &expires_at]).await;

    let store = server.restart();

    assert!((3590..=3600).contains(&store.ttl("token")));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_writes_replay_to_the_same_state() {
    let server = Arc::new(Server::start("concurrent"));

    let mut tasks = Vec::new();

    for writer in 0..8 {
        let server = Arc::clone(&server);

        tasks.push(tokio::spawn(async move {
            for round in 0..200 {
                let key = format!("key{}", round % 5);
                let value = format!("{writer}-{round}");

                match round % 7 {
                    0 => server.run(&["DEL", &key]).await,
                    1 => server.run(&["SETEX", &key, "3600", &value]).await,
                    _ => server.run(&["SET", &key, &value]).await,
                };

                if writer == 0 && round == 100 {
                    server.run(&["AOFREWRITE"]).await;
                }
            }
        }));
    }

    for task in tasks {
        task.await.unwrap();
    }

    let replayed = server.restart();
    let live = server.store.read().await;

    for index in 0..5 {
        let key = format!("key{index}");

        assert_eq!(replayed.get(&key), live.get(&key), "value of {key}");
        assert_eq!(
            replayed.ttl(&key) == -1,
            live.ttl(&key) == -1,
            "TTL presence of {key}"
        );
    }
}

/// A write that is waiting for the store lock while a rewrite or FLUSHALL
/// holds it must be persisted *after* them, in the same order it is applied.
#[tokio::test]
async fn a_write_waiting_on_the_lock_is_ordered_after_rewrite_and_flushall() {
    for flush_all in [false, true] {
        let server = Arc::new(Server::start(&format!("waiting_write_{flush_all}")));

        let mut store = server.store.write().await;

        let writer = {
            let server = Arc::clone(&server);
            tokio::spawn(async move { server.run(&["SET", "late", "1"]).await })
        };

        // Let the writer run up to the lock.
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // What AOFREWRITE / FLUSHALL do while holding the lock.
        if flush_all {
            server.aof.clear().unwrap();
            store.flush_all();
        } else {
            server.aof.start_rewrite().unwrap();
            server.aof.complete_rewrite(store.entries()).unwrap();
        }

        drop(store);
        writer.await.unwrap();

        let replayed = server.restart();

        assert_eq!(
            server.store.read().await.get("late"),
            Some(&"1".to_string())
        );
        assert_eq!(
            replayed.get("late"),
            Some(&"1".to_string()),
            "flush_all={flush_all}"
        );
    }
}
