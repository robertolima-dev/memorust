# Memors

A lightweight in-memory database written in Rust.

Memors was created as a learning project to explore systems programming, networking, concurrency, storage engines, and database internals using Rust.

The project is heavily inspired by Redis and aims to evolve incrementally while keeping the codebase simple and educational.

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

Memors currently uses an Append Only File (AOF).

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

MIT