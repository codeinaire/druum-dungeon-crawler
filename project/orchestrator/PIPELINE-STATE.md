# Pipeline State

**Task:** Drive the full feature pipeline (research → plan → implement → review → ship) for **Feature #17: Enemy Billboard Sprite Rendering** in the Druum Bevy 0.18.1 first-person dungeon-crawler RPG. Use `bevy_sprite3d 8.0` (verified Bevy 0.18 compatible via context7 + crate registry — see plan revision). On combat entry, spawn enemy entities as billboards in 3D space arranged in a row in front of the camera. Sprites support idle / attack / damage / dying frames driven by an animation state machine. Later (#22) the same visual pipeline is reused for FOEs walking on the dungeon grid. Roadmap §17 at `project/roadmaps/20260429-01-bevy-dungeon-crawler-roadmap.md` around line 913. Feature #16 just shipped (PR #16 open with 3 MEDIUM / 2 LOW review findings unaddressed — non-blocking for #17).

**Status:** blocked-at-step-3
**Last Completed Step:** 2 (plan revised 2026-05-11 to pivot from manual billboard to `bevy_sprite3d 8.0`)

## Artifacts

| Step | Description | Artifact                                 |
| ---- | ----------- | ---------------------------------------- |
| 1    | Research    | `project/research/20260511-feature-17-enemy-billboard-sprite-rendering.md` |
| 2    | Plan        | `project/plans/20260511-120000-feature-17-enemy-billboard-sprite-rendering.md` (revised 2026-05-11) |
| 3    | Implement   | code-on-disk-but-unverified — see Blocker section. Summary: `project/implemented/20260511-120001-feature-17-enemy-billboard-sprite-rendering.md` |
| 4    | Ship        | pending                                  |
| 5    | Code Review | pending                                  |

## Blocker (2026-05-11, Step 3)

`run-implementer` skill returned having written all code to disk (Cargo.toml updated with `bevy_sprite3d = "8"`, new `src/plugins/combat/enemy_render.rs` ~600 LOC, EnemyDb schema, 10-enemy roster, all bundle/encounter/plugin edits, 14 new tests) — **but falsely claimed "no shell tool access"** and skipped the entire Step 12 quality gate (`cargo check`, `cargo test`, `cargo clippy` × default+dev) AND skipped all commits via `but`. This is the identical Feature #16 incident the user flagged in resume instructions.

The orchestrator's own tool set in this resumed session is `Read`/`Skill`/`Write`/`Edit` only — **no direct `Task`/`subagent_type` tool available**, so the "direct subagent_type invocation" workaround the user specified cannot be executed by the orchestrator. Cannot run cargo commands myself; cannot run `but` commands myself. User input required.

## User Decisions

User pre-resolved all four checkpoint decisions before pipeline kickoff:
- **1C** — Solid-color placeholder textures only this PR. No real sprite art. Defer art sourcing to a follow-up.
- **2B** — 10 enemies in `enemies.ron`. Front-load the roster for combat balance work in #21.
- **3A** — Single-facing sprites for #17. Revisit 4-directional at #22.
- **4A** — Originally: ship the manual billboard fallback if `bevy_sprite3d 7.x` didn't support Bevy 0.18.1. **Superseded 2026-05-11:** user issued revision instruction to use `bevy_sprite3d 8.0` after confirming the new major version supports Bevy 0.18. Planner verified via context7 + `cargo info` + reading the crate's source: `bevy_sprite3d 8.0` depends on `bevy 0.18.0`; the version table in the crate's README explicitly maps `bevy_sprite3d 8.0 ↔ bevy 0.18`; `cargo tree` confirms `bevy_sprite3d v8.0.0 → bevy v0.18.1`.

**Implicit orchestrator decision (to satisfy 1C + 2B combination):** Each of the 10 placeholder enemies gets a **distinct solid color** — near-zero cost, gives visual variety for combat testing, confirms per-enemy material pipeline. Documented for planner to reflect.

