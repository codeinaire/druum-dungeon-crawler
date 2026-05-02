# Implementation Summary: Input System with leafwing-input-manager (Feature #5)

**Plan:** project/plans/20260502-000000-feature-5-input-system-leafwing.md
**Date:** 2026-05-02
**Status:** Complete

## Steps Completed

All 9 plan steps completed in order:

1. **Step 1 (verification gate)** — `cargo add leafwing-input-manager --dry-run` resolved v0.20.0. Registry Cargo.toml inspection confirmed `bevy = "0.18.0-rc.2"` (Cargo `^` semver: `>=0.18.0-rc.2, <0.19.0`). Our pinned `bevy = "=0.18.1"` satisfies this. Gate PASSED.

2. **Step 2 (feature flag audit)** — Defaults include `[asset, ui, mouse, keyboard, gamepad, picking]`. `asset` and `ui` are non-trivial. Opted out with `default-features = false, features = ["keyboard", "mouse"]`.

3. **Step 3 (API verification)** — Confirmed via registry source greps:
   - `Actionlike` derive: needs `PartialEq, Eq, Hash, Clone, Copy, Debug, Reflect` (macro supplies `Typed/TypePath/FromReflect` via Reflect derive)
   - `ActionState::just_pressed` takes `&A` by reference
   - `InputMap::with(action, button)` is chainable (returns `Self`)
   - `InputManagerPlugin::<T>::default()` is the standard constructor
   - No `MockInput` trait; instead `KeyCode` implements `Buttonlike::press(&self, world)` which writes `Messages<KeyboardInput>` directly
   - `ActionState<T>` is NOT auto-inserted by `InputManagerPlugin` — must call `init_resource::<ActionState<T>>()` explicitly (verified from `action_state_resource.rs` example)

4. **Step 4 (Cargo.toml)** — Added `leafwing-input-manager = { version = "=0.20.0", default-features = false, features = ["keyboard", "mouse"] }` after `bevy_asset_loader`. `cargo check` passed. Cargo.lock gained 7 new packages (leafwing-input-manager, leafwing_input_manager_macros, dyn-clone, dyn-eq, dyn-hash, enumn, serde_flexitos). No version bumps to existing packages.

5. **Step 5 (create plugin skeleton) + Step 6 (fill input maps)** — Created `src/plugins/input/mod.rs` with the 3 action enums, populated input maps, and `ActionsPlugin::build` including the plan-unspecified `init_resource::<ActionState<T>>()` calls. Updated `src/plugins/mod.rs` and `src/main.rs`.

6. **Step 7 (smoke test) + Step 8 (injection tests)** — All 5 tests written and passing. Used `KeyCode::X.press(app.world_mut())` via `Buttonlike` trait (cleaner than manual `KeyboardInput` construction).

7. **Step 9 (final verification)** — All checks pass. See verification results below.

## Steps Skipped

None.

## Deviations from the Plan

1. **`init_resource::<ActionState<T>>()` added to `ActionsPlugin::build`** — The plan's skeleton showed only `insert_resource` calls for `InputMap`. Step 3 API verification found that `InputManagerPlugin` in 0.20.0 does NOT auto-insert `ActionState` as a Resource (it only reads it via `Option<ResMut<ActionState<A>>>`). The `action_state_resource` example confirms `init_resource::<ActionState<T>>()` is required. Added 3 `init_resource` calls to `ActionsPlugin::build`.

2. **Test injection uses `KeyCode::X.press(app.world_mut())` instead of manual `KeyboardInput` construction** — Leafwing 0.20.0's `Buttonlike` trait provides a `press` method on `KeyCode` that writes the correct `Messages<KeyboardInput>` internally. This is strictly better than manually building `KeyboardInput` structs and was used throughout the tests.

3. **Steps 5 and 6 executed as one** — The plan separated skeleton creation (Step 5) from filling input maps (Step 6) to allow intermediate `cargo check`. Since both were straightforward and had no uncertainty after the API verification, they were written together in a single file creation. Both `cargo check` gates passed on the combined file.

## Verification Results

| Check | Result |
|-------|--------|
| `cargo check` | PASSED (0 warnings) |
| `cargo check --features dev` | PASSED (0 warnings) |
| `cargo clippy --all-targets -- -D warnings` | PASSED (0 warnings) |
| `cargo clippy --all-targets --features dev -- -D warnings` | PASSED (0 warnings) |
| `cargo test` (32 unit + 1 integration) | PASSED |
| `cargo test --features dev` (33 unit + 1 integration) | PASSED |
| `actions_plugin_registers_all_inputmaps` | PASSED |
| `dungeon_w_press_triggers_move_forward` | PASSED |
| `dungeon_arrow_up_also_triggers_move_forward` | PASSED |
| `menu_escape_triggers_cancel` | PASSED |
| `combat_enter_triggers_confirm` | PASSED |
| `f9_advances_game_state` (unchanged) | PASSED |
| Cargo.lock diff scope | PASSED (7 leafwing-related packages only) |
| `cargo audit` | NOT RUN (cargo-audit not installed) |

## Deferred Issues

- **`cargo audit` not verified** — `cargo-audit` is not installed in this environment. Install with `cargo install cargo-audit` and run once to check for advisories on `leafwing-input-manager 0.20.0`.

## Files Written/Modified

- `Cargo.toml` — added `leafwing-input-manager = { version = "=0.20.0", default-features = false, features = ["keyboard", "mouse"] }`
- `Cargo.lock` — updated (7 new packages: leafwing-input-manager, leafwing_input_manager_macros, dyn-clone, dyn-eq, dyn-hash, enumn, serde_flexitos)
- `src/plugins/input/mod.rs` — created (299 lines)
- `src/plugins/mod.rs` — added `pub mod input;`
- `src/main.rs` — added `input::ActionsPlugin` import and plugin registration
