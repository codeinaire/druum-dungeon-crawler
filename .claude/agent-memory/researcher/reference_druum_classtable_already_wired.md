---
name: Druum ClassTable + asset path already wired since Feature #3
description: Feature #11 (Party & Character ECS) inherits a fully-wired ClassTable stub from #3 — type exists at src/data/classes.rs, RonAssetPlugin registered in LoadingPlugin:100, asset stub at assets/classes/core.classes.ron, Handle field in DungeonAssets — only the body needs filling
type: project
---

When planning or implementing Feature #11 (Party & Character ECS Model), `ClassTable` is NOT new infrastructure — it's a 3-line stub from Feature #3 that `LoadingPlugin` already loads. The Feature #11 deliverable for the asset side is to flesh out the type, not to create the loader plumbing.

**Why:** Feature #3 (asset pipeline) preemptively wired stub `Asset` types for every later feature so the loader spec was complete from day one. This is a deliberate "freeze the loader, evolve the data" pattern. Roadmap §11 line 631 says "Author classes.ron" — that's misleading; the file already exists with `()` body.

**How to apply:**

The roadmap path `assets/data/classes.ron` is wrong. The actual path locked-in by `RonAssetPlugin::<ClassTable>::new(&["classes.ron"])` registration at `src/plugins/loading/mod.rs:100` and the existing stub at `assets/classes/core.classes.ron` is what the loader expects.

Feature #11's asset-side work:
- `src/data/classes.rs` — replace stub body with real `ClassTable { classes: Vec<ClassDef> }` schema; treat as a freeze-from-day-one file like `src/data/dungeon.rs`.
- `assets/classes/core.classes.ron` — replace `()` body with the 3-class data per Decision 8.
- DO NOT touch `src/plugins/loading/mod.rs` — RonAssetPlugin registration and DungeonAssets's `class_table: Handle<ClassTable>` field are already in place.
- DO NOT add a second loader for a different path.

Same pattern applies to `src/data/{items.rs, enemies.rs, spells.rs}` — all stubs from #3 with their Handle fields already in `DungeonAssets`. When Features #12 (items), #15 (enemies), #20 (spells) land, they will follow the same "flesh out the type, freeze from day one" trajectory.

`PartyPlugin` follows the same pre-wired pattern: `src/plugins/party/mod.rs` is an empty stub registered in `src/main.rs:32`. Feature #11 fleshes out the plugin body; src/main.rs stays byte-unchanged (cleanest-ship signal, same as #9 + #10).

This precedent matters for ALL future features that touch a #3-stubbed type or a pre-wired plugin.
