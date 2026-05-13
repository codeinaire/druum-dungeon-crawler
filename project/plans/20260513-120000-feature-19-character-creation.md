# Plan: Feature #19 — Character Creation & Class Progression

**Date:** 2026-05-13
**Status:** Complete
**Research:** project/research/20260513-feature-19-character-creation.md
**Depends on:** 20260512-173000-feature-18b-town-temple-guild.md

## Goal

Ship a Guild-driven character creation flow (Race → Class → Stat-Roll → Name → Confirm) backed by a per-class leveling system that consumes XP from combat victories. Day-one scope: 3 classes (Fighter / Mage / Priest), 5 races (Human / Elf / Dwarf / Gnome / Hobbit), creation lands a new `RecruitDef` in `RecruitPool` for the existing `handle_guild_recruit` spawn path to consume.

## Approach

Build the **leveling layer first** (pure functions in a new `src/plugins/party/progression.rs`: `level_up`, `xp_for_level`, `roll_bonus_pool`, `allocate_bonus_pool`, `can_create_class`, plus a `ProgressionRng` resource and two handler systems), then layer the **multi-step creation wizard** on top as a `GuildMode` sub-state extension (per-variant `.run_if(in_state(GuildMode::CreateXxx))` gating, identical pattern to the existing Roster/Recruit dispatch). The creation flow's **last step pushes a new `RecruitDef` into the `Assets<RecruitPool>` mutable handle** (Option A from research) and auto-switches to `GuildMode::Recruit` with the cursor on the new entry — eliminating any parallel spawn pipeline and reusing `handle_guild_recruit` byte-for-byte.

A new `Race` asset layer (`src/data/races.rs` + `assets/races/core.races.racelist.ron`) mirrors `ClassDef`/`ClassTable` exactly, carrying `BaseStats` signed-i16 offsets applied to the class's `starting_stats` via `saturating_add_signed` (per user decision Q3). The `ClassDef` schema is extended additively (`min_stats`, `allowed_races`, `advancement_requirements`, `bonus_pool_min/max`, `stat_penalty_on_change`), all gated behind `#[serde(default)]` so existing `core.classes.ron` parses without edits — but we also populate the new fields in the same RON file for Fighter/Mage/Priest as part of this PR. The XP hook at `turn_manager.rs:634` emits a new `CombatVictoryEvent` (`Message`, not `Event` — Bevy 0.18 family rename) consumed by `award_combat_xp` in `progression.rs`; a follow-up `apply_level_up_threshold_system` advances `Experience.level` and recomputes `xp_to_next_level` whenever `current_xp >= xp_to_next_level`. Class-change UI is deferred per user decision Q6 — the data shape lands day-one but no system consumes `advancement_requirements`.

## Critical

- **`MessageWriter<CombatVictoryEvent>` requires registration**: `app.add_message::<CombatVictoryEvent>()` in `PartyPlugin::build`. Bevy 0.18 panics on a writer for an unregistered message type.
- **Pure functions take `rng: &mut (impl rand::Rng + ?Sized)`**: the `?Sized` is required to permit `&mut *boxed_rng.0` from a `Box<dyn RngCore + Send + Sync>`. Matches `damage_calc` at `src/plugins/combat/damage.rs:62` and `EncounterTable::pick_group` at `src/data/encounters.rs:78`. Forgetting `?Sized` makes the `ProgressionRng`-wrapped resource untouchable.
- **`Class` enum has 8 variants but only 3 are authored**: `ClassTable::get` returns `Option<&ClassDef>` (linear scan, `src/data/classes.rs:41-43`). The creation UI MUST filter `Class::iter()` against `ClassTable::get(c).is_some()` to skip Thief/Bishop/Samurai/Lord/Ninja. **Never write exhaustive `match Class { ... }` without a wildcard arm.**
- **RON files use the DOUBLE-DOT extension** `<name>.<type>.ron`. The new file MUST be `assets/races/core.races.racelist.ron` and `RonAssetPlugin::<RaceTable>::new(&["racelist.ron"])` (no leading dot). Single-dot won't load; round-trip unit tests don't catch this — only `cargo run` does. See [[project_druum_ron_asset_naming]].
- **`Experience.xp_to_next_level` is cached, not recomputed every read**. ALL mutators of `Class` or `Experience.level` MUST call `recompute_xp_to_next_level(&mut exp, class, &table)`. Source: research Pitfall 3.
- **`derive_stats` returns `current_hp = max_hp` and `current_mp = max_mp`** (caller-clamp contract, `character.rs:128-131`). On level-up, callers MUST reset `current_hp = new_max_hp` after re-deriving (same as `temple.rs` revive path; opposite of equipment-change which uses `current.min(new_max)`).
- **Re-validate eligibility in `handle_create_confirm`** — never trust draft state alone. Defense-in-depth against state-manipulation bypass (Security trust boundary).
- **Bevy 0.18 `Class`/`Race`/`BaseStats` discriminant order is locked** for save-format stability (`character.rs:39-46, 62-72`). Do NOT reorder or insert variants in the middle.

## Steps

### Phase 1 — Schema extensions and data layer

- [x] **1.1** Extend `ClassDef` with 5 new fields (`src/data/classes.rs`, ~+20 LOC). All `#[serde(default)]` so existing RON parses unchanged:
  ```rust
  pub struct ClassDef {
      // existing fields unchanged...
      #[serde(default)] pub min_stats: BaseStats,
      #[serde(default)] pub allowed_races: Vec<Race>,
      #[serde(default)] pub advancement_requirements: Vec<ClassRequirement>,
      #[serde(default)] pub bonus_pool_min: u32,
      #[serde(default)] pub bonus_pool_max: u32,
      #[serde(default)] pub stat_penalty_on_change: BaseStats,
  }
  ```
  Also add `pub use crate::plugins::party::character::Race;` to the existing `use` block. Add the `ClassRequirement` type in the same file:
  ```rust
  #[derive(Reflect, Serialize, Deserialize, Default, Debug, Clone, PartialEq, Eq, Hash)]
  pub struct ClassRequirement { pub from_class: Class, pub min_level: u32 }
  ```
  Update the existing `class_table_round_trips_through_ron` test fixture (`fighter_def`/`mage_def`) to include the new fields with sensible values (e.g., `min_stats: BaseStats::ZERO`, `allowed_races: vec![]`, `bonus_pool_min: 5`, `bonus_pool_max: 9`, all others default).
  **Files:** `src/data/classes.rs` (+20 modified LOC, +5 modified test fixture LOC).

- [x] **1.2** Populate the new fields in `assets/classes/core.classes.ron` for Fighter/Mage/Priest (~+24 LOC). Use Wizardry-1 conventions adapted to the codebase's stat ranges (research §Pattern 3 + §Tertiary sources):
  - Fighter: `min_stats: (strength: 11, ..)`, `allowed_races: [Human, Elf, Dwarf, Gnome, Hobbit]`, `bonus_pool_min: 5`, `bonus_pool_max: 9`, `advancement_requirements: []`, `stat_penalty_on_change: ()` (all-zero — Q6=C).
  - Mage: `min_stats: (intelligence: 11, ..)`, `allowed_races: [Human, Elf, Gnome, Hobbit]`, `bonus_pool_min: 5`, `bonus_pool_max: 9`, `advancement_requirements: []`, `stat_penalty_on_change: ()`.
  - Priest: `min_stats: (piety: 11, ..)`, `allowed_races: [Human, Elf, Dwarf, Gnome]`, `bonus_pool_min: 5`, `bonus_pool_max: 9`, `advancement_requirements: []`, `stat_penalty_on_change: ()`.
  Comment each entry with its rationale per the [[project_druum_temple_guild]] precedent.
  **Files:** `assets/classes/core.classes.ron` (+24 modified LOC).

