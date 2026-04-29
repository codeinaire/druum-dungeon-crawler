# Druum

First-person grid-based dungeon crawler RPG, in the spirit of Wizardry, Etrian
Odyssey, and Legend of Grimrock. Built with Rust + [Bevy](https://bevy.org)
`0.18.1`.

## Status

Feature #1: Project skeleton & plugin architecture. Empty plugin stubs only.

## Development

Requires Rust 1.85+ (pinned via `rust-toolchain.toml`) and optionally
[`just`](https://just.systems/) for the task runner.

```sh
# Fast iteration (Bevy as dylib, ~5-8s incremental rebuilds):
cargo run --features dev
# or: just run-dev

# Type-check:
cargo check
# or: just check

# Lint:
cargo clippy -- -D warnings
# or: just clippy

# Release build (no dynamic linking):
cargo build --release
```

See `project/roadmaps/` for the full feature plan.
