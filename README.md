# Memorust

A lightweight in-memory database written in Rust.

Memorust was created as a learning project to explore systems programming, networking, concurrency, storage engines, and database internals using Rust.

The project is heavily inspired by Redis and aims to evolve incrementally while keeping the codebase simple and educational.

---

## What is it?

Memorust is an educational Redis clone: a single-binary, in-memory key-value
database server written in Rust (edition 2024), with Tokio as its only
dependency. It speaks the RESP protocol, so it is compatible with `redis-cli`
and existing Redis clients, and supports a subset of Redis commands (`PING`,
`SET`, `GET`, `DEL`, `EXISTS`, `SETEX`, `EXPIRE`, `TTL`, `INFO`, `FLUSHALL`,
`AOFREWRITE`) with TTL and Append Only File (AOF) persistence.

## Why does it exist?

Memorust started as a study project to learn, hands-on, how an in-memory
database works under the hood: async TCP networking, protocol design,
concurrency, storage engines, persistence, and performance engineering. The
focus is on keeping the code **simple and readable**, serving as learning
material rather than aiming for full feature parity with Redis.

## How to run it?

You need a recent stable Rust toolchain (the crate uses **edition 2024**, which
requires **Rust 1.85 or newer**). Install it via [rustup](https://rustup.rs/).

```bash
git clone https://github.com/robertolima-dev/memorust.git
cd memorust
cargo run        # builds and starts the server on 127.0.0.1:6379
```

In another terminal, interact with it using `redis-cli` or raw TCP:

```bash
redis-cli -p 6379
# then: PING / SET name Roberto / GET name / INFO
```

To run the tests:

```bash
cargo test
```

## How to contribute?

Contributions are very welcome! See the contribution guide in
[CONTRIBUTING.md](CONTRIBUTING.md) for the development workflow, the required
checks, and how to open a pull request. By participating in this project, you
agree to abide by our [Code of Conduct](CODE_OF_CONDUCT.md).

---

## Features

Current implementation:

- TCP Server (Tokio)
- RESP Protocol support
- Redis CLI compatibility
- In-memory key-value storage
- TTL support
- Background expiration cleaner
- Append Only File (AOF) with a buffered writer
- Periodic `fsync` (≈1s, `appendfsync everysec` style)
- AOF replay on startup
- AOF rewrite (compaction)
- INFO command
- FLUSHALL command
- Concurrent client support
- Concurrent reads under a shared lock (GET/EXISTS/TTL/PING)

---

## Supported Commands

### Connection

```text
PING
```

Response:

```text
PONG
```

---

### Key/Value

```text
SET key value
GET key
DEL key
EXISTS key
```

Example:

```text
SET name Roberto
GET name
DEL name
```

---

### Expiration

```text
SETEX key seconds value
EXPIRE key seconds
TTL key
```

Example:

```text
SETEX session 60 abc123
TTL session
```

---

### Administration

```text
INFO
FLUSHALL
AOFREWRITE
```

Example:

```text
INFO
FLUSHALL
AOFREWRITE
```

---

## Architecture

Current architecture:

```text
Client
  ↓
TCP Server (Tokio)
  ↓
RESP Parser
  ↓
Command Parser
  ↓
Executor
  ↓
Store
  ↓
AOF
```

---

## Persistence

Memorust currently uses an Append Only File (AOF).

Every mutating command is persisted:

```text
SET
DEL
SETEX
EXPIRE
```

Example:

```text
SET name Roberto
SET city SãoPaulo
DEL city
```

The AOF is replayed automatically during startup.

### Durability model

Mutating commands are appended to a buffered writer instead of opening and
flushing the file on every write. A background task flushes the buffer and
`fsync`s it to disk roughly once per second (similar to Redis'
`appendfsync everysec`).

This keeps the `fsync` off the per-command hot path — and the `fsync` itself
runs outside the writer lock, so it does not stall in-flight writes. The
trade-off is that up to ~1 second of recent writes can be lost on a crash.

---

## Running

### Start server

```bash
cargo run
```

Default address:

```text
127.0.0.1:6379
```

---

## Using Redis CLI

```bash
redis-cli -p 6379
```

Examples:

```text
PING

SET name Roberto

GET name

INFO
```

---

## Running Tests

```bash
cargo test
```

---

## Benchmark

Example benchmark:

```bash
# without pipelining
redis-benchmark -p 6379 -t set,get -n 100000

# with pipelining (reveals server throughput rather than round-trip latency)
redis-benchmark -p 6379 -t set,get -n 300000 -P 16
```

Current results (release build, Mac M-series development machine):

| Operation | No pipeline (`-P 1`) | Pipelined (`-P 16`) |
|-----------|----------------------|---------------------|
| SET       | ~155k ops/sec        | ~264k ops/sec       |
| GET       | ~158k ops/sec        | ~1.27M ops/sec      |

Without pipelining the benchmark is dominated by network round-trips, so it
mostly measures latency. Under pipelining the server's own ceiling shows: GET
scales far past SET because reads run concurrently under a shared lock, while
writes still serialize on the exclusive lock.

> Earlier versions reported ~75k SET / ~150k GET. The SET gain comes from the
> buffered AOF writer; the GET gain comes from serving reads under a shared lock.

Results may vary depending on hardware and implementation version.

---

## Roadmap

### Completed

- [x] TCP Server
- [x] RESP Protocol
- [x] Redis CLI Compatibility
- [x] TTL
- [x] Background Cleaner
- [x] AOF
- [x] AOF Replay
- [x] AOF Rewrite
- [x] INFO
- [x] FLUSHALL

### Next Steps

- [x] Buffered AOF Writer (`appendfsync everysec`)
- [ ] Async AOF Writer
- [ ] Value Types
  - [ ] Integer
  - [ ] JSON
  - [ ] List
- [ ] Config.toml
- [ ] Memory Eviction Policies
- [ ] Binary Snapshot (.db)
- [ ] Replication
- [ ] Cluster Mode

---

## Learning Goals

This project explores:

- Rust
- Tokio
- TCP Networking
- Protocol Design
- Concurrency
- Storage Engines
- Database Internals
- Persistence
- Performance Engineering

---

## License

Licensed under either of

- MIT license ([LICENSE-MIT](LICENSE-MIT))
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this project by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.