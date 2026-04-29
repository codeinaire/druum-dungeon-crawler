---
name: Druum project overview
description: First-person grid-based dungeon crawler RPG built in Rust with Bevy engine, Wizardry/Etrian Odyssey style
type: project
---

Druum is a first-person dungeon crawler RPG project in the style of Wizardry, Etrian Odyssey, and Legend of Grimrock.

**Why:** The user wants to build a classic DRPG (dungeon RPG) with grid-based movement, turn-based combat, party management, and multi-floor dungeons.

**How to apply:** All technical decisions should prioritize DRPG genre conventions. The recommended architecture is 3D dungeon geometry + 2D billboard sprites for enemies, using Bevy 0.18.x. Research document at `research/20260326-01-bevy-first-person-dungeon-crawler-rpg.md` contains full stack recommendations and architectural patterns.
