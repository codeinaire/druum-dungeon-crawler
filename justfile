# Druum dev tasks. Requires `just` (https://just.systems/).

# Default recipe: list available tasks.
default:
    @just --list

# Type-check without producing a binary. Fastest feedback loop.
check:
    cargo check

# Lint with all warnings promoted to errors.
clippy:
    cargo clippy -- -D warnings

# Run with dynamic linking enabled — fast incremental rebuilds (~5-8s).
# NEVER use this flag for a release build (see Cargo.toml comment on `dev`).
run-dev:
    cargo run --features dev

# Release build (no dynamic linking, no `dev` feature).
build-release:
    cargo build --release
