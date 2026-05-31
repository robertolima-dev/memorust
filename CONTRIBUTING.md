# Contributing to Memorust

First off, thank you for taking the time to contribute! 🎉

Memorust is a small, educational Redis clone written in Rust. It is designed to
stay **simple and readable**, so contributions that keep the codebase easy to
learn from are especially welcome.

## Ways to contribute

- 🐛 **Report bugs** — open an issue with steps to reproduce.
- 💡 **Suggest features** — check the Roadmap in the [README](README.md) first.
- 📝 **Improve docs** — typos, clarifications, and examples are all valuable.
- 🔧 **Send code** — pick an open issue or propose a change before large work.

## Getting started

```bash
git clone https://github.com/robertolima-dev/memorust.git
cd memorust
cargo build
cargo run        # starts the server on 127.0.0.1:6379
cargo test       # runs the integration test suite
```

You will need a recent stable Rust toolchain. The crate uses **edition 2024**,
which requires **Rust 1.85 or newer**. Install via [rustup](https://rustup.rs/).

## Development workflow

1. **Fork** the repository and create a topic branch from `main`:
   ```bash
   git checkout -b feat/my-change
   ```
2. Make your change, keeping commits focused and well described.
3. Make sure the checks below pass locally (CI runs the same ones).
4. **Open a pull request** against `main` and describe what and why.

### Required checks

Before pushing, run:

```bash
cargo fmt --all -- --check   # formatting
cargo clippy --all-targets -- -D warnings   # lints (warnings are errors)
cargo test                   # tests
```

The CI workflow (`.github/workflows/ci.yml`) runs these on every push and pull
request. PRs that don't pass cannot be merged.

## How the code is organized

The request flow is the spine of the project. A command travels:

```
server.rs → command.rs → command_handler.rs → executor.rs → store.rs → aof.rs
```

- **`server.rs`** — TCP listener, per-connection tasks, RESP vs. inline routing.
- **`command.rs`** — parses raw input into the `Command` enum.
- **`command_handler.rs`** — async dispatch: locking, AOF persistence, response formatting.
- **`executor.rs`** — the synchronous core that mutates the `Store`.
- **`store.rs`** — the in-memory data + expirations.
- **`aof.rs`** — append-only file persistence and rewrite/compaction.

### Adding a new command

You typically touch three files:

1. `command.rs` — parse the new command into the `Command` enum.
2. `executor.rs` — apply it to the `Store`.
3. `command_handler.rs` — if it mutates state or needs special handling, add its
   AOF serialization (`command_to_aof_line` / `should_persist`) and dispatch.

Add tests under `tests/` for the new behavior.

## Coding guidelines

- Run `cargo fmt` — formatting must match the default rustfmt style.
- Keep `cargo clippy` clean (the CI treats warnings as errors).
- Prefer clear, small functions over cleverness — this is a learning project.
- Add or update integration tests in `tests/` for any behavior change.
- Match the style and naming of the surrounding code.

## Commit messages

Write clear, present-tense messages (e.g. `Add APPEND command`). Reference the
issue number when applicable (`Fixes #12`).

## Code of Conduct

By participating, you agree to abide by our
[Code of Conduct](CODE_OF_CONDUCT.md). Please be respectful and constructive.

## License

By contributing, you agree that your contributions will be dual-licensed under
the [MIT](LICENSE-MIT) and [Apache 2.0](LICENSE-APACHE) licenses, the same terms
that cover the project.
