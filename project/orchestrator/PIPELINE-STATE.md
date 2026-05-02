# Pipeline State

**Task:** Drive the full pipeline (research → plan) for Feature #5: Input System (leafwing) from the dungeon crawler roadmap. Add `leafwing-input-manager` (pinned `=` after crates.io Bevy 0.18 compat verification — same fail-stop gate as Features #3/#4 had for new deps), define abstract `Action` enums per game context (`MenuAction`, `DungeonAction`, `CombatAction`, possibly `TownAction`), wire `InputManagerPlugin::<T>` into the appropriate plugin(s), provide `Res<ActionState<T>>` query path for downstream features, decide F9 cycler (keep direct `ButtonInput<KeyCode>` vs refactor to `DevAction::CycleGameState`) and document rationale, write tests using leafwing's press-simulation helpers. Keyboard only for v1; gamepad deferred. PAUSE at plan-approval; parent dispatches implementer manually because `SendMessage` does not actually resume returned agents (confirmed during Features #3 and #4).
**Status:** completed (with 1 MEDIUM follow-up)
**Last Completed Step:** 5

## Artifacts

| Step | Description | Artifact                                 |
| ---- | ----------- | ---------------------------------------- |
| 1    | Research    | /Users/nousunio/Repos/Learnings/claude-code/druum/project/research/20260501-235900-feature-5-input-system-leafwing.md |
| 2    | Plan        | /Users/nousunio/Repos/Learnings/claude-code/druum/project/plans/20260502-000000-feature-5-input-system-leafwing.md |
| 3    | Implement   | /Users/nousunio/Repos/Learnings/claude-code/druum/project/implemented/20260502-000000-feature-5-input-system-leafwing.md |
| 4    | Ship        | https://github.com/codeinaire/druum-dungeon-crawler/pull/5 (branch `5-input-system-leafwing`, commit `2edf71a`) |
| 5    | Code Review | /Users/nousunio/Repos/Learnings/claude-code/druum/project/reviews/20260502-000000-leafwing-input-manager-feature-5.md (verdict: WARNING — 1 MEDIUM, doc-only) |

## Implementation Notes (Step 3)

User approved the plan with both defaults intact (`DevAction` deferred, `Interact = F`) on 2026-05-02. Implementer ran the full plan with three minor deviations, all reasonable:

1. **`init_resource::<ActionState<T>>()` added to `ActionsPlugin::build`** — caught by Step 3's API verification gate. `InputManagerPlugin 0.20.0` does NOT auto-insert `ActionState` as a Resource. Three `init_resource` calls added per action enum.
2. **Tests use `KeyCode::X.press(app.world_mut())` directly** — leafwing 0.20.0's `Buttonlike` trait provides `press` on `KeyCode`, writing `Messages<KeyboardInput>` internally. Cleaner than the plan's `inject_key_press` helper template; same mechanism underneath.
3. **Steps 5 and 6 executed as one** — skeleton and filled InputMaps written together once API was verified.

Verification: all 6 commands passed with zero warnings. **33 tests default / 34 with `--features dev`** (5 new from this feature). `cargo audit` deferred (`cargo-audit` not installed locally).

Resolved leafwing version: **`leafwing-input-manager = "=0.20.0"`** with `default-features = false, features = ["keyboard", "mouse"]`. Bevy requirement `"0.18.0-rc.2"` accepts our `=0.18.1`. Step 1 fail-stop gate PASSED.

**Cargo.lock scope check** confirmed clean — 7 new entries (`leafwing-input-manager`, `leafwing_input_manager_macros`, `dyn-clone`, `dyn-eq`, `dyn-hash`, `enumn`, `serde_flexitos`); no version bumps to existing pinned deps.

LOC: `src/plugins/input/mod.rs` 299 lines (above the 185-205 estimate; extra is doc comments + test bodies — within roadmap budget).

## Research Summary

