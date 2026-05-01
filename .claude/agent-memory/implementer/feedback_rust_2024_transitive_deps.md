---
name: Rust 2024 edition — transitive deps not usable without explicit declaration
description: In this project (edition="2024"), crates that are only transitive deps (e.g. serde, ron via Bevy) cannot be referenced directly in source without being declared in Cargo.toml
type: feedback
---

In Rust 2024 edition (which this project uses — `edition = "2024"` in Cargo.toml), you cannot use a crate that is only transitively present without declaring it explicitly in `[dependencies]`.

Example failure: `use serde::{Deserialize, Serialize}` produces `error[E0432]: unresolved import 'serde'` even though serde is a transitive dep via Bevy.

**Why:** Rust 2024 edition removed automatic extern crate injection for transitive dependencies. Prior editions allowed `extern crate foo;` to find transitive deps; 2024 edition restricts the compiler graph walk.

**How to apply:** Any plan that says "X is transitively present via Bevy, do not add it explicitly" will fail for this project. Add `serde`, `ron`, or any other crate you directly reference in source code as an explicit `[dependencies]` entry. Match the version to what Bevy already pulls in to avoid unnecessary duplicate versions in `Cargo.lock`. Examples:
- `serde = { version = "1", features = ["derive"] }`
- `ron = "0.12"`
