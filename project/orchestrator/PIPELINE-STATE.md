# Pipeline State

**Task:** Drive the full feature pipeline (research → plan → implement → review → ship) for **Feature #17: Enemy Billboard Sprite Rendering** in the Druum Bevy 0.18.1 first-person dungeon-crawler RPG. Use `bevy_sprite3d 8.0` (verified Bevy 0.18 compatible via context7 + crate registry — see plan revision). On combat entry, spawn enemy entities as billboards in 3D space arranged in a row in front of the camera. Sprites support idle / attack / damage / dying frames driven by an animation state machine. Later (#22) the same visual pipeline is reused for FOEs walking on the dungeon grid. Roadmap §17 at `project/roadmaps/20260429-01-bevy-dungeon-crawler-roadmap.md` around line 913. Feature #16 just shipped (PR #16 open with 3 MEDIUM / 2 LOW review findings unaddressed — non-blocking for #17).

**Status:** step-3-complete — pending ship (step 4)
**Last Completed Step:** 3 (implement — all 6 quality gates green, committed 2026-05-11)

## Artifacts

| Step | Description | Artifact                                 |
| ---- | ----------- | ---------------------------------------- |
| 1    | Research    | `project/research/20260511-feature-17-enemy-billboard-sprite-rendering.md` |
| 2    | Plan        | `project/plans/20260511-120000-feature-17-enemy-billboard-sprite-rendering.md` (revised 2026-05-11) |
| 3    | Implement   | `project/implemented/20260511-120001-feature-17-enemy-billboard-sprite-rendering.md` |
| 4    | Ship        | pending — branch `feature/17-enemy-billboard-sprite-rendering`, commit `9ae9f7f` |
| 5    | Code Review | pending                                  |

## Commit SHAs (Step 3)

- `9ae9f7f` — `feat(combat): add enemy billboard sprite rendering (#17)` — all 16 files, 411 insertions, 29 deletions, on branch `feature/17-enemy-billboard-sprite-rendering`

## User Decisions

User pre-resolved all four checkpoint decisions before pipeline kickoff:
- **1C** — Solid-color placeholder textures only this PR. No real sprite art. Defer art sourcing to a follow-up.
- **2B** — 10 enemies in `enemies.ron`. Front-load the roster for combat balance work in #21.
- **3A** — Single-facing sprites for #17. Revisit 4-directional at #22.
- **4A** — Originally: ship the manual billboard fallback if `bevy_sprite3d 7.x` didn't support Bevy 0.18.1. **Superseded 2026-05-11:** user issued revision instruction to use `bevy_sprite3d 8.0` after confirming the new major version supports Bevy 0.18. Planner verified via context7 + `cargo info` + reading the crate's source: `bevy_sprite3d 8.0` depends on `bevy 0.18.0`; the version table in the crate's README explicitly maps `bevy_sprite3d 8.0 ↔ bevy 0.18`; `cargo tree` confirms `bevy_sprite3d v8.0.0 → bevy v0.18.1`.

**Implicit orchestrator decision (to satisfy 1C + 2B combination):** Each of the 10 placeholder enemies gets a **distinct solid color** — near-zero cost, gives visual variety for combat testing, confirms per-enemy material pipeline.

## Quality Gate Results (Step 3)

All 6 gates passed after recovery agent fixed 4 bugs in prior implementer's code:

| Command | Exit | Notes |
|---------|------|-------|
| `cargo check` | 0 | clean |
| `cargo check --features dev` | 0 | clean |
| `cargo test` | 0 | 225 lib + 6 integration tests |
| `cargo test --features dev` | 0 | 229 lib + 6 integration tests |
| `cargo clippy --all-targets -- -D warnings` | 0 | clean |
| `cargo clippy --all-targets --features dev -- -D warnings` | 0 | clean |

Bugs fixed during Step 3 recovery:
1. `core.enemies.ron` used `[r,g,b]` (RON sequence) instead of `(r,g,b)` (RON tuple) for `[f32;3]` — caused `ExpectedStructLike` parse error
2. `bevy_sprite3d::bundle_builder` requires `Assets<Image>` + `Assets<TextureAtlasLayout>` in ALL test apps that use CombatPlugin — 9 harnesses updated
3. E0716 in test code — `entity_mut().get_mut()` chain needed two named bindings
4. `clippy::collapsible_if` in `on_enemy_visual_event` — let-chain collapse required in Rust 2024 edition

## Next Step

**Step 4 — Ship**: push branch `feature/17-enemy-billboard-sprite-rendering` and open PR via `gh pr create`. Branch is at commit `9ae9f7f`. No blockers.
