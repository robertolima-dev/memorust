# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

Memors is a small, educational Redis clone: a single-binary, in-memory key-value server written
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

1. **`server.rs`** — `run_server` binds the listener, loads the AOF into a fresh `Store`,
   spawns the background expiry cleaner, then `tokio::spawn`s one `handle_client` task per
   connection. `handle_client` accumulates bytes and routes on the first byte:
   - buffer starts with `*` → **RESP array** path via `resp::parse_resp_frame` (length-prefixed,
     handles partial reads via `Incomplete`); calls `execute_command(..., resp: true)`.
   - otherwise → **inline text** path (a line terminated by `\n`); calls
     `execute_command(..., resp: false)`. Also handles `EXIT`/`QUIT`.
2. **`command.rs`** — `Command::parse(&str)` tokenizes then maps to the `Command` enum, returning
   a `MemorsError` for unknown/malformed input. New command **parsing** goes here.
3. **`command_handler.rs`** — `execute_command` is the async dispatcher. It handles the commands
   that need I/O or special locking (`AofRewrite`, `Info`, `FlushAll`) inline, persists mutating
   commands to the AOF *before* applying them (`should_persist` / `command_to_aof_line`), then
   delegates the actual store mutation to `Executor::execute`. The `resp: bool` parameter selects
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
6. **`aof.rs`** — `Aof` appends each mutating command as a plaintext line; `rewrite` compacts the
   log to a `SET`-only snapshot (temp file + atomic rename); `load` reads it back. On startup the
   server replays the AOF. Default file: `appendonly.aof` (repo root).

Supporting modules: `resp.rs` (RESP parsing + `encode_*` helpers), `tokenizer.rs` (quote-aware
whitespace splitter used by `Command::parse`), `error.rs` (`MemorsError`).

Concurrency model: many connection tasks share one `Arc<RwLock<Store>>` and one `Arc<Aof>`; each
command takes a short `write()` lock. AOF writes happen outside the store lock, before mutation.

## Gotchas

- **Two-layer command handling.** `command_handler::execute_command` owns async concerns
  (locking, AOF persistence, RESP-vs-text formatting) and special commands; `executor.rs` owns the
  pure store mutation. When adding a command, you typically touch `command.rs` (parse),
  `executor.rs` (apply), and — if it mutates or needs special handling — `command_handler.rs`
  (persistence/dispatch). Add its AOF serialization to `command_to_aof_line` and `should_persist`
  if it must survive restarts.
- **Sentinel-string coupling.** `executor.rs` returns magic strings that
  `command_handler::encode_executor_response` pattern-matches to pick a RESP type. Returning a new
  shape from the executor without updating that match will mis-encode the reply.
- **`resp` flag dual protocol.** The same handler serves both RESP clients (redis-cli) and raw
  inline text; respect the `resp` bool when adding responses.
- `parse_resp` (the line-based parser in `resp.rs`) is only reached by the `#[allow(dead_code)]`
  `handle_resp_command`; the live RESP path uses `parse_resp_frame`. Both are covered by tests.
- `appendonly.aof` is committed and is rewritten at runtime, so it routinely shows up as modified
  in `git status` after running the server (it is also listed in `.gitignore`).
- Tests are integration tests in `tests/` (`command_tests`, `store_tests`, `resp_tests`,
  `tokenizer_tests`, `aof_tests`, `executor_tests`), exercising the library crate's public API.
