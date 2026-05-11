# Pipeline Summary — Feature #17: Enemy Billboard Sprite Rendering (Research → Plan → Implement → Ship → Review)

**Date:** 2026-05-11
**Pipeline scope:** research → plan → implement → ship → review. PR #17 is open and reviewed; merge awaits user authorization.
**Status:** Pipeline COMPLETE. Verification GREEN. Reviewer verdict **PASS** — 0 CRITICAL, 0 HIGH, 0 MEDIUM, 1 LOW (non-blocking).

---

## Original task

Drive the full feature pipeline for **Feature #17: Enemy Billboard Sprite Rendering** from the Druum (Bevy 0.18.1 first-person dungeon crawler) roadmap (`project/roadmaps/20260429-01-bevy-dungeon-crawler-roadmap.md` § 17, ~line 913). Difficulty 3/5 — `bevy_sprite3d` does the heavy lifting once 0.18 compat is confirmed.

**In scope:** Spawn enemies as billboarded 3D sprites on combat entry. 10 enemies in `core.enemies.ron`, each with a distinct solid-color placeholder (1.4m × 1.8m at `pixels_per_metre = 10.0`). 4-state animation machine (Idle / Attacking / TakingDamage / Dying). Damage-shake tween on HP decrease. `face_camera` system rewrites yaw each frame via `atan2(dx, dz)` (Y-axis-locked, uses `&GlobalTransform` because the camera is a `PlayerParty` child). `spawn_enemy_visual(commands, enemy_spec, position)` API agnostic of combat vs. overworld so #22 FOEs can reuse it.

