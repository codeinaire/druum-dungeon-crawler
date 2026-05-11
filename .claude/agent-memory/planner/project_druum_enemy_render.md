---
name: Druum enemy billboard rendering (Feature #17) decisions
description: Feature #17 plan resolutions — manual-quad path locked by user 4A, Image::new_fill for placeholders, additive id field on EnemySpec, damage-shake in scope, EnemyBundle additive extension
type: project
---

Feature #17 — Enemy Billboard Sprite Rendering — plan written 2026-05-11. Key decisions and the reasoning behind them (so future plans don't re-litigate):

**D-O1 — Manual quad path (Mesh3d + Rectangle + StandardMaterial), NOT `bevy_sprite3d`:**
- User decision 4A locked this; Step A/B/C verification gate for `bevy_sprite3d` was explicitly skipped.
- Public API of `EnemyRenderPlugin` is unchanged by future swap — follow-up PR can convert `Mesh3d` → `Sprite3d` mechanically.
- Reason: roadmap authorised fallback; `bevy_sprite3d` 0.18 compat was unverifiable in research session.

**D-O2 — `Image::new_fill` for placeholder textures, NOT committed PNG files:**
- Zero new files in `assets/enemies/`; texture generated in-memory at spawn time from RON `placeholder_color: [f32; 3]`.
- Schema retains optional `sprite_path: Option<String>` so future real-art PR is a one-line data change per enemy.
- `RenderAssetUsages::RENDER_WORLD` (NOT `MAIN_WORLD` — Pitfall 7; `MAIN_WORLD` causes GPU copy to be freed at runtime).
- Generated INSIDE spawn loop (not at startup in a `PlaceholderImages` resource) — the entity owns the `MeshMaterial3d` handle; ref-counted asset cleanup drops everything when `clear_current_encounter` despawns. No leak.

**D-O3 — Additive `#[serde(default)] pub id: String` on `EnemySpec` for visual lookup:**
- Mirrors migration-path doc comment at `src/data/encounters.rs:7-11`.
- Existing `floor_01.encounters.ron` updated to populate `id` (5 specs across 4 entries).
- Back-compat: empty `id` falls back to `DEFAULT_PLACEHOLDER_COLOR` = `[0.5, 0.5, 0.5]` grey. `EnemyDb::find("")` returns `None` explicitly.

**D-O4 — Damage shake IS in scope:**
- ~20 LOC sine-jitter on `Transform.translation.x` over 0.15s, triggered by `EnemyVisualEvent::DamageTaken`.
- 4 wobbles over duration, linear amplitude attenuation.
- `DamageShake { base_x, elapsed_secs }` component lifecycle (insert on event → tick → remove + snap to base_x on expiry). Mirrors `MovementAnimation` pattern at `dungeon/mod.rs:117-153`.
- Deterministic test via `TimeUpdateStrategy::ManualDuration(160ms)`.

**D-A9 — `EnemyVisual` + `EnemyAnimation` are `Default`-derived fields on `EnemyBundle`:**
- Existing `..Default::default()` at `encounter.rs:376` picks them up — no change to spawn signature.
- `EnemyBundle` still `#[derive(Bundle, Default)]` — this is NOT one of the removed Bevy 0.17 `*Bundle` types (those were engine types like `Camera3dBundle`). Custom `#[derive(Bundle)]` is still encouraged in 0.18.
- Encounter spawn site adds ONE extra line: `commands.entity(entity).insert(EnemyVisual { id, placeholder_color })` after `.id()`, with `placeholder_color` resolved via `enemy_db.find(&spec.id)` lookup.

**D-A7 — Cleanup is FREE — DO NOT add a second despawn system:**
- Visual components live on the SAME entity as `Enemy`. `clear_current_encounter` at `combat/encounter.rs:200-215` already sweeps `Query<Entity, With<Enemy>>` and despawns each.
- Adding a second despawn risks double-despawn panics. Verification gate greps `enemy_render.rs` for `.despawn()` — must return ZERO.

**D-A10 — `EnemyVisualEvent` is a `Message`, NOT an `Event`:**
- Bevy 0.18 family rename. `#[derive(Message)]`, `MessageReader`, `MessageWriter`, `app.add_message::<T>()`. Locked by #15/#16 precedent.

**D-A11 — Minimal `DamageTaken` producer hook (planner scope call):**
- `detect_enemy_damage` system in `enemy_render.rs` compares per-frame `DerivedStats.current_hp` against a `PreviousHp(u32)` component snapshot. Emits `EnemyVisualEvent::DamageTaken` on decrease.
- `AttackStart` and `Died` producers are DEFERRED to follow-up. The events and reader exist from day one; only `DamageTaken` is wired this PR.
- Rationale: keeps coupling loose (no edit to `turn_manager.rs::execute_combat_actions`); HP-delta watcher is a 15-line system; full attack/died integration is bounded scope but better as a separate PR.

**D-A12 — Visual constants:**
- `SPRITE_WIDTH = 1.4`, `SPRITE_HEIGHT = 1.8`, `SPRITE_DISTANCE = 4.0`, `SPRITE_SPACING = 1.6`, `SPRITE_Y_OFFSET = 0.8`, `SHAKE_AMPLITUDE = 0.08`, `SHAKE_DURATION_SECS = 0.15`, `ANIMATION_FRAME_SECS = 0.12`, `DEFAULT_PLACEHOLDER_COLOR = [0.5, 0.5, 0.5]`.
- Tunable post-merge; #21 balance work may adjust spacing.

**Hue table for 10-enemy roster (`assets/enemies/core.enemies.ron`):**
- Green / Goblin / RandomAttack
- Ochre / Goblin Captain / BossFocusWeakest
- Dark Purple / Cave Spider / RandomAttack
- Red / Hobgoblin / RandomAttack
- Orange / Kobold / RandomAttack
- Yellow / Acid Slime / RandomAttack
- Blue / Ice Imp / RandomAttack
- Cyan / Wraith / RandomAttack
- Magenta / Cultist / RandomAttack
- White / Skeleton Lord / BossAttackDefendAttack(turn: 0)

**Public API for #22 FOE reuse:**
- `spawn_enemy_visual(commands, meshes, materials, images, shared_quad, entity, placeholder_color, position)` is the agnostic spawn helper.
- `spawn_enemy_billboards` (combat-specific) computes the row layout and calls `spawn_enemy_visual` per enemy.
- #22 will call `spawn_enemy_visual` directly with overworld grid positions.

**Test deltas:**
- +5 unit tests in `data::enemies::tests` (round-trip, find, find-empty-id, find-unknown-id, 10-enemy-roster-parses)
- +5 unit tests in `combat::enemy_render::tests` (face-camera math, Image::new_fill round-trip, clamp, animation Attacking→Idle, animation Dying-holds-last)
- +4 integration tests in `combat::enemy_render::app_tests` (OnEnter attaches billboards, OnExit despawns billboards, HP-delta emits DamageTaken, damage-shake snaps back to base_x)

**Non-obvious traps I avoided in the plan:**
- `Rectangle` NOT `Plane3d` (Plane3d defaults to facing +Y — floor orientation).
- `&GlobalTransform` NOT `&Transform` for DungeonCamera query (camera is child of PlayerParty).
- `Without<DungeonCamera>` on sprite query to satisfy B0001 disjoint-set rule.
- Path typo in user-supplied output: `/Users/nousunio/Repos/Learnings/druum/project/plans/` — actual path is `/Users/nousunio/Repos/Learnings/claude-code/druum/project/plans/`. Plan correctly written to the latter.