- [x] **1.3** Create `src/data/races.rs` (~+85 LOC, NEW file). Define `RaceData`, `RaceTable`, and a `get` method mirroring `ClassDef`/`ClassTable` exactly:
  ```rust
  use bevy::prelude::*;
  use serde::{Deserialize, Serialize};
  use crate::plugins::party::character::{BaseStats, Race};

  #[derive(Reflect, Serialize, Deserialize, Default, Debug, Clone, PartialEq)]
  pub struct RaceData {
      pub id: Race,
      pub display_name: String,
      /// Signed i16 offsets applied to ClassDef.starting_stats via
      /// saturating_add_signed. Per user decision Q3.
      pub stat_modifiers: BaseStats,   // NOTE: interpret each field as i16 via `as i16`
      #[serde(default)]
      pub description: String,
  }

  #[derive(Asset, Reflect, Serialize, Deserialize, Default, Debug, Clone, PartialEq)]
  pub struct RaceTable {
      pub races: Vec<RaceData>,
  }

  impl RaceTable {
      pub fn get(&self, race: Race) -> Option<&RaceData> {
          self.races.iter().find(|r| r.id == race)
      }
  }
  ```
  Add a doc-comment block matching `data/classes.rs:1-12` explaining the reverse-dep pattern (`data/` imports from `plugins/party/character`). Add a `race_table_round_trips_through_ron` unit test.
  **Files:** `src/data/races.rs` (+85 new LOC, includes 1 test).

- [x] **1.4** Wire `RaceTable` through `src/data/mod.rs` (+2 modified LOC). Add:
  ```rust
  pub mod races;
  pub use races::{RaceData, RaceTable};
  ```
  Order alphabetically between `items` and `spells`.
  **Files:** `src/data/mod.rs` (+2 modified LOC).

- [x] **1.5** Create `assets/races/core.races.racelist.ron` (~+60 new LOC). Five entries:
  ```ron
  // Feature #19 — Race data: per-race signed-i16 stat offsets applied to
  // ClassDef.starting_stats via saturating_add_signed during character creation.
  // Per user decision Q3: 5 races day-one, balanced -2..=+2 offsets.
  (
      races: [
          (
              id: Human,
              display_name: "Human",
              stat_modifiers: (strength: 0, intelligence: 0, piety: 0, vitality: 0, agility: 0, luck: 0),
              description: "Balanced; no modifiers.",
          ),
          (
              id: Elf,
              display_name: "Elf",
              // STR-1, INT+2, PIE+1, AGI+1, LCK-1, VIT-2 (i16 reinterpretation)
              stat_modifiers: (strength: 65535, intelligence: 2, piety: 1, vitality: 65534, agility: 1, luck: 65535),
              description: "Magical and dexterous; physically frail.",
          ),
          // Dwarf: STR+2, INT-2, PIE+1, VIT+2, AGI-2, LCK-1
          // Gnome: STR-1, INT+2, PIE+2, VIT-1, AGI+0, LCK-2
          // Hobbit: STR-2, INT+0, PIE-1, VIT-1, AGI+2, LCK+3
          // ... (full 5 entries authored).
      ],
  )
  ```
  **NOTE on i16 encoding in RON:** `BaseStats.strength: u16` cannot directly encode negative values. The plan stores the bit-pattern (e.g., `-1 == 65535`, `-2 == 65534`) and reinterprets via `field as i16` at the apply site. **Document this contract in `races.rs` and in `RaceData::stat_modifiers`'s doc-comment.** Add a `cargo run`-style smoke validation step at the end of the verification list to catch authoring typos.
  **Files:** `assets/races/core.races.racelist.ron` (+60 new LOC).

- [x] **1.6** Register `RonAssetPlugin::<RaceTable>` in `LoadingPlugin::build` and extend `TownAssets` with `race_table` field (`src/plugins/loading/mod.rs`, ~+8 modified LOC).
  - Add `RaceTable` to the `use crate::data::{...}` block.
  - Add `RonAssetPlugin::<RaceTable>::new(&["racelist.ron"])` to the `.add_plugins((...))` tuple (after `RonAssetPlugin::<TownServices>` to keep towncentric grouping — comment "Feature #19 — race table for character creation").
  - Add `#[asset(path = "races/core.races.racelist.ron")] pub race_table: Handle<RaceTable>,` to the `TownAssets` struct.
  **Files:** `src/plugins/loading/mod.rs` (+8 modified LOC).

### Phase 2 — Progression module (pure functions + handlers)