**Out of scope (deferred per user decisions or natural seams):**
- Real sprite art (decision 1C — solid-color placeholders only this PR; CC0 itch.io packs are the eventual source)
- 4-directional sprites (decision 3A — single-facing now; revisited at #22)
- AttackStart / Died `EnemyVisualEvent` producers (only `DamageTaken` is wired; plumbing is forward-compatible for the AttackStart/Died hooks in `turn_manager.rs::execute_combat_actions` as follow-up)
- Manual smoke test (requires GPU/display — user must run `cargo run --features dev` + F7)

**Constraint envelope (final):**
- **+1 new source file:** `src/plugins/combat/enemy_render.rs` (~991 LOC including +14 tests)
- **+1 new dependency:** `bevy_sprite3d = "8"` (confirmed Bevy 0.18-compatible via `cargo tree`)
- **9 cascade-patched files:** `data/enemies.rs` (full EnemyDb schema), `data/encounters.rs` (+ `id` field), `plugins/combat/encounter.rs` (+ `EnemyVisual` insertion), `plugins/combat/enemy.rs` (+ EnemyBundle extension), `plugins/combat/mod.rs` (+ register EnemyRenderPlugin), `plugins/combat/turn_manager.rs`, `plugins/combat/ai.rs`, `plugins/combat/ui_combat.rs`, `plugins/dungeon/features.rs`, `plugins/dungeon/tests.rs`, integration tests `tests/dungeon_geometry.rs` + `tests/dungeon_movement.rs`
- **2 new assets / 1 modified:** `assets/enemies/core.enemies.ron` (10-enemy roster with tuple-RON placeholder colors), `assets/encounters/floor_01.encounters.ron` (populate `id` fields)
- **225 default tests / 229 dev-feature tests** (vs 211 / 215 after #16 — net +14 / +14, matches plan's promised +14)
- **Diff size:** +4 823 / -31 LOC (PR #17; majority is project/* markdown — production source code is in line with the plan's +430-530 LOC estimate; the new `enemy_render.rs` carries the bulk of production LOC including tests)

---

## Artifacts produced

| Step | Description | Path |
|------|-------------|------|
| 1 | Research | `project/research/20260511-feature-17-enemy-billboard-sprite-rendering.md` |
| 2 | Plan | `project/plans/20260511-120000-feature-17-enemy-billboard-sprite-rendering.md` (Status: Approved 2026-05-11 after `bevy_sprite3d 8.0` revision; marked Complete 2026-05-11) |
| 3 | Implementation summary | `project/implemented/20260511-120001-feature-17-enemy-billboard-sprite-rendering.md` |
| 4 | PR | https://github.com/codeinaire/druum-dungeon-crawler/pull/17 — body at `project/shipper/feature-17-pr-body.md` |
| 5 | Code review | `project/reviews/20260511-152336-feature-17-enemy-billboard-sprite-rendering-pr-review.md` (Verdict: PASS) |

**Commits on `feature/17-enemy-billboard-sprite-rendering`:**
- `9ae9f7f` — `feat(combat): add enemy billboard sprite rendering (#17)` — all production code, assets, tests
- `a018f04` — `docs: mark feature/17 implementation complete, update pipeline state`

---

## User decisions (checkpoint resolutions)

1. **1C** — Solid-color placeholder textures only this PR. No real art.
2. **2B** — 10 enemies in `enemies.ron` (front-loads combat balance for #21).
3. **3A** — Single-facing sprites; revisit 4-directional at #22.
4. **4A → superseded** — Original "manual billboard fallback if `bevy_sprite3d 7.x` incompatible" was replaced mid-pipeline. The user confirmed `bevy_sprite3d 8.0` supports Bevy 0.18, so the plan was revised to use the crate directly (planner verified via `cargo tree`).

**Implicit planner call (documented in plan):** Each of the 10 placeholder enemies gets a *distinct* solid color (near-zero cost, confirms per-enemy materials work, aids visual debugging).

---

## D-Ix deviations from plan

| ID | Description | Reason | When found |
|----|-------------|--------|------------|
| D1 | `init_asset::<EnemyDb>` added to 3 lib test apps | Test scaffolds needed to load the new asset registry | Initial implementation |
| D2 | `DEFAULT_PLACEHOLDER_COLOR` exported as `pub const` | Shared between `enemy_render.rs` and `encounter.rs` | Initial implementation |
| D3 | Removed unused imports | Lint cleanup | Initial implementation |
| D4 | Mesh + StandardMaterial asset init in `enemy_render` test app | `Sprite3dPlugin` dependency | Initial implementation |
| D5 | `PreviousHp` seeding-frame semantic explicit in tests | Test 3 documents "first HP increase is not damage" boundary | Initial implementation |
| D6 | RON `(r,g,b)` tuples, NOT `[r,g,b]` sequences | RON 0.11 serializes fixed-size `[f32; 3]` as tuples, not seqs — `ExpectedStructLike` error blocked parsing | Recovery verification (D6) |
| D7 | `Assets<Image>` + `Assets<TextureAtlasLayout>` added to **all 9** test harnesses (not just 3) | `Sprite3dPlugin::bundle_builder` declares both as required system parameters; 48 test panics until fixed | Recovery verification (D7) |
| D8 | Split `EntityWorldMut` borrow in test code | E0716: `entity_mut().get_mut::<T>().unwrap()` chain drops `EntityWorldMut` before `Mut<T>` borrow is used | Recovery verification (D8) |
| D9 | `collapsible_if` → let-chain in `on_enemy_visual_event` | Rust 2024 + clippy rule | Recovery verification (D9) |

D6-D9 were caught only because the recovery implementer ran the cargo gates — the prior implementer had skipped verification.

---

## Recovery from skill-routed implementer regression

**The same Feature #16 regression occurred:** the `run-implementer` skill-routed agent wrote all code to disk correctly, then claimed "no shell access" and stopped before verification + commit. Its summary invented a non-existent ship/review hand-off target.

**Recovery path (Feature #16 precedent):** Bypass the skill-routed dispatch. Invoke `subagent_type: implementer` directly from the main session, which preserves Bash access. This worked on first try — implementer ran the 6 gates, found and fixed D6-D9, then committed via `but` per CLAUDE.md.

**Lessons** (already in implementer memory from Feature #16, reconfirmed here):
- "No shell access" claims from agents are a hallucination, not reality. Direct subagent invocation always works.
- Verification gates must run inside the implementer's own session — passing them to "the next agent" is the failure pattern.
- Plan grep checks catch real things (the `Sprite3dBillboard → EnemyBillboard` rename was caught here, ensuring no stale references survived).

The pipeline state file was kept in sync throughout: `PIPELINE-STATE.md` went `blocked-at-step-3` → `step-3-complete` → `complete` as recovery progressed.

---

## Reviewer findings (full review at `project/reviews/20260511-152336-feature-17-...md`)

**Verdict: PASS** (1 LOW, no blockers).

### LOW-1 — hardcoded fallback color in `encounter.rs`

`src/plugins/combat/encounter.rs` uses `.unwrap_or([0.5, 0.5, 0.5])` for the per-enemy placeholder color instead of referencing the exported `DEFAULT_PLACEHOLDER_COLOR` const from `enemy_render.rs`. If the default changes in one place, the two sites drift silently. Non-blocking — easy follow-up.

### All 8 high-priority correctness checks PASS

- `face_camera` uses `&GlobalTransform` with `Without<DungeonCamera>` disjoint-set guard and correct `atan2(dx, dz)` yaw math
- `Sprite3dPlugin` registered idempotently via `is_plugin_added` guard
- No `.despawn()` in `enemy_render.rs`; cleanup is free through `clear_current_encounter`'s `With<Enemy>` sweep
- `PreviousHp` first-frame seeding correctly skips counting HP increases as damage
- `AlphaMode::Mask(0.5)` only — no `Blend`
- `RenderAssetUsages::RENDER_WORLD` only — no `MAIN_WORLD`
- `unlit: true` set on `Sprite3d` material
- 10-enemy RON parses with tuple notation, distinct hues, stats calibrated against level-1 party (acc≈73, eva≈13)

---

## Outstanding items (visibility, not blocking #17)

- **PR #16 has 3 MEDIUM / 2 LOW findings still unaddressed** at user's direction. Independent of #17.
- **Manual smoke test (REVIEWER → USER):** `cargo run --features dev`, F7 to force encounter:
  - (a) Billboards appear in row with distinct hues
  - (b) Sprites face camera on rotation (yaw-only, no roll)
  - (c) Clean despawn on combat exit (no leaked entities)
  - (d) Damage-shake jitter visible

---

## Future feature dependencies (from roadmap)

- **#22 (FOEs / visible enemies on dungeon grid)** — Reuses `spawn_enemy_visual` with a different transform parent. The API was deliberately written to be agnostic of combat vs. overworld so #22 needs no rework.
- **#21 (combat balance)** — Consumes the 10-enemy roster's stat blocks (acc / eva calibrated against level-1 party).
- **AttackStart / Died event producers** — Follow-up PR. The `EnemyVisualEvent` plumbing is forward-compatible; only `turn_manager.rs::execute_combat_actions` hooks need adding.

---

## Stats

- Time elapsed: research → review, single working session
- Total subagent invocations: 5 (researcher, planner ×2 for the bevy_sprite3d 8.0 revision, implementer, shipper, reviewer)
- Gate failures fixed during recovery: 4 (D6-D9)
- Plan steps: 12 (was 11 before the `bevy_sprite3d 8.0` revision)
