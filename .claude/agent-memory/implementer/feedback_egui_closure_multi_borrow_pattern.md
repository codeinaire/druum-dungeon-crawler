---
name: egui-closure-multi-borrow-pattern
description: ResMut + Query refs inside egui FnOnce closure — compute state outside, pass owned data in
metadata:
  type: feedback
---

Compute display state OUTSIDE the `egui::Window::show(ctx, |ui| {...})` closure when the computation requires `ResMut<T>` or `Query<&T>` borrows. The closure is `FnOnce` and captures by move/reborrow; combining `ResMut` mutation with Query borrows inside `FnOnce` triggers borrow-checker errors.

**Why:** `ResMut<WarnedMissingSpells>` + `Query<&KnownSpells>` inside `egui::Window::show` closure failed to compile in Feature #20 Phase 3 SpellMenu painter.

**How to apply:** Define a local enum (e.g., `SpellMenuState`) that captures the fully-resolved display state as owned values (`Vec<SpellAsset>`, etc.). Compute it outside the closure using the `Res`/`ResMut`/`Query` params, then match on it inside the closure. The closure only captures `&menu_state` — no resource or query borrows cross the closure boundary.

Pattern:
```rust
enum MyDisplayState { /* owned variants */ }
let display_state = compute_from_resources_and_queries(...);
egui::Window::new("...").show(ctx, |ui| {
    match &display_state { /* render only */ }
});
```