- [x] **2.1** Create `src/plugins/party/progression.rs` (~+270 new LOC, NEW file). Skeleton sections in order:

  **(a) Imports + plugin (~25 LOC):**
  ```rust
  use bevy::prelude::*;
  use rand::rngs::SmallRng;
  use rand::{Rng, SeedableRng};

  use crate::data::{ClassTable, RaceTable};
  use crate::plugins::party::character::{
      BaseStats, Class, DerivedStats, Experience, PartyMember, Race, StatusEffectType,
      StatusEffects, derive_stats,
  };
  // Note: ClassDef/RaceData are pub re-exports from src/data/mod.rs (Step 1.4) but
  // the pure functions take them by &reference, so a direct `use crate::data::classes::ClassDef`
  // is also fine — match the existing pattern in src/data/town.rs.

  /// Resource wrapping a boxed RNG for production handlers. Tests pass a
  /// seeded `ChaCha8Rng` directly to the pure functions and skip this resource.
  #[derive(Resource)]
  pub struct ProgressionRng(pub Box<dyn rand::RngCore + Send + Sync>);

  impl Default for ProgressionRng {
      fn default() -> Self {
          Self(Box::new(SmallRng::from_os_rng()))
      }
  }
  ```

  **(b) Messages (~10 LOC):**
  ```rust
  /// Emitted on combat victory. Consumed by `award_combat_xp`. `Message`, not
  /// `Event` — Bevy 0.18 family rename (see [[project_druum_state_machine]]).
  #[derive(Message, Debug, Clone, Copy)]
  pub struct CombatVictoryEvent {
      pub total_xp: u32,
      pub total_gold: u32, // 0 in v1; combat-gold deferred to #21+
  }
  ```

  **(c) Pure functions (~100 LOC):**
  - `pub fn xp_for_level(target_level: u32, class_def: &ClassDef) -> u64` — implements `xp_to_level_2 * curve_factor^(target_level - 1)`. Clamps `curve_factor` to `[1.0, 10.0]` at the trust boundary. Rejects non-finite. Returns `u64::MAX` on overflow.
  - `pub fn level_up(current: &BaseStats, current_level: u32, class_def: &ClassDef, rng: &mut (impl rand::Rng + ?Sized)) -> StatGains` — pure; returns `StatGains` struct. `rng` is reserved for future stochastic growth (research §Code Examples). At level cap 99 (per user Q7=B), returns `StatGains` with all-zero deltas and `new_xp_to_next_level: u64::MAX` so the threshold system never triggers another level-up; remaining `current_xp` stays in-place (planner choice — see Open Question O1 below).
  - `pub fn xp_to_next_level_for(class_def: &ClassDef, level: u32) -> u64` — thin wrapper around `xp_for_level(level + 1, class_def)`; used by `recompute_xp_to_next_level`.
  - `pub fn recompute_xp_to_next_level(experience: &mut Experience, class: Class, table: &ClassTable)` — invalidates the cache (Pitfall 3). Used by level-up handler + creation `Confirm`.
  - `pub fn roll_bonus_pool(class_def: &ClassDef, rng: &mut (impl rand::Rng + ?Sized)) -> u32` — per user Q1=1B: implements Wizardry 80/20 roll, but the player can re-roll unlimited times before allocating. Uses `class_def.bonus_pool_min`/`bonus_pool_max` (default to 5/9 if class authored them as zero). The handler at Step 3.6 calls this on each "Re-Roll" press.
  - `pub fn allocate_bonus_pool(base: &mut BaseStats, allocations: &[u16; 6], pool: u32, race_modifiers: &BaseStats) -> Result<(), AllocError>` — sums allocations, checks ≤ pool, applies allocations to `base`, then applies race modifiers via `field.saturating_add_signed(race_modifier as i16)`. `AllocError::OverPool` on sum > pool.
  - `pub fn can_create_class(race: Race, base: &BaseStats, class_def: &ClassDef) -> Result<(), CreateError>` — checks `class_def.allowed_races.is_empty() || allowed_races.contains(&race)` AND each `min_stats.X <= base.X`. `CreateError::DisallowedRace` / `CreateError::BelowMinStat { stat, required, actual }`.
  - `pub fn level_cap() -> u32 { 99 }` — single source of truth for the cap.

  **(d) Handlers (~60 LOC):**
  - `pub fn award_combat_xp(mut reader: MessageReader<CombatVictoryEvent>, mut party: Query<(&mut Experience, &StatusEffects), With<PartyMember>>)` — splits `event.total_xp` among living members (filter `!status.has(StatusEffectType::Dead)`). Integer-truncates the per-member share (lost remainder is acceptable — research §Code Examples). `saturating_add` on `current_xp`. Clamps `event.total_xp` to `[0, 1_000_000]` at the trust boundary (Security: a single corrupted enemy XP value otherwise pushes a character to level 1000).
  - `pub fn apply_level_up_threshold_system(mut party: Query<(&mut Experience, &mut BaseStats, &mut DerivedStats, &Class, &StatusEffects), With<PartyMember>>, table: Option<Res<Assets<ClassTable>>>, town_assets: Option<Res<crate::plugins::loading::TownAssets>>, mut rng: ResMut<ProgressionRng>)` — for each member, while `current_xp >= xp_to_next_level` AND `level < level_cap()`, call `level_up`, apply gains to `BaseStats` (saturating add per-field), re-derive `DerivedStats` via `derive_stats(...)`, reset `current_hp = new_max_hp` (caller-clamp contract — character.rs:128-131), `current_mp = new_max_mp`, increment `level`, recompute cached threshold. Use `&mut *rng.0` for the `?Sized` Rng borrow.

  **(e) PartyProgressionPlugin (~10 LOC):**
  ```rust
  pub struct PartyProgressionPlugin;
  impl Plugin for PartyProgressionPlugin {
      fn build(&self, app: &mut App) {
          app.init_resource::<ProgressionRng>()
             .add_message::<CombatVictoryEvent>()
             .add_systems(Update, (
                 award_combat_xp,
                 apply_level_up_threshold_system.after(award_combat_xp),
             ));
      }
  }
  ```
  No state gating: level-up can happen at the next-frame boundary regardless of GameState (defense-in-depth: lets a delayed `CombatVictoryEvent` from the previous frame be drained even after entering `Town`/`Dungeon`).

  **(f) `#[cfg(test)] mod tests` (~75 LOC):** ~10 tests covering pure functions + level cap edge (level=99 + XP overflow stays in current_xp). See Verification.
  **Files:** `src/plugins/party/progression.rs` (+270 new LOC).

- [x] **2.2** Register progression in `PartyPlugin::build` (`src/plugins/party/mod.rs`, ~+5 modified LOC).
  - Add `pub mod progression;` after `pub mod inventory;`.
  - Add `pub use progression::{CombatVictoryEvent, ProgressionRng, StatGains, level_cap, level_up, xp_for_level, can_create_class, roll_bonus_pool, allocate_bonus_pool, recompute_xp_to_next_level};` (note: keep the list compact — re-exports add zero compile-time cost but help downstream callers).
  - Add `.add_plugins(progression::PartyProgressionPlugin)` inside `PartyPlugin::build`.
  - Register reflect types for the few `#[derive(Reflect)]` types added by progression.rs.
  **Files:** `src/plugins/party/mod.rs` (+5 modified LOC).

### Phase 3 — Guild creation wizard (UI + handlers)

- [x] **3.1** Extend `GuildMode` enum in `src/plugins/town/guild.rs:104-111` (~+10 modified LOC). Add 5 new variants AFTER the existing two, preserving discriminant order for forward-compat:
  ```rust
  pub enum GuildMode {
      #[default] Roster,
      Recruit,
      // Feature #19 — character creation wizard sub-modes.
      CreateRace,      // Step 1/5: pick a race.
      CreateClass,     // Step 2/5: pick a class (filtered by race + ClassTable::get).
      CreateRoll,      // Step 3/5: roll bonus pool; re-roll allowed.
      CreateAllocate,  // Step 3.5/5: distribute pool across 6 stats.
      CreateName,      // Step 4/5: enter character name.
      CreateConfirm,   // Step 5/5: review + commit (push to RecruitPool).
  }
  ```
  Update the `mode_label` match in `paint_guild` (`guild.rs:151-154`) to handle the new variants with a wildcard arm `_ => "Guild — Create Character"` so the existing painter doesn't break. The new dedicated painters paint the actual screen.
  **Files:** `src/plugins/town/guild.rs` (+10 modified LOC).

