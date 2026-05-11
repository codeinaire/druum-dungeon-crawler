---
name: EguiPlugin unavailable in headless tests — skip painter systems, test only Update handlers
description: EguiPlugin requires the render pipeline; MinimalPlugins tests cannot add it. Skip EguiPrimaryContextPass painters; test only Update input handlers.
type: feedback
---

`EguiPlugin::default()` panics in headless tests that use `MinimalPlugins` because it requires the Bevy render pipeline (PbrPlugin, WindowPlugin, etc.).

**Pattern for egui-based plugins (mirrors combat/ui_combat.rs):**
1. Do NOT add `EguiPlugin` in test apps.
2. Do NOT add systems that live in `EguiPrimaryContextPass` (`paint_*` functions).
3. DO add systems that live in `Update` (`handle_*_input` functions).
4. DO add `OnEnter`/`OnExit` lifecycle systems (camera spawners, etc.).
5. Verify painters manually via `cargo run --features dev` smoke test.

```rust
fn make_town_test_app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default(), StatesPlugin));
    // NO: app.add_plugins(EguiPlugin::default()); — needs render pipeline
    
    // Only register Update systems (not EguiPrimaryContextPass):
    app.add_systems(Update, handle_inn_rest.run_if(in_state(TownLocation::Inn)));
    app
}
```

**How to apply:** Whenever writing tests for a plugin that uses `EguiContexts` painters, add only the non-egui systems to the test app. Add a note in the test file explaining why painters are excluded.
