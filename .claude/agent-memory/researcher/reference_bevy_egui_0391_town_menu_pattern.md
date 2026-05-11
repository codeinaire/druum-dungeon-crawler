---
name: bevy_egui 0.39.1 multi-screen menu pattern verified for Bevy 0.18
description: bevy_egui 0.39.1 (Cargo.toml requires bevy 0.18.0) is HIGH-confidence verified on disk; the canonical pure-egui menu pattern is Camera2d spawn + PrimaryEguiContext direct-attach + per-sub-state painter system gated with run_if(in_state(...)), input handlers in Update, painters in EguiPrimaryContextPass schedule. Druum's TownPlugin (#18) will use this pattern.
type: reference
---

When building pure-egui menu screens in Druum (or any Bevy 0.18.x project) gated on a top-level GameState + a SubStates enum, the verified pattern is:

**Camera spawn (OnEnter):**
```rust
#[derive(Component)]
pub struct TownCameraRoot;  // or any state-specific marker

fn spawn_town_camera(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        TownCameraRoot,
        bevy_egui::PrimaryEguiContext,  // DIRECT ATTACH on spawn — apply_deferred makes this queryable same-frame
    ));
}

fn despawn_town_camera(mut commands: Commands, cams: Query<Entity, With<TownCameraRoot>>) {
    for e in &cams { commands.entity(e).despawn(); }
}
```

**Plugin registration:**
```rust
.add_systems(OnEnter(GameState::Town), spawn_town_camera)
.add_systems(OnExit(GameState::Town), despawn_town_camera)
.add_systems(
    bevy_egui::EguiPrimaryContextPass,  // ← painters run HERE, not Update
    (
        paint_square.run_if(in_state(TownLocation::Square)),
        paint_shop.run_if(in_state(TownLocation::Shop)),
        // ...
    ).distributive_run_if(in_state(GameState::Town)),
)
.add_systems(
    Update,  // ← input handlers run HERE
    (
        handle_square_input.run_if(in_state(TownLocation::Square)),
        handle_shop_input.run_if(in_state(TownLocation::Shop)),
        // ...
    ).distributive_run_if(in_state(GameState::Town)),
)
```

**Painter signature (read-only):**
```rust
fn paint_square(
    mut contexts: EguiContexts,
    cursor: Res<SquareMenuState>,
    gold: Res<Gold>,
) -> Result {
    let ctx = contexts.ctx_mut()?;  // ? bubbles None to Ok(()) silently — be careful if you need the error
    egui::CentralPanel::default().show(ctx, |ui| {
        ui.heading("Town Square");
        for (i, label) in MENU.iter().enumerate() {
            let color = if i == cursor.cursor { egui::Color32::YELLOW } else { egui::Color32::WHITE };
            ui.colored_label(color, label);
        }
    });
    Ok(())
}
```

**Input handler (mutating):**
```rust
fn handle_square_input(
    actions: Res<ActionState<MenuAction>>,  // leafwing
    mut cursor: ResMut<SquareMenuState>,
    mut next: ResMut<NextState<TownLocation>>,
) {
    if actions.just_pressed(&MenuAction::Down) { cursor.cursor += 1; /* clamped */ }
    if actions.just_pressed(&MenuAction::Confirm) {
        match cursor.cursor {
            0 => next.set(TownLocation::Shop),
            // ...
        }
    }
}
```

**Why painters MUST be in `EguiPrimaryContextPass`, not `Update`:** Verified at `bevy_egui-0.39.1/examples/simple.rs:9` and required by the egui multi-pass mode. `EguiPrimaryContextPass` is the schedule that wraps the egui context begin/end calls; running painters in `Update` would call `ctx.show()` on a not-yet-begun context.

**Why painters and input are split:** Painters are read-only (`Res`); input handlers write to `ResMut<NextState<T>>` and `ResMut<MenuState>`. Mixing causes one-frame UI lag and forces `ResMut` borrows in the paint critical path. Same separation precedent at `src/plugins/combat/ui_combat.rs:43-56`.

**`EguiGlobalSettings { auto_create_primary_context: false }`** must be set globally in `UiPlugin` (already done at `src/plugins/ui/mod.rs:22-25`). Without this, bevy_egui attaches `PrimaryEguiContext` to the FIRST camera spawned (which in Druum is the Loading screen Camera2d, despawned before Town's camera arrives) — leaving every later camera context-less and `ctx_mut()` silently returning `Err`.

**Canonical Druum precedents:**
- `src/plugins/combat/ui_combat.rs:1-200` — egui over `Camera3d` (`DungeonCamera`) with `SidePanel::left/right + TopBottomPanel::bottom + Window::anchor` layout. Idempotent attach via `Without<PrimaryEguiContext>` filter (because the camera is spawned via `children![...]` in OnEnter — deferred apply).
- `src/plugins/ui/minimap.rs:173-180` — same idempotent attach precedent.
- `bevy_egui-0.39.1/examples/{simple,side_panel,ui}.rs` — upstream canonical patterns.

**For Town (pure-egui Camera2d, no parent chain):** direct attach in the `commands.spawn` tuple is correct — `OnEnter` runs synchronously, apply_deferred fires before `Update`, the camera is queryable on subsequent systems.

**The two patterns are NOT interchangeable:** Camera3d parented via `children!` (dungeon) → idempotent `Update` attach. Standalone Camera2d (Town, loading screen) → direct attach in spawn.