- [x] **3.2** Add `CreationDraft` resource in a new `src/plugins/town/guild_create.rs` (~+40 new LOC for the resource + helpers). At top of file:
  ```rust
  //! Guild character-creation wizard — Feature #19.
  //!
  //! Multi-step UI gated by GuildMode::CreateXxx sub-states. State held in
  //! `CreationDraft` (cleared on OnExit(TownLocation::Guild) + on creation
  //! completion). The Confirm step pushes a RecruitDef onto Assets<RecruitPool>
  //! (Option A from research — reuses handle_guild_recruit).
  use bevy::prelude::*;
  use bevy_egui::{EguiContexts, egui};
  use leafwing_input_manager::prelude::ActionState;
  use crate::data::{ClassTable, RaceTable, RecruitDef, RecruitPool};
  use crate::data::town::{MAX_RECRUIT_POOL, clamp_recruit_pool};
  use crate::plugins::input::MenuAction;
  use crate::plugins::loading::TownAssets;
  use crate::plugins::party::character::{BaseStats, Class, PartyRow, Race};
  use crate::plugins::party::progression::{
      ProgressionRng, allocate_bonus_pool, can_create_class, roll_bonus_pool,
  };
  use crate::plugins::town::guild::{GuildMode, GuildState, RecruitedSet};
  use crate::plugins::town::toast::Toasts;

  pub const MAX_NAME_LEN: usize = 20;

  #[derive(Resource, Default, Debug, Clone)]
  pub struct CreationDraft {
      pub race: Option<Race>,
      pub class: Option<Class>,
      pub rolled_bonus: u32,    // 0 = not yet rolled
      pub allocations: [u16; 6], // STR, INT, PIE, VIT, AGI, LCK
      pub name: String,
      pub default_row: PartyRow,
  }

  impl CreationDraft {
      pub fn reset(&mut self) { *self = Self::default(); }
      pub fn allocations_sum(&self) -> u32 {
          self.allocations.iter().map(|&v| v as u32).sum()
      }
  }
  ```
  **Files:** `src/plugins/town/guild_create.rs` (+40 new LOC, header + draft).

- [x] **3.3** Add `paint_guild_create_race` (`guild_create.rs`, ~+45 new LOC). Painter for `GuildMode::CreateRace`:
  - Reads `Res<CreationDraft>`, `Res<GuildState>`, `Option<Res<TownAssets>>`, `Res<Assets<RaceTable>>`.
  - Resolves `race_table` via `town_assets.and_then(|a| race_assets.get(&a.race_table))`. Returns label `"(loading races...)"` if missing.
  - Iterates `race_table.races.iter()`, marks `> ` on `guild_state.cursor`, displays signed-i16 modifiers via `field as i16` reinterpretation.
  - Footer: `"[Up/Down] Pick  |  [Enter] Confirm  |  [Esc] Cancel creation"`.
  - Read-only (`Res<...>` only). Returns `Result` (per `paint_guild` precedent).
  **Files:** `src/plugins/town/guild_create.rs` (+45 new LOC).

- [x] **3.4** Add `paint_guild_create_class` (`guild_create.rs`, ~+55 new LOC). Painter for `GuildMode::CreateClass`:
  - Reads `Res<CreationDraft>`, `Res<GuildState>`, `Option<Res<TownAssets>>`, `Res<Assets<ClassTable>>`.
  - **Filter**: iterate `Class::iter()` candidates (or hardcode the 3 authored variants) and keep only those where `class_table.get(c).is_some()` AND (`class_def.allowed_races.is_empty()` OR `allowed_races.contains(&draft.race.unwrap_or(Race::Human))`).
  - For each visible class: display its `display_name`, `min_stats` summary (`"min STR≥{}, ..."`), and `xp_to_level_2` / `hp_per_level` for player insight. `egui::ScrollArea::show_rows` per research Pitfall 7 (defensive — fewer than 10 entries day-one but pattern is correct).
  - Footer mirrors race painter.
  - **NOTE:** Don't use exhaustive `match Class { ... }` — iterate a `[Class::Fighter, Class::Mage, Class::Priest, Class::Thief, ..., Class::Ninja]` array literal and filter. Research §Anti-Patterns.
  **Files:** `src/plugins/town/guild_create.rs` (+55 new LOC).

- [x] **3.5** Add `paint_guild_create_roll` (`guild_create.rs`, ~+30 new LOC). Painter for `GuildMode::CreateRoll`:
  - Displays selected race + class summary.
  - Big text: `"Bonus pool: {draft.rolled_bonus}"` (or `"(press R to roll)"` if `rolled_bonus == 0`).
  - Footer: `"[R] Re-roll  |  [Enter] Accept and allocate  |  [Esc] Cancel"`.
  **Files:** `src/plugins/town/guild_create.rs` (+30 new LOC).

