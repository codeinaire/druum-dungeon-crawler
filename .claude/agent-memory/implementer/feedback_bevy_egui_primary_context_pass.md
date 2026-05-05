---
name: bevy_egui 0.39.x — UI painter systems must use EguiPrimaryContextPass schedule
description: bevy_egui 0.39.x requires egui drawing systems in EguiPrimaryContextPass, not Update; non-drawing systems (event handlers, state updaters) stay in Update
type: feedback
---

Use `EguiPrimaryContextPass` schedule for any system that calls `EguiContexts::ctx_mut()` and paints egui UI. Non-painter systems (subscribers, input handlers, state transitions) continue to use `Update`.

**Why:** bevy_egui 0.39.x introduced a multipass rendering pipeline. The `EguiPrimaryContextPass` schedule is the correct hook for primary-window UI drawing. Systems registered in `Update` that attempt to draw via `EguiContexts` will silently skip or panic depending on context availability.

**How to apply:** Any time `EguiContexts` is a system parameter, register that system in `EguiPrimaryContextPass`. Systems that only read game state or fire input events continue to use `Update`. The two schedules are naturally ordered by Bevy's main schedule pipeline — no explicit ordering needed between them.

Additional API facts for bevy_egui 0.39.1:
- `EguiPlugin::default()` — use this, NOT `EguiPlugin { enable_multipass_for_primary_context: false }` (field is `#[deprecated]`)
- `EguiContexts::ctx_mut()` returns `Result<&mut egui::Context, QuerySingleError>` — painter systems must return `-> Result` and use `let ctx = contexts.ctx_mut()?;`
- `egui::Frame::none()` is deprecated — use `egui::Frame::NONE` (const) for transparent frames, `egui::Frame::new()` for a builder
