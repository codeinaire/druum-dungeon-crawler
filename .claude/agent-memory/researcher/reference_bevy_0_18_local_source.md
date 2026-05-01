---
name: Bevy 0.18.1 local source extraction
description: The full Bevy 0.18.1 crate family is extracted on disk under ~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/, enabling HIGH-confidence verification of any Bevy 0.18 API claim without web/MCP access
type: reference
---

When researching Druum (Bevy 0.18.1) features under tooling limitations (no Bash, no MCP, no WebFetch, no WebSearch), the local Cargo registry is the authoritative source.

**Path:** `/Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/`

**Crates extracted (full 0.18.1 family):**
- `bevy-0.18.1/` — umbrella crate with examples (`examples/asset/*.rs`, `examples/state/*.rs`, etc.)
- `bevy_internal-0.18.1/`
- `bevy_app-0.18.1/`, `bevy_ecs-0.18.1/`, `bevy_reflect-0.18.1/`, `bevy_state-0.18.1/`
- `bevy_asset-0.18.1/`, `bevy_asset_macros-0.18.1/`
- `bevy_ui-0.18.1/`, `bevy_ui_render-0.18.1/`, `bevy_ui_widgets-0.18.1/`
- `bevy_text-0.18.1/`, `bevy_image-0.18.1/`
- `bevy_state_macros-0.18.1/`, `bevy_reflect_derive-0.18.1/`
- `bevy_input-0.18.1/`, `bevy_log-0.18.1/`
- `bevy_render-0.18.1/`, `bevy_pbr-0.18.1/`, `bevy_camera-0.18.1/`, `bevy_mesh-0.18.1/`
- `bevy_animation-0.18.1/`, `bevy_audio-0.18.1/`, `bevy_winit-0.18.1/`, `bevy_window-0.18.1/`
- ...plus many more (transitive Bevy deps)

**What's NOT extracted (gaps that need verification recipes when relevant):**
- Third-party crates the project does not yet depend on:
  - `bevy_common_assets`, `bevy_asset_loader` (Feature #3)
  - `leafwing-input-manager` (Feature #5)
  - `bevy_kira_audio` (Feature #6)
  - `bevy_egui` (later features)
  - `moonshine-save` (Feature #23)
- The umbrella `ron 0.12.1` crate is in `Cargo.lock` but not extracted

**How to apply:**
- For any Bevy 0.18 first-party API claim, grep the local source — line numbers and file paths give HIGH confidence.
- For third-party crates, fall back to training data (MEDIUM) and provide a verification recipe (curl/cargo add) for the planner/implementer.
- Cite paths inline like `bevy_asset-0.18.1/src/lib.rs:248` so readers can verify.
- Bevy's own examples at `bevy-0.18.1/examples/<area>/<topic>.rs` are canonical "this is how you do it in 0.18" reference patterns.

This is the SAME pattern used for Feature #1 and Feature #2 research; both produced HIGH-confidence research docs that survived implementation review.