- [x] **3.6** Add `paint_guild_create_allocate` (`guild_create.rs`, ~+50 new LOC). Painter for `GuildMode::CreateAllocate`:
  - 6 rows: one per stat (STR/INT/PIE/VIT/AGI/LCK), showing `class_def.starting_stats.X + draft.allocations[i]` and the running pool spent vs. `draft.rolled_bonus`.
  - Cursor highlights the active stat; Left/Right adjusts the allocation (`actions.just_pressed(&MenuAction::Left/Right)`) by 1, clamped to `0..=remaining_pool`. (No `egui::Slider` in the painter — it's read-only; allocation deltas come from the handler at Step 3.10.)
  - Display class-eligibility status: live-call `can_create_class(draft.race?, &projected_base_stats, class_def)` and show "OK" or the specific `CreateError` reason — gives the player feedback BEFORE committing.
  - Footer: `"[Up/Down] Stat  |  [Left/Right] Allocate  |  [Enter] Continue to name  |  [Esc] Cancel"`.
  **Files:** `src/plugins/town/guild_create.rs` (+50 new LOC).

- [x] **3.7** Add `paint_guild_create_name` (`guild_create.rs`, ~+25 new LOC). Painter for `GuildMode::CreateName`:
  - `egui::TextEdit::singleline(&mut /*needs mut, see below*/)` is NOT read-only friendly. **Implementation note:** since painters are read-only by project convention, we DISPLAY the current `draft.name` and instruct the player to type. The handler at Step 3.11 reads `KeyboardInput` messages directly to append chars / handle backspace. Alternative if egui's `TextEdit` is needed: paint via `ui.label(format!("Name: {}_", draft.name))` and rely on the handler for input mutation.
  - Footer: `"[Type] Edit  |  [Backspace] Delete  |  [Enter] Confirm  |  [Esc] Cancel"`.
  - Truncate display to `MAX_NAME_LEN` (defense-in-depth — the handler also enforces).
  **Files:** `src/plugins/town/guild_create.rs` (+25 new LOC).

- [x] **3.8** Add `paint_guild_create_confirm` (`guild_create.rs`, ~+40 new LOC). Painter for `GuildMode::CreateConfirm`:
  - Shows the full character summary: name, race + display-name, class + display-name, final `BaseStats` after `starting_stats + allocations + race_modifiers` (with saturating-add-signed for the race step), `default_row: Front` (hardcoded; future polish).
  - Footer: `"[Enter] Confirm & recruit  |  [Esc] Back to allocation"`.
  **Files:** `src/plugins/town/guild_create.rs` (+40 new LOC).

- [x] **3.9** Add `handle_guild_create_input` (`guild_create.rs`, ~+90 new LOC). The umbrella navigation handler for all 6 creation sub-modes. **One system, branches on `guild_state.mode`**:
  - `MenuAction::Cancel` → reset draft + switch to `GuildMode::Roster` (or Recruit if user started from Recruit — track origin via a small `Option<GuildMode>` field on `CreationDraft`, or just always go to Roster for simplicity).
  - `MenuAction::Up`/`Down` → adjust `guild_state.cursor` against the list-len of the current sub-mode (race list / class list / 6 stats).
  - `MenuAction::Confirm` → advances the sub-mode:
    - `CreateRace` → if cursor resolves to a valid race, write `draft.race = Some(...)` and switch to `CreateClass`. Reset `guild_state.cursor = 0`.
    - `CreateClass` → if cursor resolves to an authored + allowed class, write `draft.class = Some(...)` and switch to `CreateRoll`. Reset `cursor`. Defense-in-depth check `class_table.get(c).is_some()` even though painter filtered (research Security: state-manipulation bypass).
    - `CreateRoll` → if `draft.rolled_bonus > 0`, switch to `CreateAllocate` with `cursor = 0`. Otherwise no-op (force the player to roll first).
    - `CreateAllocate` → defense-in-depth `can_create_class(draft.race, &projected_base, class_def)` validation; if Err, push a `Toasts` message and stay. If Ok, switch to `CreateName`.
    - `CreateName` → if `!draft.name.trim().is_empty()` and `draft.name.len() <= MAX_NAME_LEN`, switch to `CreateConfirm`. Otherwise no-op.
    - `CreateConfirm` → handled by Step 3.12's separate handler (`handle_guild_create_confirm`) so the mutation of `Assets<RecruitPool>` is in a system with `ResMut<Assets<RecruitPool>>`.
  **Files:** `src/plugins/town/guild_create.rs` (+90 new LOC).

- [x] **3.10** Add `handle_guild_create_allocate` (`guild_create.rs`, ~+40 new LOC). Per-stat allocation handler, gated on `GuildMode::CreateAllocate`:
  - On `MenuAction::Left` (cursor stat) → `draft.allocations[cursor] = draft.allocations[cursor].saturating_sub(1)`.
  - On `MenuAction::Right` (cursor stat) → if `draft.allocations_sum() + 1 <= draft.rolled_bonus`, increment `draft.allocations[cursor]`. Otherwise push a toast `"Bonus pool fully allocated."` (rate-limit to one per second via a `Local<f32>` cooldown to avoid toast spam).
  - The `[Up/Down]` cursor movement is handled by `handle_guild_create_input` (Step 3.9).
  **Files:** `src/plugins/town/guild_create.rs` (+40 new LOC).

- [x] **3.11** Add `handle_guild_create_name_input` (`guild_create.rs`, ~+50 new LOC). Reads `KeyboardInput` messages directly (NOT leafwing — character keys aren't mapped). Gated on `GuildMode::CreateName`:
  ```rust
  pub fn handle_guild_create_name_input(
      guild_state: Res<GuildState>,
      mut draft: ResMut<CreationDraft>,
      mut events: MessageReader<bevy::input::keyboard::KeyboardInput>,
  ) {
      if guild_state.mode != GuildMode::CreateName { return; }
      for event in events.read() {
          if event.state != bevy::input::ButtonState::Pressed { continue; }
          match &event.logical_key {
              bevy::input::keyboard::Key::Character(s) => {
                  if draft.name.len() + s.len() <= MAX_NAME_LEN
                      && s.chars().all(|c| c.is_ascii_alphanumeric() || c == ' ') {
                      draft.name.push_str(s);
                  }
              }
              bevy::input::keyboard::Key::Backspace => { draft.name.pop(); }
              _ => {}
          }
      }
  }
  ```
  Length-enforcement at TWO points: this handler + final truncate in Confirm. Trust boundary per Security risk "Player-controlled name overflow".
  **Files:** `src/plugins/town/guild_create.rs` (+50 new LOC).

- [x] **3.12** Add `handle_guild_create_roll` and `handle_guild_create_confirm` (`guild_create.rs`, ~+80 new LOC). These are the two MUTATING ASSET handlers.

  **`handle_guild_create_roll`** (gated `GuildMode::CreateRoll`):
  ```rust
  pub fn handle_guild_create_roll(
      guild_state: Res<GuildState>,
      mut draft: ResMut<CreationDraft>,
      actions: Res<ActionState<MenuAction>>,
      class_assets: Res<Assets<ClassTable>>,
      town_assets: Option<Res<TownAssets>>,
      mut rng: ResMut<ProgressionRng>,
  ) {
      if guild_state.mode != GuildMode::CreateRoll { return; }
      if !actions.just_pressed(&MenuAction::Recruit) { return; }  // 'R' = re-roll
      let Some(class) = draft.class else { return };
      let Some(assets) = town_assets else { return };
      let Some(table) = class_assets.get(&assets.class_table) else { return };
      let Some(class_def) = table.get(class) else { return };
      draft.rolled_bonus = roll_bonus_pool(class_def, &mut *rng.0);
      // Reset previous allocations whenever a new pool is rolled (Pitfall: leftover
      // allocations would exceed the new pool size).
      draft.allocations = [0; 6];
  }
  ```

  **`handle_guild_create_confirm`** (gated `GuildMode::CreateConfirm`, on `MenuAction::Confirm`):
  - Defense-in-depth re-validate: `can_create_class(draft.race?, &final_base_stats, class_def)` — push toast + return on Err.
  - Truncate `draft.name` to `MAX_NAME_LEN` (trust boundary).
  - Apply allocations + race modifiers to `class_def.starting_stats` (via `saturating_add` for allocations and `saturating_add_signed` for race i16 deltas).
  - Pull `MAX_RECRUIT_POOL` cap: if `pool.recruits.len() >= MAX_RECRUIT_POOL`, push toast `"Recruit pool full — visit Temple/Guild to dismiss someone first."` and return.
  - Mutate `Assets<RecruitPool>::get_mut(&town_assets.recruit_pool)`: `pool.recruits.push(RecruitDef { name, race, class, base_stats, default_row: PartyRow::Front })`.
  - Reset `draft` (`draft.reset()`).
  - Auto-switch to `GuildMode::Recruit`, set `guild_state.cursor` to the just-pushed index (`pool.recruits.len() - 1`).
  - Push a toast: `"{name} has joined the guild!"`.
  **Files:** `src/plugins/town/guild_create.rs` (+80 new LOC).

- [x] **3.13** Wire creation handlers + painters in `src/plugins/town/mod.rs` (~+30 modified LOC). Add to the `EguiPrimaryContextPass` tuple:
  ```rust
  paint_guild_create_race.run_if(in_state(TownLocation::Guild).and(/*GuildMode==CreateRace*/)),
  ...
  ```
  Since `GuildMode` is a plain `enum` (not a Bevy `States` impl), per-variant `.run_if` cannot use `in_state(...)`. Instead, define helper `run_if` closures:
  ```rust
  fn in_guild_mode(target: GuildMode) -> impl FnMut(Res<GuildState>) -> bool + Clone {
      move |state: Res<GuildState>| state.mode == target
  }
  ```
  Register all 6 create painters + 4 create handlers (input, allocate, name_input, roll, confirm) and combine: `.run_if(in_state(TownLocation::Guild)).run_if(in_guild_mode(GuildMode::CreateXxx))`.
  Also add `app.init_resource::<CreationDraft>()` to `TownPlugin::build`.
  Also add `app.add_systems(OnExit(TownLocation::Guild), |mut d: ResMut<CreationDraft>| d.reset())` so leaving Guild mid-creation discards the draft (research §Pattern 2).
  **Files:** `src/plugins/town/mod.rs` (+30 modified LOC).

- [x] **3.14** Wire creation entry-point in the existing `handle_guild_input` at `guild.rs:254-295` (~+10 modified LOC). Add a new keypress to start creation: on `MenuAction::NextTarget` (the `]` key — already bound, currently unused in Guild mode) while in `GuildMode::Roster`, switch to `GuildMode::CreateRace` and reset `guild_state.cursor = 0`. Document this in `paint_guild`'s footer label by appending `"  |  []] New character (creation)"`.
  Rationale: NextTarget is already wired through `default_menu_input_map` at `input/mod.rs:248`; reusing it avoids touching the InputMap. The 'C' letter would be ideal but `default_menu_input_map` doesn't bind it.
  **Alternative if NextTarget feels wrong:** add `MenuAction::Create` as a new variant in `src/plugins/input/mod.rs`. The cost is +5 LOC across the enum, default map, and logical-key handler. Planner's choice: **use NextTarget** to keep Δ Input = 0 and accept the modest UX abuse.
  **Files:** `src/plugins/town/guild.rs` (+10 modified LOC).

### Phase 4 — Combat victory XP hook

- [x] **4.1** Modify `check_victory_defeat_flee` at `src/plugins/combat/turn_manager.rs:598-646` to emit `CombatVictoryEvent` (~+15 modified LOC). At the `// 3. Victory.` branch (line 634):
  ```rust
  use crate::plugins::party::progression::CombatVictoryEvent;
  // (add to the imports at top: `use crate::plugins::party::progression::CombatVictoryEvent;`
  //  and to the system signature: `mut victory_writer: MessageWriter<CombatVictoryEvent>,`)
  if all_enemies_dead {
      let total_xp = compute_xp_from_enemies(&enemies);
      combat_log.push("Victory!".into(), input_state.current_turn);
      victory_writer.write(CombatVictoryEvent { total_xp, total_gold: 0 });
      next_state.set(GameState::Dungeon);
      return;
  }
  ```
  Add a pure free function in the same file (above `check_victory_defeat_flee` or in a sibling helper section):
  ```rust
  /// Compute total XP from defeated enemies.
  ///
  /// **Planner decision (not in research's 7-question batch):** since
  /// `EnemySpec` lacks an authored `xp_reward` field and adding one would
  /// require an `assets/encounters/floor_01.encounters.ron` migration outside
  /// the scope of this PR, derive XP from `max_hp` as a proxy for "toughness".
  /// Formula: `xp = max_hp / 2`, clamped to [0, 1_000_000] per enemy.
  /// Future polish: add `EnemySpec.xp_reward: Option<u32>` (#21+).
  fn compute_xp_from_enemies(
      enemies: &Query<(&DerivedStats, &StatusEffects), With<Enemy>>,
  ) -> u32 {
      enemies.iter()
          .map(|(d, _)| (d.max_hp / 2).min(1_000_000))
          .sum::<u32>()
          .min(1_000_000)  // additional sum-cap defense-in-depth
  }
  ```
  **Files:** `src/plugins/combat/turn_manager.rs` (+15 modified LOC).

### Phase 5 — Tests

- [x] **5.1** Pure-function unit tests in `src/plugins/party/progression.rs::tests` (~+110 new LOC, 10 tests). See Verification section for the exact list.
  **Files:** `src/plugins/party/progression.rs` (+110 new LOC — counted in 2.1's 270).

- [x] **5.2** Integration test for the XP threshold + level-up handler (`src/plugins/party/progression.rs::tests`, ~+40 new LOC, +1 test):
  - `xp_threshold_triggers_level_up`: spawn a `PartyMember` at level 1 with `current_xp = 99`, `xp_to_next_level = 100`, then bump `current_xp = 150` and run `app.update()`. Assert `level == 2`, `current_xp == 150` (XP is not consumed — accumulates per design), `xp_to_next_level == 150` (Fighter curve 100 * 1.5).
  Use `MinimalPlugins + StatesPlugin + AssetPlugin::default()` per `guild.rs:649-712` pattern. Insert `ProgressionRng(Box::new(ChaCha8Rng::seed_from_u64(42)))` for determinism. Mock `Assets<ClassTable>` with one fighter entry.
  **Files:** `src/plugins/party/progression.rs` (+40 new LOC, counted in 2.1).

- [x] **5.3** Integration tests in `src/plugins/town/guild_create.rs::tests` (~+90 new LOC, 3 tests). Follow the `make_guild_test_app` pattern from `guild.rs:649-712`. **Adapt by including:** `CreationDraft`, `ProgressionRng` (with ChaCha8Rng for determinism), `Assets<RaceTable>`, `Assets<ClassTable>` (populated with fighter/mage/priest fixtures), `TownAssets.race_table` and `TownAssets.class_table` mock handles.
  - `creation_confirm_appends_to_pool`: drive the draft to `CreateConfirm` with valid Human/Fighter, press Enter, assert `pool.recruits.len() += 1`, `guild_state.mode == GuildMode::Recruit`, draft reset.
  - `creation_rejects_class_below_min_stats`: drive draft to `CreateAllocate` with Mage class + base allocations giving INT < 11; assert toast pushed and stay in `CreateAllocate`.
  - `creation_pool_full_blocks_confirm`: prefill the pool with `MAX_RECRUIT_POOL` entries; press Confirm; assert no push happened, toast was pushed.
  **Files:** `src/plugins/town/guild_create.rs` (+90 new LOC).

- [x] **5.4** Unit test for the XP-hook helper in `src/plugins/combat/turn_manager.rs::tests` (~+25 new LOC, 1 test).
  - `compute_xp_from_enemies_sums_half_max_hp`: spawn 3 enemies with `max_hp = 30/30/60`; assert helper returns `15 + 15 + 30 == 60`.
  Use the existing `make_turn_manager_test_app` if one exists, or `MinimalPlugins + StatesPlugin`.
  **Files:** `src/plugins/combat/turn_manager.rs` (+25 modified LOC).

### Phase 6 — Documentation polish (single-line per file)

- [x] **6.1** Add doc-comment summaries for the 4 new top-level entry points (`progression.rs` module doc, `guild_create.rs` module doc, `races.rs` module doc — `races.rs` already has one from Step 1.3). Each module doc references its research feature (#19) and the linked plan file `project/plans/20260513-120000-feature-19-character-creation.md`. (~+15 modified LOC total across 3 files.)
  **Files:** `src/plugins/party/progression.rs`, `src/plugins/town/guild_create.rs`, `src/data/races.rs` (already done in 1.3).

## Security

**Known vulnerabilities:** No CVEs against the recommended versions (`bevy 0.18.1`, `bevy_egui 0.39.1`, `bevy_common_assets 0.16.0`, `bevy_asset_loader 0.26.0`, `leafwing-input-manager 0.20.0`, `rand 0.9`, `rand_chacha 0.9`, `ron 0.12`) as of 2026-05-13. Feature #19 introduces zero new direct dependencies. Δ Cargo.toml = 0.

**Architectural risks (apply at the trust boundaries below):**

- **Crafted RON exploits** — clamp at the asset-consumer trust boundary:
  - `xp_curve_factor: f32` — clamp to `[1.0, 10.0]` and reject non-finite in `xp_for_level`. Without this, `f32::INFINITY` casts to 0 (NaN path) and bypasses level-up gating; `f32::NaN` casts to undefined u64.
  - `hp_per_level`, `mp_per_level: u32` — saturating arithmetic in `apply_level_up_threshold_system`. `u32::MAX` HP/MP gain is harmless with `saturating_add`.
  - `min_stats: BaseStats` (per-field `u16`) — clamp each field to `[3, 18]` at the `can_create_class` use-site (Wizardry hard cap). Without this, crafted `min_stats: BaseStats { strength: u16::MAX, .. }` makes every class permanently rejected.
  - `xp_to_level_2: u64` — clamp to `[1, 1_000_000_000]` in `xp_for_level`.
  - `bonus_pool_min`/`bonus_pool_max: u32` — clamp to `[0, 100]` in `roll_bonus_pool`. Default to 5/9 when both zero.

- **Player-controlled inputs** — clamp at the handler trust boundary:
  - `CreationDraft.name` — `MAX_NAME_LEN = 20` enforced in two places: `handle_guild_create_name_input` rejects characters beyond the limit, AND `handle_guild_create_confirm` calls `draft.name.truncate(MAX_NAME_LEN)` as defense-in-depth. Without this, a pathological key-repeat could push a 100MB string into `RecruitDef.name` → painter slowdown.
  - `CreationDraft.allocations` — `allocate_bonus_pool` returns `Err(OverPool)` if `sum > pool`. `handle_guild_create_confirm` re-checks; defense-in-depth.
  - `CreationDraft.class` — defense-in-depth `can_create_class(...)` re-validation in `handle_guild_create_confirm` even though painter filtered. Mitigates state-manipulation bypass (Security risk: "player force-sets draft.class = Class::Ninja").

- **Combat-victory XP overflow** — `CombatVictoryEvent.total_xp` is clamped to `[0, 1_000_000]` per event in BOTH the producer (`compute_xp_from_enemies` per-enemy cap + sum cap) AND the consumer (`award_combat_xp` clamps `event.total_xp.min(1_000_000)` before splitting). Without this, a corrupted enemy could push a character from level 1 to 99 in a single event.

- **`Assets<RecruitPool>` mutation** — `handle_guild_create_confirm` is the SOLE writer to `pool.recruits`. The clamp to `MAX_RECRUIT_POOL = 32` is checked BEFORE the push, mirroring the `clamp_recruit_pool` defense-in-depth at the painter (`data/town.rs:120-123`).

**Trust boundaries summary:**

| Boundary | Validation |
|---|---|
| `core.classes.ron` load | Clamp `min_stats` fields to `[3, 18]`, `hp/mp_per_level` to `[0, 64]`, `xp_to_level_2` to `[1, 1e9]`, `xp_curve_factor` to `[1.0, 10.0]` (and reject non-finite). |
| `core.races.racelist.ron` load | `stat_modifiers` reinterpretation `field as i16` is intrinsically bounded by u16 (no further clamp needed); but applying via `saturating_add_signed` prevents underflow on the recipient stat. |
| `CreationDraft.name` Confirm | `truncate(MAX_NAME_LEN)`. |
| `CreationDraft.allocations` Confirm | `sum <= rolled_bonus` AND `can_create_class(...)` passes. |
| `CreationDraft.class` Confirm | `can_create_class(...)` re-runs. |
| `CombatVictoryEvent.total_xp` write | `compute_xp_from_enemies` sums clamped per-enemy values, then caps sum. |
| `CombatVictoryEvent.total_xp` read | `award_combat_xp` re-clamps to `[0, 1_000_000]` (defense-in-depth). |

## Open Questions

- **O1 (Resolved by planner):** XP saturation at level cap. Per user Q7=B, level cap is 99. **Decision:** when a character reaches level 99, `current_xp` accumulates above `xp_to_next_level` and stays there (no information loss). The level-up threshold system early-returns when `level >= level_cap()`. Rationale: simpler invariant (XP is monotonic), matches Wizardry/Etrian convention of "XP plateau visible to the player". Documented in `apply_level_up_threshold_system`'s doc-comment.

- **O2 (Resolved by planner):** `compute_xp_from_enemies` formula. Per the research's NEW pure helper note, the formula was unspecified. **Decision:** `xp_per_enemy = max_hp / 2`, clamped per-enemy and sum-clamped to `[0, 1_000_000]`. Rationale: zero asset migration (no new `EnemySpec` field, no `core.enemies.ron` edit), uses existing `DerivedStats.max_hp` as a proxy for "toughness". Future polish (#21+) can add `EnemySpec.xp_reward: Option<u32>` as an additive field. Documented inline at the helper.

- **O3 (Resolved by planner):** Sub-mode representation. Per user brief, "extend `GuildMode` enum OR add a parallel nested `CreationState`". **Decision:** extend `GuildMode` with 5 new variants. Rationale: per-variant `.run_if(...)` gating via a small `in_guild_mode(target: GuildMode)` closure helper is the cleanest match for the existing Roster/Recruit pattern; nested `CreationState` would add a second source-of-truth for "which painter to render" that the existing Roster/Recruit dispatch in `paint_guild` does not have.

- **O4 (Resolved by planner):** File split. **Decision:** new sibling file `src/plugins/town/guild_create.rs` rather than extending `guild.rs` in-place. Rationale: research recommends sibling split; mirrors `combat/{damage,targeting,ai,turn_manager}.rs` separation pattern; keeps `guild.rs` at ~1150 LOC and isolates the wizard's ~470 new LOC.

- **O5 (Resolved by planner):** Creation entry-point keybind. **Decision:** repurpose `MenuAction::NextTarget` (`]` key) while in `GuildMode::Roster` — avoids any change to `default_menu_input_map`. Alternative `MenuAction::Create` adds enum + map + logical-key bindings; deferred until UX testing demands it.

## Implementation Discoveries

1. **B0002 false alarm in `handle_guild_create_confirm`**: The plan sketched both `Res<GuildState>` and `ResMut<GuildState>` as separate params. Fixed by using only `ResMut<GuildState>` throughout.

2. **Double-CentralPanel bug**: `paint_guild` always rendered a `CentralPanel` including in creation modes. The dedicated creation painters also render a `CentralPanel`. Having two `CentralPanel`s in one egui frame causes layout issues (second gets 0 space). Fixed by adding an early-return block in `paint_guild` for all 6 creation sub-modes, skipping the `CentralPanel` when creation painters will handle it.

3. **Test `creation_rejects_class_below_min_stats` missing Dwarf in race table**: The test uses `race: Some(Race::Dwarf)` to test race-exclusion via `can_create_class`. But the test race table only contained Human, so `projected_base_stats` returned `None` (Dwarf not found) → no toast was pushed → assertion failed. Fixed by adding `dwarf_race_data()` to the test race table.

4. **`SeedableRng` not in scope in `guild_create.rs` tests**: `ChaCha8Rng::seed_from_u64` requires `rand::SeedableRng` to be in scope. The parent module only imports `rand::Rng` (not `SeedableRng`), and `use super::*` doesn't help here. Fixed by adding `use rand::SeedableRng;` to the test module.

5. **Nested `if let` chains would trigger `clippy::collapsible_if`**: Multiple nested `if let Some(x) = ... { if let Some(y) = ... { ... } }` patterns throughout `handle_guild_create_input` and `paint_guild_create_allocate`. Converted to Rust 2024 let-chain syntax (`if let A && let B { ... }`).

6. **`ClassDef` struct literal requires 6 new fields**: The new `ClassRequirement` struct and the 6 `#[serde(default)]` fields must be in all `ClassDef` literal constructions in tests. Test fixtures updated in both `progression.rs` and `guild_create.rs` tests.

7. **`AllocError` is not exported from `guild_create.rs`**: The plan re-exports `AllocError` via `party/mod.rs`. The `guild_create.rs` only calls `allocate_bonus_pool(...).is_err()` so `AllocError` is never referenced directly in that file. No issue.

8. **`TownAssets` struct gains two new fields**: All TownAssets struct literal constructions in test apps (guild.rs, inn.rs, temple.rs, guild_create.rs) need `race_table: Handle::default()` and `class_table: Handle::default()` plus `init_asset::<ClassTable>()` and `init_asset::<RaceTable>()` calls.

9. **`handle_guild_create_name_input` requires `Messages<KeyboardInput>` in tests**: `MessageReader<KeyboardInput>` is a system param that validates at runtime — if `Messages<KeyboardInput>` is not registered (e.g., `InputPlugin` is absent from `MinimalPlugins`), the system panics. Fixed by adding `app.add_message::<KeyboardInput>()` to `make_create_test_app()`. The system itself early-returns when mode != CreateName, but param validation happens before the body.

## Verification

All 6 quality gates MUST pass (zero warnings, zero failures):

- [ ] `cargo check` — Automatic
- [ ] `cargo check --features dev` — Automatic
- [ ] `cargo test` — Automatic — expected new baseline: existing-count + 14 lib tests + 1 integration test = ~14 lib + 1 integration NEW
- [ ] `cargo test --features dev` — Automatic
- [ ] `cargo clippy --all-targets -- -D warnings` — Automatic
- [ ] `cargo clippy --all-targets --features dev -- -D warnings` — Automatic

**New tests (~14 across files):**

| File | Test | Type |
|---|---|---|
| `src/data/classes.rs` | (existing `class_table_round_trips_through_ron` fixture updated to include new fields — counts as modified, not new) | unit |
| `src/data/races.rs` | `race_table_round_trips_through_ron` | unit |
| `src/data/races.rs` | `race_table_get_returns_authored_race` | unit |
| `src/plugins/party/progression.rs` | `xp_curve_matches_formula_fighter` (Mage L2=100, L3=150, L4=225) | unit |
| `src/plugins/party/progression.rs` | `xp_curve_saturates_at_u64_max` (factor=10, level=1000 → u64::MAX) | unit |
| `src/plugins/party/progression.rs` | `xp_curve_rejects_non_finite_factor` (factor=f32::NAN → u64::MAX or 0) | unit |
| `src/plugins/party/progression.rs` | `level_up_fighter_l1_to_l2_yields_correct_gains` | unit |
| `src/plugins/party/progression.rs` | `level_up_at_cap_returns_zero_gains` (level 99 → all-zero) | unit |
| `src/plugins/party/progression.rs` | `bonus_pool_roll_seeded_deterministic` (ChaCha8Rng seed=42 → known value) | unit |
| `src/plugins/party/progression.rs` | `bonus_pool_uses_class_authored_range` (Fighter min=5/max=9) | unit |
| `src/plugins/party/progression.rs` | `allocate_bonus_rejects_overflow` | unit |
| `src/plugins/party/progression.rs` | `allocate_bonus_applies_race_modifiers_via_saturating_add_signed` | unit |
| `src/plugins/party/progression.rs` | `can_create_class_enforces_min_stats` | unit |
| `src/plugins/party/progression.rs` | `can_create_class_enforces_allowed_races` | unit |
| `src/plugins/party/progression.rs` | `xp_threshold_triggers_level_up` (integration; full app) | integration |
| `src/plugins/town/guild_create.rs` | `creation_confirm_appends_to_pool` (integration; full app) | integration |
| `src/plugins/town/guild_create.rs` | `creation_rejects_class_below_min_stats` (integration; full app) | integration |
| `src/plugins/town/guild_create.rs` | `creation_pool_full_blocks_confirm` (integration; full app) | integration |
| `src/plugins/combat/turn_manager.rs` | `compute_xp_from_enemies_sums_half_max_hp` | unit |

**Verification commands by test:**

- [ ] Run `cargo test progression` — Automatic — covers all 11 progression unit/integration tests in one filter.
- [ ] Run `cargo test guild_create` — Automatic — covers the 3 guild_create integration tests.
- [ ] Run `cargo test races` — Automatic — covers the 2 race-data unit tests.
- [ ] Run `cargo test compute_xp_from_enemies` — Automatic — covers the XP-helper unit test.
- [ ] Run `cargo test class_table_round_trips_through_ron` — Automatic — confirms the modified ClassDef fixture round-trips with new fields populated.

**Manual smoke test (Manual — eyeballs only):**

- [ ] **RON load smoke** — `cargo run --features dev` and confirm no panic with `"failed to load asset"` for `assets/races/core.races.racelist.ron`. **Watch out for:** RON files use the DOUBLE-DOT extension `<name>.<type>.ron`. The new file MUST be named exactly `core.races.racelist.ron` (NOT `core.racelist.ron` or `races.ron`) and `RonAssetPlugin::<RaceTable>::new(&["racelist.ron"])` (NO leading dot). Single-dot won't load; round-trip unit tests don't catch this — only `cargo run` does. See [[project_druum_ron_asset_naming]].
- [ ] **End-to-end creation smoke** — Launch with `cargo run --features dev`, F9-cycle to Town, Esc to Square, walk to Guild, press `]` to start creation, walk through all 6 sub-modes, Confirm, return to Recruit mode, press Enter on the newly-listed character, confirm `PartyMember` count increases by 1.
- [ ] **Level-up smoke** — Launch with `cargo run --features dev`, F9-cycle to Combat, win a fight (use dev encounter), F9 to next state, return to Town/Guild Roster, confirm the party member's `Lv` increased by ≥1 (depends on XP curve vs combined `max_hp/2` from the dev encounter — Fighter's `xp_to_level_2 = 100` means ~200 HP of dead enemies pushes L1 → L2).
- [ ] **Class-roster filter smoke** — In CreateClass sub-mode, confirm only Fighter/Mage/Priest are listed; Thief/Bishop/Samurai/Lord/Ninja do not appear.

**Git operations (use GitButler, not raw git):**

- [ ] Stage uncommitted hunks to the feature branch: `but rub zz feature-19-character-creation`
- [ ] Commit: `but commit --message-file <path-to-msg>` (NEVER `git commit` on `gitbutler/workspace` — pre-commit hook blocks it). Note: `but commit` sweeps all unassigned hunks; verify with `but status` that no unrelated zz hunks pollute the commit.
- [ ] Push (runs husky pre-push hooks): `but push -u origin feature-19-character-creation`
