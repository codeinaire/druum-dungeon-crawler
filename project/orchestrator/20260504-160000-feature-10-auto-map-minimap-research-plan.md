# Pipeline Summary — Feature #10: Auto-Map / Minimap (research → plan, stopped at plan-approval)

**Date:** 2026-05-04
**Pipeline scope:** research → plan → STOP. Implementer NOT dispatched (per brief: parent dispatches manually after user resolves D1–D7 because `SendMessage` does not actually resume returned agents — confirmed across Features #3-#9).
**Status:** Plan drafted; awaiting user decisions on 7 Category B questions before implementer dispatch.

## Original Task

Drive the full pipeline (research → plan, then PAUSE for plan approval) for **Feature #10: Auto-Map / Minimap** from the dungeon crawler roadmap (lines 538–587). Difficulty 2.5/5. Bevy 0.18.1 + Rust. First feature to add a non-trivial dependency (`bevy_egui`).

Adds: `ExploredCells` resource (HashMap of `(floor, x, y)` → `ExploredState ∈ {Unseen, Visited, KnownByOther}`), `MovedEvent` subscriber that flips cells to Visited, egui painter rendering grid + walls (per-WallType color) + explored shading + player arrow, full-screen view in `DungeonSubState::Map`, top-right minimap overlay during `DungeonSubState::Exploring`, dark-zone cells skip updates with "?" indicator, debug "show full map" toggle.

## Stages Run

### Stage 1 — Research (run-researcher skill)

Document: `/Users/nousunio/Repos/Learnings/claude-code/druum/project/research/20260504-130000-feature-10-auto-map-minimap.md`

**HIGH-confidence verifications (codebase, all on-disk):**
- `MovedEvent` exists at `src/plugins/dungeon/mod.rs:192-197`; derives `Message` (NOT `Event` — Bevy 0.18 family rename); carries `from`/`to`/`facing`; written at line 678 only after committed translation moves. Subscribers MUST use `MessageReader<MovedEvent>`.
- `DungeonSubState::Map` declared at `src/plugins/state/mod.rs:23` — `OnEnter`/`OnExit` work for free.
- `DungeonAction::OpenMap` exists at `src/plugins/input/mod.rs:79`; bound to `KeyCode::KeyM` at line 150 — no input-enum work needed.
- `DungeonFloor` public API: `width: u32`, `height: u32` (direct field access), `walls: Vec<Vec<WallMask>>` indexed `[y][x]` (NOT `[x][y]`), `dark_zone: bool` on `CellFeatures`. `src/data/dungeon.rs` stays frozen.
- Plugin sibling-submodule precedent: `audio/{mod.rs, bgm.rs, sfx.rs}` is the model for `dungeon/{mod.rs, tests.rs, minimap.rs}`.

**MEDIUM-confidence (gated by Steps A/B/C in the plan):**
- `bevy_egui` 0.18.1-compatible version. Local Cargo cache has NO `bevy_egui` installed. Roadmap says `0.39` but Features #3 and #5 both deviated from their roadmap versions. Plan mandates `cargo add bevy_egui --dry-run` HALT GATE before any `Cargo.toml` edit.
- `EguiPlugin` constructor shape (unit struct vs `EguiPlugin { enable_multipass_for_primary_context: bool }`).
- `EguiContexts::ctx_mut()` return type (`&mut Context` vs `Result<&mut Context, _>`).

**Architecture recommendations (all HIGH confidence):**
- Sibling `MinimapPlugin` registered alongside `DungeonPlugin` in `main.rs` (NOT nested inside `DungeonPlugin::build`).
- `ExploredCells` is a `Resource` (not Component on PlayerParty — that entity despawns on `OnExit(Dungeon)` per `mod.rs:412-419`).
- Egui canvas painter (NOT render-to-texture for #10 — floor_01 is 6×6, ~144 segments, two orders of magnitude under egui's threshold).
- `update_explored_on_move` runs `.after(handle_dungeon_input)` — make `handle_dungeon_input` `pub(crate)` to allow the import (Pitfall 3).
- Painter systems share `MinimapSet` membership and run `.after(update_explored_on_move)` (Pitfall 10 read-after-write race).
- `ExploredCells` does NOT reset on `OnExit(GameState::Dungeon)` (preserve across re-entries).
- `show_full` toggle is `#[cfg(feature = "dev")]`-gated (security: prevent dark-zone bypass in shipping builds).

**10 pitfalls documented**, each tied to a specific guard in the plan.

**Memory updates by researcher:**
- `feedback_third_party_crate_step_a_b_c_pattern.md` — codifies the gate ritual for any future Bevy-crate addition.
- `reference_druum_dungeon_substate_and_input_state.md` — captures the pre-#10 state machine + input wiring verification so future features don't re-investigate.

### Stage 2 — Planning (run-planner skill)

Plan: `/Users/nousunio/Repos/Learnings/claude-code/druum/project/plans/20260504-150000-feature-10-auto-map-minimap.md` (Status: **Draft**)

**Structure:** Pre-pipeline action + 13 numbered steps (Step 0 + Steps A/B/C + Steps 1–9):

- **Pre-pipeline:** `git fetch origin && git pull origin main` (PR #9 was merged on GitHub; local main is behind — verified in brief).
- **Step 0:** Baseline measurement (`grep -c '#\[test\]'`, record `BASELINE_LIB`).
- **Step A:** `cargo add bevy_egui --dry-run` resolves the Bevy 0.18-compatible version. **HALT GATE** if no compatible version exists.
- **Step B:** Feature audit on resolved crate's `[features]` block. Decides D8 (defaults vs `default-features = false` with explicit minimal feature list).
- **Step C:** Grep resolved source for `EguiPlugin` shape, `EguiContexts` API, `ctx_mut()` return type, expected Cargo.lock entries (cleanest-ship signal).
- **Step 1:** Add resolved `bevy_egui` to `Cargo.toml` (one line).
- **Step 2:** Register `EguiPlugin` in `src/main.rs`.
- **Step 3:** Create `src/plugins/dungeon/minimap.rs` skeleton (`MinimapPlugin`, `ExploredCells`, `ExploredState`, `MinimapSet`).
- **Step 4:** Make `handle_dungeon_input` `pub(crate)` in `dungeon/mod.rs` (Pitfall 3 prerequisite).
- **Step 5:** Implement `update_explored_on_move` subscriber + dark-zone gate + Layer 2 tests (~5).
- **Step 6:** Implement `paint_minimap_overlay` (top-right 200×200) + `paint_minimap_full` (CentralPanel) + shared `paint_floor_into` helper.
- **Step 7:** Implement `handle_map_open_close` (M toggles, Escape exits) + Layer 2b tests (~3).
- **Step 8:** `#[cfg(feature = "dev")]` `show_full` debug toggle (F8).
- **Step 9:** Final verification + diff review + manual smoke checklist for user.

**Tests:** ~+8 net new (Layer 1 ~3 helper, Layer 2 ~5 subscriber/dark-zone/show_full, Layer 2b ~3 open/close), 0 integration tests. Final verification asserts `BASELINE_LIB + 8` (±1).

**Verification:** all 7 commands must pass with zero warnings + `cargo audit` zero advisories on `bevy_egui` tree + `git diff --stat` final check (Cargo.toml +1, Cargo.lock egui tree only, `minimap.rs` new, `mod.rs` += `pub mod minimap;` + `pub(crate)` change, `main.rs` += 3 lines for `use` + `EguiPlugin` + `MinimapPlugin`). NO other files touched. + manual smoke checklist (8 items deferred to user).

**Atomic commit boundaries:** 8 commits, one per logical step. Each commit must compile + `cargo test` must pass.

**Cleanest-ship signal for #10:** Δ deps = +1 (`bevy_egui`). NOT Δ deps = 0 — the new bar is "Cargo.toml += 1 line, Cargo.lock += quantified egui tree, no unrelated transitive bumps."

**Memory updates by planner:**
- `project_druum_minimap.md` — Feature #10 architectural decisions for #11/#12/#20/#23/#25 downstream.
- Index updated.

## User Decisions Required (D1–D7)

The plan's `Open Questions` section lists 7 Category C decisions blocking implementer dispatch. Each has the research recommendation marked. **Per the user's `feedback_user_answers_to_options.md` memory: prose answers will trigger one clarifying question — letter-only answers (A/B/C) are preferred.**

- **D1 — Plugin module structure:** A (sibling MinimapPlugin in main.rs — RECOMMENDED) / B (nested in DungeonPlugin::build) / C (`src/plugins/ui/minimap.rs`)
- **D2 — Where ExploredCells lives:** A (Resource — RECOMMENDED) / B (Component on PlayerParty — argued against: party despawns) / C (lazy from history — argued against: O(history))
- **D3 — Canvas vs RTT:** A (egui canvas — RECOMMENDED for #10) / B (render-to-texture)
- **D4 — Overlay placement + size:** A (top-right 200×200 no background — RECOMMENDED) / B (bottom corner) / C (toggleable corner — deferred to #25)
- **D5 — `OpenMap` toggle behavior:** A (M toggles + Escape exits — RECOMMENDED) / B (M only) / C (M-open Escape-close-only)
- **D6 — Visited semantics:** A (destination cell only — RECOMMENDED for v1) / B (destination + adjacent line-of-sight — deferred to #25)
- **D7 — `KnownByOther` provenance:** A (declare variant + slight tint, no v1 producer — RECOMMENDED) / B (don't add until #12/#20) / C (richer enum upfront — YAGNI)

**D8 (`bevy_egui` features opt-out)** is NOT user-facing — Step B's feature audit decides it (defaults vs `default-features = false` with explicit minimal feature list). Resolved inline during implementation.

## Pre-Implementation Action Items

Before parent dispatches implementer, the following must happen:

1. **User answers D1–D7** (letter answers preferred per memory; prose answers trigger one clarifying question).
2. **Local main brought up to date:** `cd /Users/nousunio/Repos/Learnings/claude-code/druum && git fetch origin && git log main..origin/main --oneline` (expect at least the PR #9 merge commit `7235274`); then `git checkout main && git pull origin main`. Branching for #10 must happen from up-to-date main.
3. **Branch:** `git checkout -b ja-feature-10-auto-map-minimap` (suggested name; user may override).
4. **Implementer dispatched manually** by parent with the plan path and the D1–D7 resolutions baked into the prompt (`SendMessage` does not actually resume returned agents — established Feature #3-#9 pattern).

## Implementation Notes for Parent / Implementer

- **First feature to add a non-trivial dep since Feature #5.** Extra care on the manifest diff is warranted. Step C quantifies expected `Cargo.lock` entries before the edit; the final `git diff Cargo.lock` review compares actual against expected. Any unrelated entry (e.g., a `bevy_render` patch bump from upward unification) STOPS implementation for investigation.
- **Halt-and-escalate gates in plan:** Step A (no Bevy 0.18-compatible bevy_egui version), Step 1 (`cargo check` errors after dep add), Step 9 (final diff has unrelated files).
- **Test count baseline mismatch:** brief says 68 lib + 3 integration default / 69 lib + 3 integration with dev. Memory from #9 says 69 lib + 3 integration. Plan's Step 0 takes a fresh `grep -c '#\[test\]'` reading and uses that as `BASELINE_LIB`. Final assertion is `BASELINE_LIB + ~8` (±1), not against the brief's value.
- **`#[cfg(feature = "dev")] init_resource::<ButtonInput<KeyCode>>()` gotcha** still applies for App-level tests when `--features dev` is on (carryover from Features #7-#9).
- **`src/data/dungeon.rs` is FROZEN.** Plan reads floor data through existing public API only. If implementer hits a need for schema change, STOP and surface as a question.
- **Plan does NOT pre-resolve user-facing Category B decisions.** D1–D7 must come back from user before implementation. The plan structure is "decisions inserted by parent in implementer prompt" — implementer treats them as locked.

## Pipeline Scope Met

This run executed:
- Research (Stage 1) — done
- Plan (Stage 2) — done
- Pipeline summary (this file) — done
- Implementation, ship, code review — INTENTIONALLY out of scope per brief.

## Artifacts Index

- **Research:** `/Users/nousunio/Repos/Learnings/claude-code/druum/project/research/20260504-130000-feature-10-auto-map-minimap.md`
- **Plan:** `/Users/nousunio/Repos/Learnings/claude-code/druum/project/plans/20260504-150000-feature-10-auto-map-minimap.md` (Status: Draft, awaiting D1–D7)
- **Pipeline state:** `/Users/nousunio/Repos/Learnings/claude-code/druum/project/orchestrator/PIPELINE-STATE.md` (Status: plan-approval pending)
- **Roadmap (source of truth):** `/Users/nousunio/Repos/Learnings/claude-code/druum/project/roadmaps/20260429-01-bevy-dungeon-crawler-roadmap.md` §10 (lines 538–587)
- **Predecessor (Feature #9):** `/Users/nousunio/Repos/Learnings/claude-code/druum/project/implemented/20260504-050000-feature-9-dungeon-lighting-atmosphere.md`
- **Master research (architecture context):** `/Users/nousunio/Repos/Learnings/claude-code/druum/research/20260326-01-bevy-first-person-dungeon-crawler-rpg.md`
