# Review: Feature #7 — Grid Movement & First-Person Camera

**Date:** 2026-05-03
**PR:** https://github.com/codeinaire/druum-dungeon-crawler/pull/7
**Branch:** `7-grid-movement-first-person-camera`
**Verdict:** APPROVE

## Severity Counts

| Severity | Count |
|----------|-------|
| CRITICAL | 0     |
| HIGH     | 0     |
| MEDIUM   | 0     |
| LOW      | 1     |

## Files Reviewed

Full review:
- `src/plugins/dungeon/mod.rs` (997 LOC — primary new file)
- `tests/dungeon_movement.rs` (169 LOC — new integration test)
- `Cargo.toml` (byte-change gate, both branches)
- `src/plugins/input/mod.rs` (DungeonAction enum verification)
- `src/plugins/state/mod.rs` (DungeonSubState, GameState)
- `src/plugins/audio/sfx.rs` (SfxRequest/SfxKind contract)
- `src/plugins/loading/mod.rs` (DungeonAssets struct shape)
- `src/data/dungeon.rs` (Direction, DungeonFloor, WallMask, can_move)

Skipped (pure rustfmt normalization per PR description, no semantic changes):
- `src/plugins/audio/mod.rs`
- `src/plugins/combat/mod.rs`
- `src/plugins/town/mod.rs`
- `src/main.rs`
- `tests/dungeon_floor_loads.rs`
- Agent memory files (implementer/researcher/planner)

## Key Findings

### [LOW] Stale doc-comment in `data/dungeon.rs:18`

**File:** `src/data/dungeon.rs:18`

The `Direction` doc-comment states `world_z = -grid_y * cell_size`. The actual implemented convention in `src/plugins/dungeon/mod.rs` is `world_z = +grid_y * CELL_SIZE` (positive). The two are contradictory. File correctly treated as frozen this PR; the `dungeon/mod.rs` module-level doc is the source of truth. Recommend fixing the doc-comment in Feature #8 (which already touches this domain) to prevent misleading Feature #8's renderer implementer.

## Hard-Gate Check Results

All critical checks passed:
- Cargo.toml + Cargo.lock byte-unchanged (SHA `bd231bcf` identical on branch and main)
- `MovedEvent` derives `Message`, registered via `app.add_message::<MovedEvent>()`
- Logical-vs-visual separation: `GridPosition`/`Facing` update immediately at commit; `MovementAnimation` lerps `Transform`
- `MovedEvent` + `SfxRequest::Footstep` written ONLY for translation moves (not turns, not wall-bumps)
- Input lock = `Without<MovementAnimation>` query filter (no separate flag)
- `handle_dungeon_input.before(animate_movement)` explicitly in `DungeonPlugin::build`
- State-gating at SYSTEM level: `run_if(in_state(...))` — no `Plugin::run_if`
- World coordinate convention: `world_z = +grid_y * CELL_SIZE` — North = (0,-1) → -Z world → `Quat::IDENTITY`
- `Camera3d` as component tuple via `children![...]` — no `Camera3dBundle`
- `Time::delta_secs()` (not `delta_seconds()`)
- No `rand` calls
- `despawn()` recursive in Bevy 0.18 — child `DungeonCamera` cleaned up automatically
- `#[cfg(feature = "dev")] app.init_resource::<ButtonInput<KeyCode>>()` in both test apps

## TestState Deviation Assessment

`LoadingPlugin` hang workaround is sound. Test loads real `floor_01.dungeon.ron` via `RonAssetPlugin`, exercises real `spawn_party_and_camera` via `OnEnter(GameState::Dungeon)`. Only `AudioAssets` (irrelevant to this feature) is bypassed. `AssertDone` + `Update` system pattern correctly handles one-frame command-deferral. Test value not compromised.
