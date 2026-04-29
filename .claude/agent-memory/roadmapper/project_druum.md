---
name: Druum project context
description: Greenfield Bevy 0.18.1 first-person dungeon crawler RPG (Wizardry / Etrian Odyssey style); architectural commitments and risk areas
type: project
---

The `druum` project is a greenfield Rust game built in Bevy 0.18.1, targeting a first-person grid-based dungeon crawler RPG in the tradition of Wizardry / Etrian Odyssey / Legend of Grimrock. As of 2026-04-29 the repo contains only a research document (`research/20260326-01-bevy-first-person-dungeon-crawler-rpg.md`) and the first roadmap (`project/roadmaps/20260429-01-bevy-dungeon-crawler-roadmap.md`) — no source code yet.

**Why:** Project scope and architectural commitments locked in by the research doc shape how every later feature should be analyzed. Knowing these prevents re-deriving them and avoids accidentally proposing options the research already eliminated.

**How to apply:**
- The research locks in: Bevy 0.18.1 (pinned via `=0.18.1`), plugin-per-subsystem architecture, `bevy_egui` for menus (not bevy_ui), Option B rendering (3D dungeon geometry + billboard enemy sprites), razor-wall grid representation in RON, `leafwing-input-manager` + `bevy_kira_audio`. Don't propose Unity, Godot, or alternative engines unless the user explicitly asks to revisit.
- Highest-risk areas (per research): Bevy version churn (#Pitfall 1), UI complexity (#Pitfall 2), no editor (#Pitfall 3), encounter rate tuning (#Pitfall 4), save system architectural debt (#Pitfall 5), RPG balance combinatorial explosion (#Pitfall 6). Any roadmap or feature plan should explicitly mitigate the relevant pitfall(s).
- Open questions still unresolved: bevy_sprite3d 0.18 compat verification, save crate choice (moonshine-save vs bevy_save vs custom), auto-map render approach (egui canvas vs render-to-texture), art pipeline for billboards (single-facing vs directional).
- Genre commitments: party of 4-6, front/back row, 8 classes (Wizardry-style), turn-based combat with action queue sorted by speed, town hub (shop/inn/temple/guild), random encounters + FOEs, multi-floor dungeon with traps/teleporters/spinners/secret walls.
- The user's first roadmap was structured as 25 features ordered by difficulty + dependency, with a recommended first sprint of #1 Skeleton, #2 State Machine, #3 Asset Pipeline, #4 Grid Data Model, #7 Movement — the sprint that produces "a walkable corridor". Future planning should respect this validated ordering unless the project's needs have shifted.
