# Implementation Summary: Party & Character ECS Model — Feature #11

**Date:** 2026-05-04
**Branch:** `ja-feature-11-party-character-ecs-model`
**Plan:** `/Users/nousunio/Repos/Learnings/claude-code/druum/project/plans/20260504-170000-feature-11-party-character-ecs-model.md`

## Steps Completed

All 8 automated steps completed successfully. Step 9 (manual smoke test) is deferred to the user.

| Step | Description | Commit |
|------|-------------|--------|
| 0 | Baseline measurement, branch creation | (no commit) |
| 1 | `ItemAsset` + `ItemStatBlock` stubs in `src/data/items.rs` | `cb08d9c` |
| 2+3 | `ClassTable` schema + 12 character components + `derive_stats` | `4cb1052` |
| 4 | `PartyPlugin` wired with `PartySize` resource + dev-only spawn | `4eb6da0` |
| 5 | `src/data/mod.rs` re-exports for `ClassDef`, `ItemAsset`, `ItemStatBlock` | `0d8bc16` |
| 6 | `assets/classes/core.classes.ron` authored with 3 classes | `d786ba1` |
| 7 | `tests/class_table_loads.rs` Layer 3 integration test | `f2a6924` |
| 8 | Implementation Discoveries + plan marked Complete | `0b2b3ff` |

## Files Changed (by this feature's commits only)

- `src/plugins/party/character.rs` — NEW (600 LOC): 12 components, `PartyMemberBundle`, `derive_stats` pure function, `PartySize` resource, 8 Layer 1 unit tests.
- `src/plugins/party/mod.rs` — REPLACED: `PartyPlugin` wired with resource init, type registration, dev-only spawn system.
- `src/data/classes.rs` — REPLACED: `ClassTable` + `ClassDef` schema (replaces Feature #3 stub).
- `src/data/items.rs` — EXTENDED: `ItemAsset` + `ItemStatBlock` stubs added alongside existing `ItemDb`.
- `src/data/mod.rs` — UPDATED: re-exports for `ClassDef`, `ItemAsset`, `ItemStatBlock`.
- `assets/classes/core.classes.ron` — REPLACED: 3-class RON data (Fighter/Mage/Priest).
- `tests/class_table_loads.rs` — NEW: Layer 3 integration test via `RonAssetPlugin`.

**Byte-unchanged:** `src/main.rs`, `Cargo.toml`, `Cargo.lock` — confirmed via `git diff`.

## Deviations from Plan

### 1. `Handle<T>` does NOT implement `Serialize + Deserialize` in Bevy 0.18

**Impact:** `Equipment` component cannot derive `Serialize + Deserialize`.

The plan stated "Handle<ItemAsset> serializes cleanly as an asset path" (research §Architecture Options). This is incorrect — `bevy_asset-0.18.1`'s `Handle<T>` has no serde impl. `Equipment` is the only component of the 12 that lacks `Serialize + Deserialize`.

**Resolution:** Documented inline in `src/plugins/party/character.rs`. Feature #23 (save/load) must implement custom serde for `Equipment` (serialize each slot as `Option<AssetPath>`, re-resolve handles on load). All other 11 components and `PartySize` carry the full serde set.

### 2. `PartySize` needed `Reflect` derive

The plan's derive set for `PartySize` omitted `Reflect`. `app.register_type::<PartySize>()` requires `GetTypeRegistration` provided by `#[derive(Reflect)]`. Fixed by adding `Reflect` to the derive set.

### 3. `GameState` import scoped inside `#[cfg(feature = "dev")]` block

`use crate::plugins::state::GameState` was only needed in the dev-gated block. Placing it at file level triggered an "unused import" warning in default builds. Fixed by placing it inside the `{ ... }` block.

## Verification Results

| Check | Result |
|-------|--------|
| `cargo check` | PASS — zero warnings |
| `cargo check --features dev` | PASS — zero warnings |
| `cargo clippy --all-targets -- -D warnings` | PASS — zero warnings |
| `cargo clippy --all-targets --features dev -- -D warnings` | PASS — zero warnings |
| `cargo fmt --check` | PASS — exit 0 |
| `cargo test` | PASS — 78 lib + 4 integration (class_table_loads new) |
| `cargo test --features dev` | PASS — 79 lib + 4 integration |
| `cargo test --test class_table_loads` | PASS — 1 test |
| `git diff Cargo.toml` | EMPTY — byte-unchanged |
| `git diff Cargo.lock` | EMPTY — byte-unchanged |
| `git diff src/main.rs` | EMPTY — byte-unchanged |

## Test Count Impact

- Baseline (main-only, Feature #10 unapplied): 67 lib tests (default), 68 (dev)
- Post-#11: 78 lib (default), 79 (dev) — net +11 lib tests
- Integration: 3 → 4 (added `class_table_loads`)

New tests added:
- `src/plugins/party/character.rs::tests`: 8 (derive_stats × 5, PartySize, StatusEffects, BaseStats RON)
- `src/data/classes.rs::tests`: 2 (ClassTable RON round-trip, ClassTable::get)
- `src/data/items.rs::tests`: 1 (ItemStatBlock RON round-trip)
- `tests/class_table_loads.rs`: 1 integration (ClassTable via RonAssetPlugin)

## Manual Smoke Test (Deferred to User — Step 9)

The game requires a graphical environment to run. The user should perform:

1. `cargo run --features dev`
2. Press F9 to advance: TitleScreen → Town → Dungeon. Verify stdout shows `Spawned 4 debug party members`.
3. Press F9 again (Dungeon → Combat) and F9 back into Dungeon. Verify stdout shows `Skipping debug party spawn: 4 party members already exist` (idempotence guard).
4. `cargo run` (no `--features dev`). Cycle to Dungeon via normal game flow. Verify NO spawn log appears (the `#[cfg(feature = "dev")]` gate excludes the system entirely).

## LOC Impact

- New: `src/plugins/party/character.rs` ~600 LOC
- New: `tests/class_table_loads.rs` ~108 LOC
- Replaced: `src/data/classes.rs` ~160 LOC (was 12)
- Extended: `src/data/items.rs` ~90 LOC (was 12)
- Replaced: `src/plugins/party/mod.rs` ~100 LOC (was 10)

## Delta Dependencies: 0

No new entries in `Cargo.toml` or `Cargo.lock`.
