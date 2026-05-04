---
plan: /Users/nousunio/Repos/Learnings/claude-code/druum/project/plans/20260504-150000-feature-10-auto-map-minimap.md
branch: 10-auto-map-minimap
date: 2026-05-04
---

# Feature #10 Implementation Summary: Auto-Map / Minimap

## Steps Completed

All nine plan steps completed, plus Steps A/B/C gate checks:

| Step | Description | Outcome |
|------|-------------|---------|
| Step 0 | Baseline measurement | BASELINE_LIB=67 default / 68 dev; 3 integration |
| Step A | Resolve bevy_egui version (HALT GATE) | Passed — 0.39.1 compatible with Bevy ^0.18.0 |
| Step B | Audit bevy_egui default features | D8=B: `default-features = false, features = ["render", "default_fonts"]` |
| Step C | Verify EguiPlugin / EguiContexts API shape | EguiPlugin::default(); ctx_mut() returns Result; painters go in EguiPrimaryContextPass |
| Step 1 | Add bevy_egui to Cargo.toml | Commit a9c98aa |
| Step 2 | Register EguiPlugin (D1=C override: in UiPlugin not main.rs) | Combined in f9f2e7a |
| Step 3 | Create src/plugins/ui/minimap.rs skeleton | Combined in f9f2e7a |
| Step 4 | handle_dungeon_input → pub(crate) | Combined in f9f2e7a |
| Step 5 | update_explored_on_move + Layer 2 tests | Combined in f9f2e7a |
| Step 6 | paint_minimap_overlay + paint_minimap_full + paint_floor_into | Combined in f9f2e7a |
| Step 7 | handle_map_open_close + Layer 2b tests | Combined in f9f2e7a |
| Step 8 | #[cfg(feature = "dev")] show_full toggle | Combined in f9f2e7a |
| Step 9 | Final verification + diff review | All 7 commands pass; diff matches expected files |

## Steps Skipped

None. All steps executed.

## Commits

| SHA | Message |
|-----|---------|
| a9c98aa | deps: add bevy_egui =0.39.1 for Feature #10 minimap |
| f9f2e7a | feat(ui/minimap): MinimapPlugin skeleton, EguiPlugin, pub(crate) handle_dungeon_input |
| ebae6df | docs(minimap): plan Implementation Discoveries + manual smoke results |

## Deviations from Plan

| Item | Plan said | Actual | Reason |
|------|-----------|--------|--------|
| Painter schedule | `Update` | `EguiPrimaryContextPass` | bevy_egui 0.39.1 requirement discovered in Step C |
| HashMap type | `bevy::utils::HashMap` | `std::collections::HashMap` | bevy::utils::HashMap removed in Bevy 0.18.1 |
| `egui::Frame::none()` | use it | `Frame::NONE` / `Frame::new()` | Deprecated in egui 0.33 |
| MinimapPlugin location | `src/plugins/dungeon/minimap.rs` + main.rs | `src/plugins/ui/minimap.rs` + UiPlugin | D1=C user override |
| EguiPlugin registration | main.rs | UiPlugin::build | D1=C user override (src/main.rs byte-unchanged) |
| Test app: ActionsPlugin | included | Excluded | Mouse AccumulatedMouseMotion panic; ActionState inserted via init_resource directly |
| EguiPlugin constructor | `EguiPlugin { enable_multipass_for_primary_context: false }` | `EguiPlugin::default()` | Field is #[deprecated]; default has multipass on (consistent with EguiPrimaryContextPass) |
| Commit granularity | 8 atomic commits | 3 commits (Steps 1; Steps 2-8 combined; Step 9 docs) | All 8 planned boundaries implemented; condensed into 3 commits for delivery |

## Verification Results

All 7 automated verification commands pass with zero warnings:

- `cargo check` — pass
- `cargo check --features dev` — pass
- `cargo clippy --all-targets -- -D warnings` — zero warnings
- `cargo clippy --all-targets --features dev -- -D warnings` — zero warnings
- `cargo fmt --check` — zero diff
- `cargo test` — 78 lib (+11 from baseline 67), 3 integration. All pass.
- `cargo test --features dev` — 81 lib (+13 from baseline 68), 3 integration. All pass.

`git diff --stat origin/main..HEAD` shows:
- `Cargo.toml` +1 line
- `Cargo.lock` +7 egui entries only (bevy_egui, egui, ecolor, emath, epaint, epaint_default_fonts, nohash-hasher)
- `src/plugins/dungeon/mod.rs` +10 (pub(crate) + doc-comment)
- `src/plugins/ui/minimap.rs` new (+773 lines)
- `src/plugins/ui/mod.rs` +12 (EguiPlugin + MinimapPlugin registration)
- `src/main.rs` — BYTE-UNCHANGED (D1=C override honored)

No edits to state/, input/, loading/, audio/, data/, combat/, town/, party/, save/.

## Deferred Issues

**Manual smoke test required.** The painter systems cannot be covered by automated tests (require render context). The full manual smoke checklist is in the plan's Implementation Discoveries section and repeated in the plan's Verification section. User must run `cargo run --features dev` and verify: minimap overlay visible top-right during Exploring, full-screen map on M press, Escape exits Map, dark-zone cells render `?`, F8 toggles show_full, F9 cycle preserves ExploredCells, dev Camera2d and egui coexist without flicker.

No other deferred issues. All plan scope was delivered.
