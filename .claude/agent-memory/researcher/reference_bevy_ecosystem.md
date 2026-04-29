---
name: Bevy ecosystem reference
description: Key Bevy 0.18.x crate versions, compatibility, and documentation URLs for the Druum project
type: reference
---

Bevy 0.18.1 (latest stable as of 2026-03-26). Breaking changes every ~3 months -- pin versions.

**Key crate compatibility for Bevy 0.18:**
- bevy_egui 0.39.x -- complex UI (egui immediate mode)
- leafwing-input-manager 0.18.x -- input mapping
- bevy_kira_audio 0.25.x -- audio channels/playback
- bevy_ecs_tilemap 0.18.x -- tilemap rendering (if needed)
- bevy_sprite3d 7.x -- billboard sprites in 3D (verify exact 0.18 compat)
- bevy_common_assets -- RON/JSON asset loading
- bevy_asset_loader -- declarative loading states

**Documentation:**
- Bevy official: https://bevy.org/learn/
- Bevy cheat book (unofficial but excellent): https://bevy-cheatbook.github.io/
- Bevy examples: https://bevy.org/examples/
- Migration guides: https://bevy.org/learn/migration-guides/
- Tainted Coders guides: https://taintedcoders.com/bevy/

**Key limitation to remember:** bevy_ui is inadequate for complex game menus; always use bevy_egui for RPG UI screens. Bevy has no editor -- build custom tooling or use data-driven RON files with hot-reload.
