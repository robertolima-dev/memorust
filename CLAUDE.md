# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

Memorust is a small, educational Redis clone: a single-binary, in-memory key-value server written
in Rust (edition 2024) with Tokio as its only dependency. It is also exposed as a library crate
(`src/lib.rs`) so the modules can be unit/integration tested directly. The codebase is
intentionally simple and flat — everything lives in `src/*.rs`.

## Commands

```bash
cargo run                 # build + start the server on 127.0.0.1:6379
cargo build --release     # optimized build
cargo test                # run the integration tests in tests/
cargo test should_parse   # run a subset of tests by name substring
cargo fmt
cargo clippy
```

Interact with a running server using a Redis client or raw TCP:

```bash
redis-cli -p 6379         # then: PING / SET name Roberto / GET name / INFO
# or inline over telnet/nc, one command per line: SET name Roberto
```

Supported commands (authoritative list in `src/command.rs`):
`PING SET GET DEL EXISTS SETEX EXPIRE TTL INFO FLUSHALL AOFREWRITE`.

## Request flow / architecture

A command flows through these modules in order — this is the spine of the codebase:

1. **`server.rs`** — `run_server` binds the listener and calls `serve`, which loads the AOF into
   a fresh `Store`, spawns the background expiry cleaner and AOF flusher, then `tokio::spawn`s one
   `handle_client` task per connection until the shutdown signal (Ctrl+C / SIGTERM), when it
   takes the store write lock and does a final AOF flush. `handle_client` accumulates bytes and routes on the first byte:
   - buffer starts with `*` → **RESP array** path via `resp::parse_resp_frame` (length-prefixed,
     handles partial reads via `Incomplete`); calls `execute_command(..., resp: true)`.
   - otherwise → **inline text** path (a line terminated by `\n`); calls
     `execute_command(..., resp: false)`. Also handles `EXIT`/`QUIT`.
2. **`command.rs`** — `Command::from_args(&[String])` maps already-split arguments to the
   `Command` enum, returning a `MemorustError` for unknown/malformed input. The RESP path and AOF
   replay call it directly; `Command::parse(&str)` (inline text) tokenizes first, then calls it.
   New command **parsing** goes here.
3. **`command_handler.rs`** — `execute_command` is the async dispatcher. It handles the commands
   that need I/O or special locking (`AofRewrite`, `Info`, `FlushAll`) inline. For mutating
   commands it takes the store write lock, appends the AOF records (`aof_records`), then delegates
   the actual store mutation to `Executor::execute` — both under the same lock. The `resp: bool` parameter selects
   the response format: real RESP encodings (`+OK\r\n`, `$-1\r\n`, `:1\r\n`, bulk strings) when
   true, plain `…\r\n` text when false.
4. **`executor.rs`** — `Executor::execute(&mut Store, Command) -> String` is the **synchronous core**
   that performs the data-structure operations and returns a small sentinel string (`"OK"`,
   `"PONG"`, `"NIL"`, `"0"`/`"1"`, a value, a TTL number). It is called both by `execute_command`
   *and* directly by AOF replay (`load_aof_into_store`). It is live code, not a duplicate.
5. **`store.rs`** — `Store` holds two `HashMap`s (`data` + `expirations`). It is **not internally
   synchronized**; the server wraps it in `Arc<RwLock<Store>>` (`SharedStore`). Expiry is lazy
   (checked on `get`/`ttl`) plus a periodic `cleanup_expired_keys` sweep run every 5s by the
   background task.
6. **`aof.rs`** — `Aof` appends each mutating command as a RESP array (explicit lengths, so any
   value round-trips); `start_rewrite` + `complete_rewrite` compact the log to a `SET` + `EXPIREAT`
   snapshot (temp file + atomic rename) without holding the store lock while writing, keeping
   records appended meanwhile in `pending`; `load` reads it back (and still accepts the old one-command-per-line format). On startup the
   server replays the AOF. Default file: `appendonly.aof` (repo root).

Supporting modules: `resp.rs` (RESP parsing + `encode_*` helpers), `tokenizer.rs` (quote-aware
whitespace splitter used by `Command::parse`), `error.rs` (`MemorustError`).

Concurrency model: many connection tasks share one `Arc<RwLock<Store>>` and one `Arc<Aof>`; each
command takes a short `write()` lock. Mutating commands append to the AOF *inside* that lock, so
the AOF order always matches the order mutations hit memory; `AOFREWRITE` and `FLUSHALL` rely on
this (they discard the AOF's unflushed buffer instead of writing it). Blocking file work —
the periodic `flush`/fsync and `complete_rewrite` — runs on `spawn_blocking`, never on an async
worker; `start_rewrite` + `Store::entries` run together under the store lock, and a `FLUSHALL`
during a rewrite aborts it.

## Gotchas

- **Two-layer command handling.** `command_handler::execute_command` owns async concerns
  (locking, AOF persistence, RESP-vs-text formatting) and special commands; `executor.rs` owns the
  pure store mutation. When adding a command, you typically touch `command.rs` (parse),
  `executor.rs` (apply), and — if it mutates or needs special handling — `command_handler.rs`
  (persistence/dispatch). `aof_records` matches every variant on purpose: decide there whether the
  new command is persisted. Store relative TTLs as absolute `EXPIREAT`.
- **Sentinel-string coupling.** `executor.rs` returns magic strings that
  `command_handler::encode_executor_response` pattern-matches to pick a RESP type. Returning a new
  shape from the executor without updating that match will mis-encode the reply.
- **`resp` flag dual protocol.** The same handler serves both RESP clients (redis-cli) and raw
  inline text; respect the `resp` bool when adding responses.
- `parse_resp_frame` works on bytes and enforces `MAX_FRAME_LEN` / `MAX_ARRAY_LEN`; on a framing
  error the server closes the connection (it can't tell where the next command starts).
- `appendonly.aof` is committed and is rewritten at runtime, so it routinely shows up as modified
  in `git status` after running the server (it is also listed in `.gitignore`).
- Tests are integration tests in `tests/` (`command_tests`, `store_tests`, `resp_tests`,
  `tokenizer_tests`, `aof_tests`, `executor_tests`, plus `persistence_tests` — handler + replay —
  and `server_tests` — real TCP), exercising the library crate's public API.
