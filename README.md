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
- Append Only File (AOF)
- AOF replay on startup
- AOF rewrite (compaction)
- INFO command
- FLUSHALL command
- Concurrent client support

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
redis-benchmark \
    -p 6379 \
    -t set,get \
    -n 100000
```

Current results (Mac M-series development machine):

| Operation | Throughput |
|------------|------------|
| SET | ~75k ops/sec |
| GET | ~150k ops/sec |

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