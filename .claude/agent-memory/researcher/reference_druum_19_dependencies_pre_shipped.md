---
name: reference-druum-19-dependencies-pre-shipped
description: Feature #19 (Character Creation & Class Progression) — what is already in tree vs new from #19; XP hook location pinned; RNG pattern canonical
metadata:
  type: reference
---

Druum Feature #19 (Character Creation & Class Progression) lands with **far more scaffolding pre-shipped than the roadmap "+500-900 LOC" suggests**. Verified by direct file inspection 2026-05-13.

**Already in tree (verified line numbers):**
- `Race` enum — `src/plugins/party/character.rs:39-46`. All 5 variants declared (Human/Elf/Dwarf/Gnome/Hobbit). Serde + Reflect derived. Discriminant order locked.
- `Class` enum — `src/plugins/party/character.rs:62-72`. All 8 variants declared; only Fighter/Mage/Priest authored in RON.
- `BaseStats` (6 u16 stats), `DerivedStats`, `Experience` with cached `xp_to_next_level`, `PartyMemberBundle` — all in `character.rs`.
- `ClassDef` with `starting_stats`, `growth_per_level`, `hp_per_level`, `mp_per_level`, `xp_to_level_2`, `xp_curve_factor` — `src/data/classes.rs:50-69`.
- `assets/classes/core.classes.ron` — Fighter/Mage/Priest fully authored (lines 1-79).
- `RecruitPool` (Vec<RecruitDef>) + `RecruitedSet` (HashSet<usize>) + `MAX_RECRUIT_POOL=32` — `src/data/town.rs:46,133-152`, `src/plugins/town/guild.rs:87-89`.
- `handle_guild_recruit` — `src/plugins/town/guild.rs:307-390` — spawns PartyMemberBundle from a RecruitDef.
- RNG pattern: `rand 0.9` direct dep + `rand_chacha 0.9` dev-dep (`Cargo.toml:37,40`); pure fns take `rng: &mut (impl rand::Rng + ?Sized)`; production wraps `Box<dyn rand::RngCore + Send + Sync>`; tests use `ChaCha8Rng::seed_from_u64(seed)`. Examples: `data/encounters.rs:78-96`, `combat/damage.rs:62-67`, `combat/encounter.rs:91-96`.

**NOT yet in tree — Feature #19 builds:**
- `RaceData` / `RaceTable` asset — does not exist anywhere. Race enum is pure discriminant.
- `progression.rs` — new file. `level_up`, `xp_for_level`, `roll_bonus_pool`, `allocate_bonus_pool`, `can_create_class`, `can_change_class` are all new pure functions.
- Combat XP hook — verified at `src/plugins/combat/turn_manager.rs:634-638`. The `// 3. Victory.` branch is currently just `next_state.set(GameState::Dungeon)` with NO XP code. The hook insertion is a 5-LOC edit to emit a new `CombatVictoryEvent` message.
- `CombatVictoryEvent` — new Message type, consumed by `award_combat_xp` system in progression.
- Class-change graph data (`advancement_requirements`, `min_stats`, `allowed_races`, `bonus_pool_min/max`) — `ClassDef` extension needed.
- egui multi-step wizard — `GuildMode` enum extension with `CreateRace/CreateClass/CreateRoll/CreateAllocate/CreateName/CreateConfirm` variants + `CreationDraft` resource.

**Critical design decisions for #19:**
- Creation destination = `RecruitPool.recruits.push(...)` (NOT direct spawn). Reuses `handle_guild_recruit` byte-for-byte. Forward-compat anticipated in `guild.rs:14-19`.
- `ClassTable::get` returns `Option<&ClassDef>` — UI must filter authored-only classes. Never write exhaustive `match Class { ... }` without wildcard.
- Day-one MVP: 3 classes (Fighter/Mage/Priest). Class-change UI deferred but `advancement_requirements` data field shipped day-one.
- Race modifiers: cleanest MVP is "modify bonus-pool roll range, NOT starting_stats" — avoids u16-underflow pitfall.
- Δ Cargo.toml = 0. No new direct deps needed.

LOC estimate: ~835 (270 progression.rs + 280 guild_create.rs + 75 races.rs + 50 RON race table + 40 classes.rs extension + 30 classes.ron extension + 10 loading + 25 town/mod + 30 guild.rs + 10 turn_manager + 15 party/mod). Fits roadmap budget of 500-900.

See [[reference-druum-18b-dependencies-pre-shipped]] for the parallel pre-flight inventory of #18b — same scaffolding-discovery pattern.
