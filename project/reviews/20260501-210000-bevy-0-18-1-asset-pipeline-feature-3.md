# Code Review: Bevy 0.18.1 Asset Pipeline & RON Loading (Feature #3)

**Date:** 2026-05-01
**PR:** https://github.com/codeinaire/druum-dungeon-crawler/pull/3
**Verdict:** APPROVE
**Reviewed by:** Code Reviewer agent

## Files Reviewed (Full Coverage)

- `Cargo.toml` — full
- `src/main.rs` — full
- `src/plugins/loading/mod.rs` — full
- `src/plugins/mod.rs` — full
- `src/data/mod.rs` — full
- `src/data/dungeon.rs` — full
- `src/data/items.rs`, `enemies.rs`, `classes.rs`, `spells.rs` — full
- `assets/README.md` — full
- `assets/dungeons/floor_01.dungeon.ron`, `items/core.items.ron`, `enemies/core.enemies.ron`, `classes/core.classes.ron`, `spells/core.spells.ron` — full

## Behavioral Delta

`GameState::Loading` now auto-advances to `GameState::TitleScreen` once five stub RON assets (all `()`) are resolved by `bevy_asset_loader::LoadingState`. A Camera2d + "Loading..." text node spawn on `OnEnter(Loading)` and despawn on `OnExit(Loading)` via a shared `LoadingScreenRoot` marker. Two crates pinned: `bevy_common_assets =0.16.0`, `bevy_asset_loader =0.26.0`. `bevy/file_watcher` added to the `dev` feature composition; `AssetPlugin { watch_for_changes_override: Some(cfg!(feature = "dev")), ..default() }` wired in `main.rs`. No changes to the state machine itself.

## Static Analysis Results

All five symmetric verification commands pass with zero warnings:

| Command | Result |
|---------|--------|
| `cargo check` | PASS |
| `cargo check --features dev` | PASS |
| `cargo clippy --all-targets -- -D warnings` | PASS |
| `cargo clippy --all-targets --features dev -- -D warnings` | PASS |
| `cargo test` | PASS (2 tests) |
| `cargo test --features dev` | PASS (3 tests) |

## Severity Counts

| Severity | Count |
|----------|-------|
| CRITICAL | 0 |
| HIGH | 0 |
| MEDIUM | 0 |
| LOW | 2 |

## Findings

### [LOW] PR description and test plan say "Loading -> Town" — code correctly targets TitleScreen

**File:** PR description (not source code)

**Issue:** Two bullet points in the PR body say "transitions to Town once RON assets resolve" and the test plan similarly says "transitions to Town". The implementation correctly targets `GameState::TitleScreen` (confirmed at `src/plugins/loading/mod.rs:74`). This is a copy-paste error in the description only — the code is correct.

**Fix:** Update the PR description's Summary and Test Plan bullets to say "TitleScreen" instead of "Town".

---

### [LOW] `ron` and `serde` not pinned with `=` unlike other Cargo deps in this project

**File:** `Cargo.toml:26-27`

**Issue:** The project pins all versioned deps with `=` (Bevy at `=0.18.1`, both NiklasEi crates at `=0.16.0` and `=0.26.0`). The deviation-driven `serde = { version = "1", features = ["derive"] }` and `ron = "0.12"` use semver-compatible ranges. For the Rust 2024 edition workaround use-case these resolve to stable, widely-used versions with no practical churn risk, so this is LOW severity rather than HIGH. If a future `serde 2.0` or `ron 0.13` drops, the lockfile pin will hold anyway. Worth making consistent if the project convention is applied uniformly.

**Fix:** Pin both with `=` matching the lockfile-resolved versions:

```toml
serde = { version = "=1.0.228", features = ["derive"] }
ron   = "=0.12.1"
```

(Run `cargo update --dry-run` first to confirm the current lockfile pins.)

---

## Implementation Quality Notes (informational, not findings)

These are observations worth carrying forward — all are correct as implemented, no action required for this PR.

**`bevy_common_assets 0.16.0` uses `ron 0.11.0` internally, project uses `ron 0.12.1`.** Cargo carries both as separate semver-incompatible copies in the lockfile. The round-trip test in `src/data/dungeon.rs` exercises `ron 0.12` (our explicit dep), while `bevy_common_assets`' loader uses `ron 0.11` internally. For empty struct bodies (`()`), the format is identical across both versions. Feature #4, which adds real fields, should verify that the RON `0.11` serialization output from `bevy_common_assets`' loader is compatible with the test's `ron 0.12` assertions — or update the test to deserialize with the same version the loader uses. The implementation note documents this correctly.

**`bevy_asset_loader 0.26.0` does not declare `bevy` as a direct dep in the lockfile.** It depends on `bevy_app`, `bevy_asset`, `bevy_ecs`, `bevy_log`, `bevy_platform`, `bevy_reflect`, `bevy_state`, `bevy_utils` directly — all at `0.18.1`. This confirms Bevy 0.18 compatibility was correctly verified in Step 1.

**All critical plan invariants verified:**
- `DungeonAssets` derives both `AssetCollection` AND `Resource` — correct
- `RonAssetPlugin` registrations come before `add_loading_state` in `Plugin::build` — correct
- No `next.set(GameState::TitleScreen)` called from anywhere in `LoadingPlugin` — confirmed
- Multi-dot extensions used: `"dungeon.ron"`, `"items.ron"`, etc. — confirmed
- Camera2d tagged `LoadingScreenRoot` alongside UI tree — confirmed
- Hot-reload: both pieces present (`bevy/file_watcher` in `dev` feature AND `AssetPlugin { watch_for_changes_override: Some(cfg!(feature = "dev")), ..default() }`) — confirmed
- `src/plugins/state/mod.rs` matches the Feature #2 PR #2 source — unmodified

## Verdict: APPROVE
