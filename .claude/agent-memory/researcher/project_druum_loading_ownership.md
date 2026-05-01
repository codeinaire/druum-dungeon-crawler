---
name: Druum loading-state ownership constraint
description: Feature #3 (Asset Pipeline) is the sole owner of the `GameState::Loading -> TitleScreen` transition; no other plugin auto-advances from Loading
type: project
---

`GameState::Loading` is the default game state and intentionally has NO auto-advance from Feature #2 (state machine). The F9 dev hotkey is the only way out in Feature #2.

**Why:** Feature #3 (asset/RON pipeline) owns the `Loading → TitleScreen` transition exclusively. The transition fires when `bevy_asset_loader::LoadingState` confirms all assets in the `DungeonAssets` collection are loaded-with-dependencies. This was a deliberate decision baked into Feature #2's plan to avoid cross-feature coupling on the loading sequence.

**How to apply:**
- When researching ANY future feature, if it touches `OnEnter(GameState::Loading)` or `next_state.set(GameState::TitleScreen)`, flag it as a violation of this constraint and route the request through `LoadingPlugin` instead.
- Save/load (Feature #23) may want to re-enter `Loading` while restoring a save — that's allowed (it's RE-entering, not auto-advancing). The `Loading → TitleScreen` exit path is the constrained one.
- If a future feature genuinely needs to gate something on "all assets are loaded", it should add to `DungeonAssets` (the existing collection in `LoadingPlugin`) rather than spawn a parallel collection or a parallel transition.

Source: `.claude/agent-memory/planner/project_druum_state_machine.md` (Feature #2 planner decision, 2026-04-29).
