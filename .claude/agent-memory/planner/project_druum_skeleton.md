---
name: Druum project skeleton decisions
description: Architectural decisions baked into Feature #1 (Bevy 0.18.1 skeleton) that constrain later features
type: project
---

Druum is a Rust/Bevy first-person dungeon crawler. Feature #1 (project skeleton) commits to several decisions that shape Features #2-#7+:

- **Bevy version pinned with `=0.18.1`** (exact-match operator, not `^`). A bump to 0.18.2 or 0.19 must be a deliberate edit. **Why:** research pitfall #1 — accidental `cargo update` to a major version triggers a multi-week migration. **How to apply:** when later features want a Bevy bump for a feature/fix, treat it as its own task with its own research+plan; never let it sneak in via `cargo update`.

- **`dev` feature flag isolates `dynamic_linking`**. `cargo run --features dev` is the fast iteration path; `cargo build --release` must NOT pull `dynamic_linking`. **Why:** dylib-linked binaries fail to launch outside `target/debug/` — research Q2 caveat #2. **How to apply:** never add `bevy/dynamic_linking` to `default` features; never ship a release artifact built with `--features dev`.

- **macOS-first, no `x11` feature.** Linux support is deferred until CI lands. **Why:** `x11` is Linux-only; on macOS Cocoa is used transparently via `bevy_winit`. **How to apply:** when Linux CI is added later, use a target-cfg block (`[target.'cfg(target_os = "linux")'.dependencies]`) to add `x11` — do NOT add it unconditionally.

- **Plugin module layout: 7 flat plugins under `src/plugins/`**: `dungeon`, `combat`, `party`, `town`, `ui`, `audio`, `save`. Each is its own `mod.rs` exporting `Xxx Plugin: Plugin`. **Why:** roadmap-defined separation of concerns; each will own its own systems/resources as features land. **How to apply:** new feature areas extend an existing plugin or get a new sibling under `src/plugins/<name>/`. Do not introduce nested or cross-cutting plugin hierarchies without revisiting this decision.

- **Toolchain pinned to `1.85.0` exact patch in `rust-toolchain.toml`.** **Why:** `channel = "stable"` floats and produces non-reproducible diagnostics across machines and CI; pinning protects against new lints flagging existing code as warnings. **How to apply:** bumping Rust is a deliberate edit, not a `rustup update` side-effect. If a future feature wants edition 2024 features that require Rust >1.85, bump both this file and `Cargo.toml`'s `rust-version` together.

- **`justfile` (not `Makefile`) for task runner.** **Why:** task brief preference; cleaner syntax. **How to apply:** put dev tasks in `justfile`, not `package.json` scripts or shell scripts.

Plan file: `project/plans/20260429-022500-bevy-0-18-1-skeleton-init.md`. Research: `project/research/20260429-021500-bevy-0-18-1-skeleton-init.md`.