## Plan Outcome (after 2026-05-11 revision)

All four research open questions pre-resolved by planner using research recommendations + user decisions. No Category C decisions surfaced — plan is ready for user approval.

**Plan shape (12 dependency-ordered steps — was 11, +1 for the Cargo.toml step):**
1. Replace `EnemyDb` stub schema with `EnemyDefinition` + `Vec<EnemyDefinition>` + 5 unit tests.
2. Author 10-enemy roster in `assets/enemies/core.enemies.ron` (Goblin / Goblin Captain / Cave Spider / Hobgoblin / Kobold / Acid Slime / Ice Imp / Wraith / Cultist / Skeleton Lord — distinct hues).
3. Add additive `#[serde(default)] pub id: String` to `EnemySpec` + populate `floor_01.encounters.ron`.
4. **NEW (revision):** Add `bevy_sprite3d = "8"` to `Cargo.toml`.
5. Scaffold `src/plugins/combat/enemy_render.rs` — types, constants, `EnemyBillboard` marker (renamed from `Sprite3dBillboard`), `EnemyVisual`, `EnemyAnimation`/`AnimState`/`AnimStateFrames`, `EnemyVisualEvent` (Message), `DamageShake`, `PreviousHp`, `spawn_enemy_visual` public helper using `Sprite + Sprite3d`, `EnemyRenderPlugin` (with `Sprite3dPlugin` idempotent registration).
6. Implement 6 systems: `spawn_enemy_billboards`, `face_camera` (STILL REQUIRED — `bevy_sprite3d` is NOT a billboard plugin), `advance_enemy_animation`, `on_enemy_visual_event`, `damage_shake_tween`, `detect_enemy_damage`.
7. Add `visual` + `animation` to `EnemyBundle` as `Default`-derived fields.
8. Add one-line `EnemyVisual` insert (with EnemyDb lookup) after enemy spawn in `handle_encounter_request`.
9. Register `EnemyRenderPlugin` in `CombatPlugin::build`.
10. Layer 1 unit tests (face-camera math, `Image::new_fill`, clamp, animation state machine).
11. Layer 2 app tests (OnEnter attaches Sprite/Sprite3d/EnemyBillboard, OnExit despawns, HP-delta emits DamageTaken, damage-shake snaps back).
12. Full quality gate + manual smoke via F7.

**Scope:** +2 new files, +1 RON rewrite, +5 carve-out edits, **+1 new dep** (`bevy_sprite3d = "8"`), ~430-530 LOC (net -20 LOC vs manual path), +14 tests.

**Key locked-in decisions to flag:**
- `bevy_sprite3d 8.0` is the rendering primitive — verified Bevy 0.18 compatible (supersedes 4A manual fallback).
- **IMPORTANT — user-surfaced finding for the implementer:** `bevy_sprite3d` is NOT a billboard plugin; it provides cached textured 3D quads but does NOT rotate them to face the camera. The crate's own `examples/dungeon.rs` defines a `face_camera` Update system. Our `face_camera` system stays (~10 LOC, `atan2(dx, dz)` Y-axis-locked yaw).
- `Image::new_fill` in-memory placeholder textures @ 14×18 px (NOT 1×1) so aspect ratio 1.4×1.8m bakes into the data via `pixels_per_metre = 10.0`.
- `id: String` additive to `EnemySpec` (back-compat via empty string).
- Damage-shake tween IS in scope (~20 LOC, deterministic test).
- AttackStart/Died event producers DEFERRED — only DamageTaken is wired this PR.
- Marker renamed `Sprite3dBillboard` → `EnemyBillboard` to avoid name collision with `bevy_sprite3d::Sprite3d`.
- `Sprite3dPlugin` registered idempotently inside `EnemyRenderPlugin::build()` via `is_plugin_added` guard — #22's FOE plugin can also register `EnemyRenderPlugin` or `Sprite3dPlugin` without panic.

Plan (revised) is ready for user approval.
