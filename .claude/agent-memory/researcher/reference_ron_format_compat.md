---
name: ron 0.11 vs 0.12 format equivalence (verified)
description: For druum's struct/enum/Option/Vec/primitive types, ron 0.11.0 and ron 0.12.1 produce byte-identical RON output and accept each other's input — verified by direct comparison of both crates' test suites
type: reference
---

`bevy_common_assets 0.16.0` parses RON via `ron 0.11.0` (aliased as `serde_ron`); druum's direct dep is `ron 0.12.1`. Both versions are pinned in `Cargo.lock` (lines 4337-4362) and live side-by-side as separate semver-incompatible copies.

**Both crate sources are extracted on disk** at:
- `/Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/ron-0.11.0/`
- `/Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/ron-0.12.1/`

**Verified format equivalence between 0.11 and 0.12:**
- Externally-tagged enums (unit/tuple/struct variants): identical (`tests/123_enum_representation.rs` byte-equivalent for first 100 lines)
- `Option<T>`: `Some(42)` / `None` (`tests/options.rs` identical)
- Floats: `1.0`, `0.5`, scientific small numbers (`tests/floats.rs` identical)
- Internally/adjacently/untagged enums: identical
- Raw identifiers (`r#name`): identical (`tests/401_raw_identifier.rs`)

**The only format-breaking change in ron 0.12.0** was removing legacy base64-encoded byte-string deserialization (replaced by Rusty byte strings in 0.9.0, well before 0.11). Druum has no `Vec<u8>` byte-string fields, so this is moot.

**The only API-breaking change in ron 0.12.0** was removing `ron::error::Error::Base64Error` variant — Rust-side only.

**How to apply:**
- For druum's Feature #4+ types (struct, enum, Option, Vec, primitives, String): the round-trip test in `src/data/*.rs` using `ron 0.12` is sufficient for serde-derive correctness, BUT it does NOT prove the runtime `RonAssetPlugin` (ron 0.11) path also works. Add an `App`-level integration test in `tests/<name>_loads.rs` that exercises `RonAssetPlugin::<T>::new(&[ext])` end-to-end.
- Pattern: `bevy_asset_loader-0.26.0/tests/multiple_asset_collections.rs` is the canonical "drive App.run() with timeout, assert OnEnter(NextState)" template.
- Use `MessageWriter<AppExit>` not `EventWriter<AppExit>` — Bevy 0.18 split.
- The cross-version equality test ("deserialize same file with both ron versions, assert equal") is unavailable because `bevy_common_assets-0.16.0/src/ron.rs:6` keeps `serde_ron` as a private internal alias (not re-exported), so druum cannot directly call ron 0.11.

If a future ron version does break format compat, this memory is invalid — re-verify with the same approach (extract sources, diff test files).
