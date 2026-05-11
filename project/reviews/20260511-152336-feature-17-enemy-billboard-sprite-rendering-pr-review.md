# Feature #17 PR Review

**PR:** https://github.com/codeinaire/druum-dungeon-crawler/pull/17
**Reviewer:** code-reviewer agent
**Date:** 2026-05-11

## Verdict

PASS — all 8 high-priority checks confirmed correct; one LOW finding (hardcoded fallback colour in encounter.rs instead of the exported constant).

## Finding Counts

- CRITICAL: 0
- HIGH: 0
- MEDIUM: 0
- LOW: 1

---

## Findings

### [LOW] `encounter.rs` hardcodes `[0.5, 0.5, 0.5]` instead of using `DEFAULT_PLACEHOLDER_COLOR`

**Location:** `src/plugins/combat/encounter.rs` — inside `handle_encounter_request`, the `.unwrap_or(...)` after the `EnemyDb` lookup.

**Issue:** `enemy_render.rs` exports `pub const DEFAULT_PLACEHOLDER_COLOR: [f32; 3] = [0.5, 0.5, 0.5]`. The fallback colour in `encounter.rs` uses the literal value instead and only marks the relationship via a comment (`// mirrors DEFAULT_PLACEHOLDER_COLOR`). If the default is ever changed for design reasons, `encounter.rs` silently diverges.

**Suggested fix:**

```rust
// current
.unwrap_or([0.5, 0.5, 0.5]); // mirrors DEFAULT_PLACEHOLDER_COLOR

// preferred
.unwrap_or(crate::plugins::combat::enemy_render::DEFAULT_PLACEHOLDER_COLOR);
```

This is strictly cosmetic for the current constant value; the comment makes the intent clear enough for now. Not a blocker.

---

## High-Priority Checks (all pass)

1. **`face_camera` math** — Queries `&GlobalTransform` (not `&Transform`) from `DungeonCamera`. `Without<DungeonCamera>` filter on sprite query satisfies Bevy B0001. Yaw computed via `atan2(dx, dz)` Y-axis-locked. The unit test covers all four cardinal directions with 1e-4 tolerance. CORRECT.

2. **`Sprite3dPlugin` idempotency** — `if !app.is_plugin_added::<Sprite3dPlugin>() { app.add_plugins(Sprite3dPlugin); }` is present verbatim in `EnemyRenderPlugin::build`. Future plugins that also call `add_plugins(Sprite3dPlugin)` or `add_plugins(EnemyRenderPlugin)` won't panic. CORRECT.

3. **Cleanup semantics** — No `.despawn()` anywhere in `enemy_render.rs`. Visual components (`Sprite`, `Sprite3d`, `EnemyBillboard`, `DamageShake`, `PreviousHp`) live on the same entity as `Enemy`. `clear_current_encounter` (encounter.rs) despawns by `With<Enemy>` sweep — all visual components go transitively. No double-despawn risk. CORRECT.

4. **`PreviousHp` seeding semantic** — `None` branch: inserts `PreviousHp(stats.current_hp)` without emitting `DamageTaken`. An HP _increase_ on the seeding frame is not counted as damage (`30 < 0` is false). Integration test 3 explicitly verifies this: sets HP from 0→30 (no event), then 30→25 (event fires). CORRECT.

5. **`AlphaMode::Mask(0.5)` not `Blend`** — `Sprite3d { alpha_mode: AlphaMode::Mask(0.5), .. }` is set explicitly in `spawn_enemy_visual`. No `Blend` variant anywhere in the file. CORRECT.

6. **`RenderAssetUsages::RENDER_WORLD`** — Used in `Image::new_fill` inside `spawn_enemy_visual`. No `MAIN_WORLD` reference in the file. CORRECT.

7. **`unlit: true`** — `Sprite3d { unlit: true, .. }` is set in `spawn_enemy_visual`. CORRECT.

8. **10-enemy roster sanity** — All 10 enemies present in `core.enemies.ron` with distinct hues. `placeholder_color` uses RON tuple notation `(r, g, b)` (not bracket `[r, g, b]`). Stats for `goblin` / `goblin_captain` / `cave_spider` match `floor_01.encounters.ron` exactly (single source of truth for #21 balance). The `core_enemies_ron_parses_with_10_enemies` test verifies parse + unique-id count. CORRECT.

---

## What looked good

- **`spawn_enemy_visual` is genuinely agnostic of combat context.** No `CurrentEncounter` lookup, no `GameState` gate, no reference to `CombatPlugin` internals. The function signature (`commands, images, entity, color, position`) is exactly the seam #22 needs to call from overworld grid logic. The combat-specific row layout lives entirely in `spawn_enemy_billboards`, which is the right partition.

- **`EnemyVisualEvent` forward-compatibility.** Three variants (`AttackStart`, `DamageTaken`, `Died`) with a dedicated reader in `on_enemy_visual_event` that already has match arms for all three. Adding `AttackStart` or `Died` producers in a future PR requires zero changes to the event type or the reader — only new emitters in `turn_manager.rs`. The design absorbs the scope reduction cleanly.

- **Test quality on the damage-detection system.** Integration test 3 (`hp_delta_emits_damage_taken_event`) correctly exercises the seeding-frame edge case and directly reads from the `Messages<EnemyVisualEvent>` buffer rather than faking it. The two-step sequence (0→30 no event, 30→25 event) is the precise boundary condition for the invariant.

- **Production LOC is within plan estimate.** `enemy_render.rs` has ~395 LOC of production code + ~596 LOC of tests + doc comments. `enemies.rs` adds ~100 LOC production. Total new production code ≈ 495 LOC — squarely within the plan's +430-530 estimate. The 4823 total diff insertions are dominated by project/research/memory markdown, not source files.

---

## Items the user must verify (out of reviewer scope)

These require running the app (`cargo run --features dev`) and pressing **F7** from the dungeon view to force-start combat:

- [ ] **Enemy billboards appear in a row** in front of the camera with distinct solid colours (no two enemies the same hue).
- [ ] **Sprites face the camera** as you rotate — yaw tracks correctly, no roll.
- [ ] **Clean despawn on combat exit** (press Escape or let combat resolve) — no leaked billboard entities in the scene.
- [ ] **Damage-shake jitter** visible when an enemy takes HP damage — brief x-axis sine-jitter over ~0.15 s.
- [ ] **AttackStart / Died animations** — stub arms exist; confirming no panic is sufficient (producers are deferred).

---

## Files reviewed (full coverage)

- `src/plugins/combat/enemy_render.rs` — full read (991 LOC)
- `src/data/enemies.rs` — full read (145 LOC)
- `src/plugins/combat/encounter.rs` — full read (new additions + existing context)
- `src/plugins/combat/enemy.rs` — full read
- `src/plugins/combat/mod.rs` — full read
- `src/data/encounters.rs` — full read
- `assets/enemies/core.enemies.ron` — full read
- `assets/encounters/floor_01.encounters.ron` — full read
- `Cargo.toml` — full read
- Agent/planner/implementer memory files — sampled for deviation context