Research is HIGH-confidence on all Bevy 0.18.1 first-party APIs (verified against on-disk source: `Plugin` trait has no `run_if`, `KeyboardInput` is a `Message` not `Event`, `keyboard_input_system` clears `ButtonInput<KeyCode>` at `PreUpdate` start, `Messages<E>` lives at `bevy_ecs-0.18.1/src/message/messages.rs:95`, KeyboardInput has 6 fields). MEDIUM-confidence on leafwing-specifics (training-data only — Cargo registry has no extracted leafwing crate to grep), gated behind a Step A `cargo add --dry-run` verification recipe that the planner codified into Step 1 of the plan.

Top recommendations (all adopted by the plan):
- Plugin location: `src/plugins/input/mod.rs`, plugin name `ActionsPlugin` (not `InputPlugin` — would collide with `bevy::input::InputPlugin`).
- Per-context action enums (`MenuAction`, `DungeonAction`, `CombatAction`); `MenuAction` reused for Town v1.
- `InputMap<T>` as `Resource` (not per-entity `Component`).
- Centralized `InputManagerPlugin::<T>` registrations in `ActionsPlugin`.
- F9 cycler stays direct on `Res<ButtonInput<KeyCode>>` (Option A — research §RQ7).
- Arrow keys strafe (modern convention) — research C9.
- Tests use full `InputPlugin` + `KeyboardInput` message injection (NOT the Feature #2 F9-test bypass pattern — that's the OPPOSITE of what action-state tests need).
- Roadmap line 314's `.run_if(in_state(...))` on the plugin call DOES NOT COMPILE — flagged as anti-pattern.

## Plan Summary

9 commit-ordered steps with three fail-stop verification gates BEFORE any project file is touched:

- **Step 1:** Run `cargo add leafwing-input-manager --dry-run` and verify resolved version's `bevy = "..."` requirement accepts `=0.18.1`. HALT and escalate to user if no compatible release (same playbook as `moonshine-save` swap in Feature #3).
- **Step 2:** `cargo info leafwing-input-manager --version <ver>` to audit feature flags; pick minimal set (likely keyboard+mouse defaults; opt out of `egui`/`asset` if present).
- **Step 3:** `grep` resolved crate to verify `Actionlike` derive shape, `InputMap` builder, `ActionState::just_pressed` arg shape, `MockInput` existence.
- **Step 4:** Edit Cargo.toml — add `leafwing-input-manager = "=<resolved-version>"` (no extra features unless Step 2 mandates).
- **Step 5:** Create `src/plugins/input/mod.rs` with the three action enums + `ActionsPlugin` skeleton.
- **Step 6:** Fill `default_*_input_map()` bodies with keyboard bindings (WASD+arrows for movement, Q/E for turn, F for Interact, Tab for Inventory, M for Map, Esc for Pause/Cancel, Enter for Confirm).
- **Step 7:** Smoke test `actions_plugin_registers_all_inputmaps`.
- **Step 8:** Four `KeyboardInput`-message-injection tests (W→MoveForward, ArrowUp→MoveForward, Escape→Cancel, Enter→Confirm).
- **Step 9:** Final 6-command verification matrix + Cargo.lock diff scope check + F9 test unchanged.

Estimated LOC: +185-205 (within the +120-200 roadmap budget; on the high end due to comprehensive 5-test coverage and the F9/DevAction carve-out documentation).

Plan defers `DevAction` (no v1 callers; six cfg-gating points for zero value) and resolves `Interact = F` (avoids TurnRight=E conflict and Confirm=Space muscle-memory bleed). Both surfaced as Open Questions for user awareness.

## User Decisions

**Plan-approval checkpoint is now active.** Two genuine residual decisions surfaced for user awareness; both have defensible defaults baked into the plan:

1. **`DevAction` lands in v1 or defers? — DEFERRED in plan.** A placeholder enum adds 6 cfg-gating points for zero v1 callers. First leafwing-routed dev hotkey introduces `DevAction` naturally at that time.
2. **`Interact` keybinding? — `F` in plan.** Avoids the `TurnRight = E` conflict; `Space` would overload Confirm in MenuAction/CombatAction.

Pipeline pauses here per task instructions. Resume from Step 3 (Implement) once the user approves the plan in the parent session.
