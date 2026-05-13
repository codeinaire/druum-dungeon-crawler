# Feature #19 — Character Creation & Class Progression — Research

**Researched:** 2026-05-13
**Domain:** Bevy 0.18.1 Town hub UI (egui), Wizardry-style DRPG character generation, per-class leveling, RNG-deterministic stat math
**Confidence:** HIGH (codebase claims), MEDIUM (Wizardry mechanics — historical), HIGH (Bevy 0.18 / rand 0.9 patterns — verified in tree)

---

## Summary

Feature #19 lands in a **far more pre-built scaffold than the roadmap line "+500 to +900 LOC" suggests**. All character data shape (`Race`, `Class`, `BaseStats`, `Experience`, `PartyMemberBundle`), the asset loader path (`RonAssetPlugin::<ClassTable>` registered, `core.classes.ron` already authored for Fighter/Mage/Priest with `growth_per_level`, `hp_per_level`, `mp_per_level`, `xp_to_level_2`, `xp_curve_factor`), and the recruit-flow seam (`RecruitPool` → `handle_guild_recruit` → `PartyMemberBundle` spawn) are **already in tree** from Features #11 / #18a / #18b. The seeded-RNG pattern (`rand 0.9 + rand_chacha 0.9 ChaCha8Rng::seed_from_u64`) is canonical across `damage.rs`, `targeting.rs`, `encounter.rs` — Feature #19 inherits it.

What is genuinely new for #19: (1) a **`RaceTable` asset + `races.racelist.ron`** (no `RaceData` type exists today); (2) **`progression.rs`** with `level_up(...) -> StatGains` consulting `ClassDef.growth_per_level` (the data is authored; the function isn't); (3) **`xp_to_next_level` recompute** + the threshold-cross system that drives it; (4) **the multi-step Guild "Create" wizard** in egui — a sub-mode of `GuildMode` that walks Race → Class → Roll/Allocate → Name → Confirm and ultimately calls **`RecruitPool` push** (not direct party spawn), reusing #18b's Recruit→PartyMemberBundle pipeline byte-for-byte; (5) the **XP hook** at `turn_manager.rs:634` (current `// 3. Victory.` branch is a single `next_state.set(GameState::Dungeon)` — no XP code exists in combat today, the roadmap text is correct that it needs adding); (6) the **class-change submode + advancement-graph data** (`advancement_requirements` field on `ClassDef`).

**Primary recommendation:** Build `progression.rs` first (pure level-up math + XP curve + combat-victory event), defer class-change to a follow-up sub-feature, treat the egui wizard as a Guild **sub-state machine** (extend `GuildMode` enum with `CreationStage` variants), persist creation result via the existing `RecruitPool` (Option-A landing — eliminates parallel pipelines and zero net new spawn code), and **author race modifiers + bonus-pool roll deterministically with `ChaCha8Rng::seed_from_u64(seed)` in production AND tests** so the rolled-bonus-pool flavor (recommended Pattern 5-B) doesn't introduce flaky tests.

---

## What This Lands Into (HIGH confidence — verified in tree)

### Character data model — already shipped by #11

`src/plugins/party/character.rs:39-194` defines (verified line numbers):

| Component | Variants / Fields | Notes |
|---|---|---|
| `Race` enum | `Human`, `Elf`, `Dwarf`, `Gnome`, `Hobbit` | All 5 declared; discriminant order locked for save-format stability (line 39-46). `Serialize`, `Deserialize`, `Reflect` derived. |
| `Class` enum | `Fighter`, `Mage`, `Priest`, `Thief`, `Bishop`, `Samurai`, `Lord`, `Ninja` | All 8 declared but only 3 authored in `core.classes.ron`. **Never write exhaustive `match Class { ... }` — always include wildcard** (line 56-58). |
| `BaseStats` | `strength`, `intelligence`, `piety`, `vitality`, `agility`, `luck` (all `u16`) | The Wizardry six. Has `BaseStats::ZERO` const. Round-trips RON cleanly (test `base_stats_round_trips_through_ron`, line 508). |
| `DerivedStats` | `max_hp`, `current_hp`, `max_mp`, `current_mp`, `attack`, `defense`, `magic_attack`, `magic_defense`, `speed`, `accuracy`, `evasion` (all `u32`, 0-100 scale on acc/eva) | Computed by `derive_stats`. **Caller-clamp pattern (OQ1):** `derive_stats` returns `current_hp = max_hp`; callers clamp on level-up (reset to max) or equip change (`current.min(new_max)`). |
| `Experience` | `level: u32`, `current_xp: u64`, `xp_to_next_level: u64` | **Cached `xp_to_next_level` (OQ2)** — recomputed by level-up system in #14 (deferred to #19 per `// updated by the level-up system in #14`). |
| `PartyMember` | ZST marker | Distinguishes from `Enemy`. |
| `PartySlot(usize)` | 0..PartySize.0 | Slot identity separable from `PartyMember` (line 187-191). |
| `PartyMemberBundle` | bundle helper | Includes `marker, name, race, class, base_stats, derived_stats, experience, party_row, party_slot, equipment, status_effects`. **Already the spawn vehicle from `handle_guild_recruit`.** |

### ClassDef schema — already shipped by #11

`src/data/classes.rs:50-69`:

```rust
pub struct ClassDef {
    pub id: Class,
    pub display_name: String,
    pub starting_stats: BaseStats,            // level-1 baseline pre-roll
    pub growth_per_level: BaseStats,          // additive per-level gain
    pub hp_per_level: u32,                    // deterministic HP gain
    pub mp_per_level: u32,                    // deterministic MP gain
    pub xp_to_level_2: u64,                   // base of XP curve
    pub xp_curve_factor: f32,                 // multiplier per level
}
```

`xp_to_next = xp_to_level_2 * curve_factor ^ (level - 1)` is the documented formula. Fighter / Mage / Priest authored in `assets/classes/core.classes.ron` (verified, lines 1-79).

**What's missing from `ClassDef`** that #19 needs (the planner must choose how to add — see Open Questions):
- `min_stats: BaseStats` (creation-time minimum to pick this class)
- `allowed_races: Vec<Race>` (race-class restriction matrix)
- `advancement_requirements: Vec<ClassRequirement>` (class-change graph)
- `bonus_pool_min: u32`, `bonus_pool_max: u32` (per-class roll range — Wizardry made Lord/Ninja require the high-roll path)
- `stat_penalty_on_change: BaseStats` (class-change penalty — likely zero, per QoL Option C)

### Recruit flow — already shipped by #18b

`src/plugins/town/guild.rs:307-390` — `handle_guild_recruit`:
1. Reads `RecruitPool` asset via `Res<Assets<RecruitPool>>`.
2. Clamps to `MAX_RECRUIT_POOL=32` (line 209).
3. Picks `recruit = recruits.get(cursor)`.
4. Spawns `PartyMemberBundle { name, race, class, base_stats, derived_stats: derive_stats(&base_stats, &[], &default_status, 1), party_row, party_slot: next_free_slot, ..Default::default() }` + `Inventory::default()`.
5. Marks `RecruitedSet.indices.insert(cursor)` (per-session dedup).

**The creation-flow end-state is: push a new `RecruitDef` into `RecruitPool.recruits`.** This is the load-bearing decision (see Open Question 4) — it means the creation flow does NOT need new spawn code; it appends to an existing `Vec<RecruitDef>` and lets the same Recruit handler do the spawn. **This is the cheapest path by far.**

### Combat-victory hook — NOT YET WIRED, line confirmed

`src/plugins/combat/turn_manager.rs:634-638`:

```rust
// 3. Victory.
if all_enemies_dead {
    combat_log.push("Victory!".into(), input_state.current_turn);
    next_state.set(GameState::Dungeon);
    return;
}
```

No XP, no gold (combat-gold is also deferred — `Gold::earn` exists but isn't called by combat). The XP hook is a **new system or a new in-place addition** at this exact line. Recommend: emit a `CombatVictoryEvent { total_xp: u32, total_gold: u32 }` message, consumed by a new `award_combat_xp` system in `plugins::party::progression`. This keeps `turn_manager.rs` free of party iteration and matches the existing event-handler pattern from #14 (`EquipmentChangedEvent`).

### RNG pattern — canonical across codebase

The codebase has a very clean precedent (verified in `data/encounters.rs:78-96`, `combat/damage.rs:62-67`, `combat/targeting.rs:37`, `combat/encounter.rs:85-96`):

- **Pure functions** take `rng: &mut (impl rand::Rng + ?Sized)`. The `?Sized` is required to permit `&mut *boxed_rng.0` from a `Box<dyn RngCore + Send + Sync>`.
- **Production-time** RNG lives in a resource wrapping `Box<dyn rand::RngCore + Send + Sync>`. `EncounterRng` (`combat/encounter.rs:91`) initialises via `Self(Box::new(SmallRng::from_os_rng()))`.
- **Tests** inject `rand_chacha::ChaCha8Rng::seed_from_u64(42)` directly. `rand_chacha` is a `[dev-dependencies]` entry — Feature #19 inherits it for free.
- `Cargo.toml:37` declares `rand 0.9` with `features = ["std", "std_rng", "small_rng", "os_rng"]`.

**For #19:** introduce `ProgressionRng(Box<dyn rand::RngCore + Send + Sync>)` initialised from `SmallRng::from_os_rng()`. Pure functions `roll_bonus_pool(class_def: &ClassDef, rng: &mut impl Rng) -> u32` and `level_up(...) -> StatGains` take `rng` by `&mut`. No new dep needed.

### egui patterns in tree

- `bevy_egui = 0.39.1`, `default-features = false, features = ["render", "default_fonts"]` (HIGH — Cargo.toml:28).
- Painter/handler split is enforced project-wide (Town doc, `src/plugins/town/mod.rs:111-160`):
  - Painters run in `EguiPrimaryContextPass`, are `Res<...>` / `Query<...>` only (no `ResMut`, no `Commands`).
  - Handlers run in `Update`, may mutate.
  - Both tuples use `.distributive_run_if(in_state(GameState::Town))`; per-system `.run_if(in_state(TownLocation::Guild))`.
- Patterns already in use: `egui::CentralPanel`, `egui::TopBottomPanel`, `egui::ScrollArea` (in `combat/ui_combat.rs`), `egui::Frame`, `egui::Window` (in combat), `egui::Color32`, `egui::Layout::right_to_left`, `egui::Align::Center`.
- Patterns NOT YET used in tree (new for #19): `egui::ComboBox`, `egui::Slider`, `egui::TextEdit::singleline`, `egui::ScrollArea::vertical().show_rows(...)`. All are stable bevy_egui 0.39 / egui 0.32 APIs.

---

## Architecture Options

### Option A — RecruitPool landing (RECOMMENDED, default)

The creation flow's last step is `pool.recruits.push(RecruitDef { name, race, class, base_stats, default_row })`. The user then enters the existing Recruit submode and presses Enter on the new entry, which spawns the `PartyMemberBundle` via `handle_guild_recruit`.

| Pros | Cons |
|---|---|
| **Zero new spawn pipeline** — reuses `handle_guild_recruit` byte-for-byte. | Two-step UX: "you've created the character, now recruit them" — needs a clear post-creation toast or auto-jump back to Recruit mode. |
| `MAX_RECRUIT_POOL=32` (declared `src/data/town.rs:46`) already bounds the pool. | A pure RON-loaded asset is now mutated at runtime — but `Assets<T>` mutation is the standard pattern (no schema concern). |
| **Aligns with #18b's plan-time anticipation:** "Recruit has NO minimum-active check (forward-compat with #19 Character Creation where the active roster starts empty)" (verified `guild.rs:14-19`). | If the player creates a character mid-dungeon (out of scope — Town only), the pool would grow indefinitely without re-init on save/load. Solved by #23 save-load. |
| Day-one and class-change creation reuse one path. | None blocking. |

### Option B — Direct party-slot spawn

Creation immediately spawns a `PartyMemberBundle` and assigns `PartySlot(next_free)`, skipping the pool entirely.

| Pros | Cons |
|---|---|
| One-step UX. | **Duplicates spawn code** with `handle_guild_recruit`, raising regression risk on bundle drift. |
| | Bypasses the `RecruitedSet` dedup (would need a parallel guard). |
| | Breaks #18b's symmetry: recruited NPCs go via RecruitPool; created PCs would go a different way → planner has to special-case status messages, inventory init, slot resolution, party-full checks. |
| | Requires touching `RecruitedSet` semantics or duplicating them. |

**Recommended:** Option A. The "two-step" UX concern dissolves once the creation handler auto-switches to `GuildMode::Recruit` with the cursor on the newly-appended index (one extra line of code).

### Counterarguments to Option A

- **"It mixes RON-authored and runtime-authored entries in the same `Vec`."** True, but the `Vec<RecruitDef>` schema is identical — there's no provenance flag, just a name string. If the planner cares to distinguish, add an optional `#[serde(default)] is_player_created: bool` field — but this is unnecessary for #19.
- **"It complicates save/load."** Feature #23 already needs custom serde for `RecruitPool` because it's an `Asset` not a `Resource`. The decision belongs to #23, not #19.

---

## Architecture Patterns

### Pattern 1 — `progression.rs` skeleton (new file)

```
src/plugins/party/progression.rs
├── ProgressionRng resource              // Box<dyn RngCore + Send + Sync>
├── CombatVictoryEvent message           // { xp: u32, gold: u32 }
├── pub fn level_up(...) -> StatGains    // pure: takes &ClassDef + &mut Rng
├── pub fn xp_for_next_level(level, def) -> u64
├── pub fn roll_bonus_pool(class, &mut Rng) -> u32
├── pub fn allocate_bonus_pool(base: &mut BaseStats, allocations: &[u16; 6], pool: u32) -> Result<(), AllocError>
├── apply_combat_xp_handler               // system: CombatVictoryEvent -> Experience++
├── apply_level_up_threshold_system       // system: checks current_xp >= xp_to_next_level; calls level_up
└── #[cfg(test)] mod tests                // 8-12 tests
```

The same painter/handler split as Town applies — `progression.rs` is data + handlers; UI lives in `guild.rs`.

### Pattern 2 — Guild creation wizard as sub-mode

Extend `GuildMode` in `src/plugins/town/guild.rs:104-111`:

```rust
pub enum GuildMode {
    #[default]
    Roster,                          // existing
    Recruit,                         // existing
    CreateRace,                      // NEW: race picker
    CreateClass,                     // NEW: class picker (filtered by selected Race)
    CreateRoll,                      // NEW: roll bonus pool + show
    CreateAllocate,                  // NEW: distribute bonus points
    CreateName,                      // NEW: TextEdit::singleline
    CreateConfirm,                   // NEW: summary screen + push to RecruitPool
}
```

A separate `Resource` holds the in-progress draft:

```rust
#[derive(Resource, Default, Debug)]
pub struct CreationDraft {
    pub race: Option<Race>,
    pub class: Option<Class>,
    pub rolled_bonus: u32,            // 0 if not yet rolled
    pub allocations: [u16; 6],        // STR, INT, PIE, VIT, AGI, LCK
    pub name: String,                 // <= MAX_NAME_LEN
    pub default_row: PartyRow,
}
```

**On `OnExit(TownLocation::Guild)` or `GuildMode -> Roster` reset:** clear the draft. Forgetting this leaks stale state between visits.

**Rationale for the enum-variant approach (over a `Option<CreationStage>` bool):** sub-states get free `.run_if(...)` gating on painters/handlers. `paint_guild_create_race` runs only in `GuildMode::CreateRace`. This is the same pattern Town uses with `TownLocation` sub-states, so the codebase reader's mental model is consistent.

### Pattern 3 — Wizardry-style "bonus pool" stat allocation

Wizardry-1 (Apple II / DOS 1981) allocation:
- After picking class candidate, the engine rolls bonus points: 80% chance of 5-9 points, 20% chance of 10-19 points.
- The player distributes those points across STR/IQ/PIE/VIT/AGI/LCK on top of the race's baseline starting stats.
- The class becomes selectable only if the resulting stats meet the class's minimum thresholds (e.g., Mage needs IQ ≥ 11; Bishop needs IQ ≥ 12 AND PIE ≥ 12).
- The pool is rolled ONCE per session and is non-rerollable; modern remakes (Wizardry 1 Renaissance, Wizardry Variants Daphne) add a re-roll button.

Sources for the mechanic specification:
- Wizardry-1 manual (1981, "Character Generation" section). MEDIUM confidence — historical document, only secondary digitisations available.
- StrategyWiki "Wizardry/Character creation" page (community-maintained). LOW confidence on exact numbers; HIGH confidence on the general roll-and-distribute flow.
- Wizardry Renaissance and Wizardry-The Five Ordeals (Steam, 2023) preserve the bonus-pool flavor with optional re-roll.

The numbers below should be treated as **a recommended starting point** that the user signs off on as Open Question 1:

| Bonus pool roll | Probability | Use case |
|---|---|---|
| 5 | ~16% | Common — cannot make a Bishop (needs ~24 points to satisfy both IQ ≥ 12 and PIE ≥ 12 over Human baseline) |
| 6 | ~16% | Common |
| 7 | ~16% | Common |
| 8 | ~16% | Common |
| 9 | ~16% | Common — borderline for Mage/Priest |
| 10-19 | ~20% (spread) | Rare — enables Samurai / Lord / Ninja |

A simpler MVP roll: `bonus = if rng.gen::<f32>() < 0.8 { rng.gen_range(5..=9) } else { rng.gen_range(10..=19) }`.

### Pattern 4 — Class-change graph as data, not code

`assets/classes/core.classes.ron` gains `advancement_requirements` per class:

```ron
(
    id: Bishop,
    advancement_requirements: [
        // Must be Mage at level >= 5 OR Priest at level >= 5.
        // Bishop is Wizardry-1: Mage L5 AND Priest L5 in the canonical reading;
        // a friendlier MVP is OR — the planner picks.
        (from_class: Mage, min_level: 5),
        (from_class: Priest, min_level: 5),
    ],
    min_stats: (intelligence: 12, piety: 12, ..),  // applied at class-change too
),
```

A pure function `can_change_class(char_class, char_level, char_stats, target_def) -> Result<(), ChangeError>` makes the graph **testable without an `App`**. The graph itself is data; only Rust code needed is the eligibility check + the "reset stats to new class minimums" mutation. **This is the pitfall surface area for "locked-out characters":** every test on the graph should assert *both* the eligible-to-change case AND the rejected case, per pair of classes.

For #19 MVP, **defer class-change to a sub-feature** (the roadmap's "what this touches" lists it but it's the cheapest thing to defer):
- Day-one: scaffold `advancement_requirements: Vec<ClassRequirement>` field on `ClassDef`, leave empty for Fighter/Mage/Priest. The data shape is locked.
- Day-two (sub-feature in a follow-up PR): wire the UI `GuildMode::ClassChange` and the eligibility check.

### Anti-Patterns to Avoid

- **Hand-rolling level-up XP-threshold checks per-frame inside a painter.** Painters are read-only. The threshold check belongs in `apply_level_up_threshold_system` in `Update`.
- **Reading `current_xp` from a draft resource and `Experience` from the entity in the same handler without a clear write order.** Use a single source of truth at a time: draft for in-progress creation, `Experience` for committed characters.
- **Calling `level_up` from inside `derive_stats`.** `derive_stats` is pure and side-effect-free — see file-level doc `character.rs:380`. Level-up *invokes* `derive_stats` after mutating `Experience.level` and `BaseStats`; the reverse direction would create a circular contract.
- **Putting `Race` modifiers inside the existing `Race` enum.** Race modifiers live in a `RaceData` asset (analogous to `ClassDef`). The enum stays a pure discriminant — same reverse-dep pattern as `data/classes.rs` (verified `data/classes.rs:5-12`).
- **Exhaustive `match Class { ... }`.** Always include a wildcard. Five of eight `Class` variants are declared-but-unauthored.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---|---|---|---|
| Weighted random pick (for rolled-bonus tier selection) | bespoke probability code | `rand::distr::weighted::WeightedIndex` (already used `data/encounters.rs:92`) | Numerically stable; cached cumulative array; constant-time sample. |
| Multi-step wizard state machine | manual `Resource<bool>` + `match step { 0 => ..., 1 => ... }` | `enum GuildMode` extension + per-variant `.run_if(in_state(...))` (free state-gating) | Matches Town's `TownLocation` sub-state pattern; zero new abstraction. |
| RNG seeding | `thread_rng()` (not deterministic across runs) | `ChaCha8Rng::seed_from_u64` in tests; `SmallRng::from_os_rng()` in prod | Codebase convention (5+ files). Tests must be deterministic. |
| Single-line text input | `egui::TextEdit::multiline` + manual newline rejection | `egui::TextEdit::singleline(&mut self.name)` + `.char_limit(MAX_NAME_LEN)` | Stable egui 0.32 API. |
| Stat slider | manual `+` / `-` buttons | `egui::Slider::new(&mut value, min..=max)` | Built-in keyboard-arrow integration; respects egui focus model. |
| Async RON re-load of a mutated `RecruitPool` | `commands.spawn` a fresh handle | `Assets<RecruitPool>::get_mut(&handle).recruits.push(def)` | Bevy 0.18 `Assets<T>` permits in-place mutation; the handle keeps pointing at the same internal id. |

---

## Common Pitfalls

### Pitfall 1: Locked-out characters from class-graph errors

**What goes wrong:** A character starts as a Mage, reaches L5, attempts to change to Bishop, but the advancement_requirements list contains a typo (`Mage` written but no entry; required AND not OR). The character is permanently stuck in their current class with no way forward.

**Why it happens:** Class-change is a graph and graphs have transitive properties that single-variable tests miss. Particularly: stat-penalty-on-change → character no longer meets the *original* class's min_stats → can't change back.

**How to avoid:**
- A test matrix: for every pair (from_class, to_class) where both are authored, assert eligibility-positive AND eligibility-negative cases.
- A "safety check" in the class-change handler: if `derive_stats(new_class, new_base_stats) < some_floor`, refuse the change with a clear toast.
- Recommended MVP: **no stat penalty on class change** (Open Question 6 = Option C). Solves 80% of the lock-out risk.

### Pitfall 2: Rolled-RNG flakiness in tests

**What goes wrong:** A test that depends on a creation flow's stat result uses `SmallRng::from_os_rng()` and fails intermittently. Tests should be deterministic.

**Why it happens:** Bevy app tests default to non-deterministic system order under `MinimalPlugins`. RNG calls from inside a system happen in non-deterministic order if multiple systems share the same `ResMut<ProgressionRng>`.

**How to avoid:**
- All level-up / bonus-roll math is a **pure function** taking `rng: &mut impl Rng`. Tests pass `ChaCha8Rng::seed_from_u64(42)` directly to the pure function (matches `damage.rs:185`, `encounter.rs:639`).
- The `ProgressionRng` resource only exists for the production handler that wraps the pure function. Tests don't insert it — they call the pure function directly.
- For tests of the level-up *system* (not the pure function), `app.insert_resource(ProgressionRng(Box::new(ChaCha8Rng::seed_from_u64(seed))))` before `app.update()`.

### Pitfall 3: `xp_to_next_level` cache drift

**What goes wrong:** A character's `Experience.xp_to_next_level` was set when the character was a Mage at level 4. The player class-changes to Priest. `xp_to_next_level` is still pointing at Mage's curve.

**Why it happens:** The `xp_to_next_level` field is **cached** (OQ2 in character.rs:163), not recomputed every read. Cache invalidation has the usual two-hard-problems status.

**How to avoid:**
- Write a single `recompute_xp_to_next_level(experience: &mut Experience, class: Class, table: &ClassTable)` helper that ALL callers (level-up, class-change, character-creation) invoke. Same single-source-of-truth pattern as `derive_stats`.
- Class-change handler MUST call this helper after updating `Class`.
- Test: `class_change_recomputes_xp_threshold`.

### Pitfall 4: RecruitPool indexing drift after creation push

**What goes wrong:** Player creates a character; the new `RecruitDef` lands at `recruits[5]` (after 5 RON-authored ones). The Recruit handler's `recruited.indices` set treats index 5 as a stable identifier across sessions. On a re-load, indices shift if the RON file is edited.

**Why it happens:** `RecruitedSet` uses positional indices (`HashSet<usize>`) — see `guild.rs:87-89`. Index 5 today might be a different `RecruitDef` after a content patch.

**How to avoid:**
- Document this as a known limitation for #19 (it's already a #23 save/load problem in the existing code).
- For #19: **just-created characters get auto-recruited immediately** (the creation `Confirm` step auto-switches to `Recruit` mode with cursor at the just-pushed index and confirms in the same handler call). Then `RecruitedSet.indices.insert(new_index)` is fired the same frame. The pool entry remains for save/load purposes but the active roster sees the character immediately.
- The risk only materialises if the player abandons creation half-way through `RecruitPool.recruits.push(...)`. Guard: only push to the pool on the final Confirm step, never mid-flow.

### Pitfall 5: Race modifier sign + saturating arithmetic

**What goes wrong:** Wizardry-1 race modifiers include **negative** values (Elf: STR -1, IQ +1; Dwarf: STR +2, AGI -1). `BaseStats` fields are `u16`. Subtracting 2 from a stat that starts at 1 underflows.

**Why it happens:** Wizardry's modifiers were applied to a value with implicit floor handling. Rust's `u16` does not.

**How to avoid:**
- Apply modifiers as `i16`: `let modified = (base as i16 + modifier as i16).max(3).min(18) as u16;` (Wizardry stats are clamped to 3..=18 by convention).
- Or: store `BaseStats` as `i16` internally and clamp on read (more invasive — not recommended).
- Or: author the *post-modifier* starting stats per (Race, Class) pair as the source of truth, and never apply modifiers in code. This is what #11 effectively did — `ClassDef.starting_stats` is the human-baseline; race modifiers don't exist in code yet. **This is the cleanest MVP path:** the `RaceData` is informational (UI display) and modifies the bonus_pool roll range or class-eligibility, NOT the starting stats. **The planner should consider this.**

### Pitfall 6: Cap the class roster, ship MVP

**What goes wrong:** Day-one shipment of all 8 classes with full advancement graphs makes the leveling system impossible to balance. Players hit edge cases (Ninja needing 17+ in every stat) before combat itself is fun.

**Why it happens:** The roadmap explicitly warns about this (line 1074-1075): "Resist shipping all 8 classes day one. Three classes is enough to validate the leveling system; expand once #15 combat is fun and balanced."

**How to avoid:**
- Day-one: **Fighter, Mage, Priest only.** (`core.classes.ron` already in this state.)
- Creation UI surfaces only authored classes by filtering `Class::iter()` against `ClassTable.get(c).is_some()` (the existing `get` returns `Option<&ClassDef>`, line 41-43).
- Day-two: Thief / Bishop / Samurai / Lord / Ninja are sub-features in follow-up PRs after #15 combat balance work.

### Pitfall 7: The `_state.rs` egui re-allocation per frame

**What goes wrong:** The painter allocates `Vec<&RecruitDef>` every frame via `clamp_recruit_pool`. With a multi-step wizard rendering more lists (filtered classes, allocations), the per-frame allocation cost grows.

**Why it happens:** egui is an immediate-mode UI library; some allocation per frame is expected. But the volume matters at 60fps.

**How to avoid:**
- Use `egui::ScrollArea::vertical().show_rows(ui, row_height, total_rows, |ui, row_range| { ... })` for the class/race list — it only paints visible rows, not all of them.
- Use `egui::ScrollArea::auto_shrink([false; 2])` for fixed-size panels so layout doesn't relayout on each frame.
- The egui complexity here is comparable to combat's `ui_combat.rs` — which is fine. Don't over-engineer.

---

## Security

### Known Vulnerabilities

| Library | CVE / Advisory | Severity | Status | Action |
|---|---|---|---|---|
| `bevy 0.18.1` | None found | — | Active dev branch | OK |
| `bevy_egui 0.39.1` | None found | — | Active dev branch | OK |
| `bevy_common_assets 0.16.0` | None found | — | Active dev branch | OK |
| `bevy_asset_loader 0.26.0` | None found | — | Active dev branch | OK |
| `leafwing-input-manager 0.20.0` | None found | — | Active dev branch | OK |
| `rand 0.9` | None found | — | Active dev branch | OK |
| `ron 0.12` | None found | — | Active dev branch | OK |

_(No CVEs against the recommended versions as of 2026-05-13, per `cargo audit`-equivalent inspection of the current Cargo.lock. Sources: rustsec.org/advisories. Feature #19 introduces **zero new direct dependencies** — Δ deps = 0.)_

### Architectural Security Risks

| Risk | Affected | How It Manifests | Secure Pattern | Anti-Pattern to Avoid |
|---|---|---|---|---|
| Crafted RON `classes.ron` with `hp_per_level: u32::MAX` | `level_up` | Stat saturation already handled in `derive_stats` via `saturating_*` ops; level-up gain must use same. | `experience.level.saturating_add(1)`; `max_hp = max_hp.saturating_add(class_def.hp_per_level)`. | Plain `+=`. |
| Crafted RON with `xp_curve_factor: f32::INFINITY` or `NaN` | `xp_for_next_level` | `u64` cast from `f32` becomes 0 (NaN), bypasses level-up gate; or panics in some math paths. | Clamp `xp_curve_factor` to `[1.0, 10.0]` at the trust boundary; reject `!is_finite()`. | Letting un-validated f32 propagate into u64 cast. |
| Crafted RON with `min_stats: BaseStats { strength: u16::MAX, .. }` | class-change eligibility | All class changes always rejected → no soft-lock from corrupted data, but silently un-fun. | Apply `min(18)` (Wizardry's hard cap) to all stat-minimum reads. | Letting authors set arbitrary values without a runtime cap. |
| Player-controlled `CreationDraft.name` field overflow | `RecruitDef.name`, eventually `CharacterName` | Memory bloat from a 100MB string; egui paints become slow. | `MAX_NAME_LEN: usize = 20` enforced via `TextEdit::char_limit(MAX_NAME_LEN)` AND at the trust boundary in the Confirm handler (`name.truncate(MAX_NAME_LEN)`). | No length limit. |
| Player-controlled stat allocation totals exceed pool | `allocate_bonus_pool` | Player creates infinitely powerful character. | `allocate_bonus_pool` returns `Err(AllocError::OverPool)` if `sum(allocations) > pool`. Confirm step refuses with a toast. | `unwrap()` or silently truncate. |
| Class-eligibility check bypass via state manipulation | `handle_create_confirm` | Player force-sets `CreationDraft.class = Class::Ninja` and skips the eligibility gate. | Re-validate eligibility in `handle_create_confirm` (defense-in-depth — never trust prior state). | Trusting that the painter filtered to eligible classes only. |

### Trust Boundaries

- **`classes.ron` / `races.racelist.ron` load** — clamp `min_stats` to `[3, 18]`, `hp_per_level` and `mp_per_level` to `[0, 64]` (Wizardry caps), `xp_to_level_2` to `[1, 1_000_000_000]`, `xp_curve_factor` to `[1.0, 10.0]` and reject non-finite. If skipped: crafted assets pivot the entire economy.
- **`CreationDraft.name` Confirm** — clamp to `MAX_NAME_LEN`. If skipped: memory bloat, painter slowness, save-file ballooning.
- **`CreationDraft.allocations` Confirm** — sum ≤ rolled_bonus. If skipped: infinite-stat exploit.
- **`CreationDraft.class` Confirm** — re-run `can_create_class(race, draft_base_stats, class_def)`. If skipped: state-manipulation bypass.
- **`CombatVictoryEvent.xp` write** — clamp to `[0, 1_000_000]` per event. If skipped: a single overflowing enemy XP value pushes a character to level 1000.

---

## Performance

| Metric | Value / Range | Source | Notes |
|---|---|---|---|
| Per-frame egui paint cost (Town screen) | <2ms estimated | Extrapolation from `ui_combat.rs` complexity vs. Town size | New screen adds ~30 widgets; not a bottleneck. |
| Bonus-pool roll per character creation | ~1µs | `rand::SmallRng` benchmark order-of-magnitude | One-shot per Confirm; negligible. |
| `level_up` per character | ~100 saturating mul/add ops, no allocations | Code review | <1µs; not a bottleneck. |
| `xp_for_next_level` (f32 pow recompute) | ~50ns | f32 powf order-of-magnitude | One call per level-up + one per class-change; cached otherwise. |
| RON load of `races.racelist.ron` (5 entries, ~1KB) | <5ms | `RonAssetPlugin` typical | Loaded once at boot via `bevy_asset_loader`. |
| Compile time delta | +0.3s | Roadmap line 1059 estimate | One new file (`progression.rs`) + RecruitDef/ClassDef field additions. |

_(No benchmarks were run; values are order-of-magnitude estimates. None of these metrics is likely to be a bottleneck for #19. Flag for validation: only if the Guild creation flow exceeds 16ms per frame in playtest.)_

---

## Code Examples

### Pure level-up function

```rust
// Source: NEW for #19, modeled on `derive_stats` (src/plugins/party/character.rs:380)
//          and `damage_calc` (src/plugins/combat/damage.rs:62).

/// Result of applying one level-up to a character.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StatGains {
    pub base_stats_delta: BaseStats,
    pub hp_gain: u32,
    pub mp_gain: u32,
    pub new_xp_to_next_level: u64,
}

/// Compute the stat changes from one level-up.
///
/// **PURE** — no mutations. Caller applies the gains to the character's
/// `BaseStats` + `Experience` and then re-derives `DerivedStats`.
///
/// `rng` is consumed if the class has stochastic growth in the future
/// (e.g., per-Wizardry-rule HP gain on a class-specific die roll). For
/// MVP, growth is deterministic — the `rng` parameter reserves the seam.
pub fn level_up(
    current: &BaseStats,
    current_level: u32,
    class_def: &ClassDef,
    _rng: &mut (impl rand::Rng + ?Sized),
) -> StatGains {
    let g = &class_def.growth_per_level;
    let base_stats_delta = BaseStats {
        strength: g.strength,
        intelligence: g.intelligence,
        piety: g.piety,
        vitality: g.vitality,
        agility: g.agility,
        luck: g.luck,
    };
    // Caller will saturate-add these to the character's current stats.
    StatGains {
        base_stats_delta,
        hp_gain: class_def.hp_per_level,
        mp_gain: class_def.mp_per_level,
        new_xp_to_next_level: xp_for_level(current_level + 1, class_def),
    }
}

pub fn xp_for_level(target_level: u32, class_def: &ClassDef) -> u64 {
    if target_level <= 1 {
        return 0;
    }
    // Clamp curve_factor at the trust boundary (defense against crafted RON).
    let factor = class_def.xp_curve_factor.clamp(1.0, 10.0) as f64;
    let exponent = (target_level - 1) as i32;
    let xp = (class_def.xp_to_level_2 as f64) * factor.powi(exponent);
    if !xp.is_finite() {
        return u64::MAX;
    }
    xp.clamp(0.0, u64::MAX as f64) as u64
}
```

### Bonus-pool roll (Wizardry-flavored, deterministic-testable)

```rust
// Source: NEW for #19, pattern from data/encounters.rs:78.

/// Roll the Wizardry-style bonus pool for character creation.
///
/// Roll: 80% chance of 5..=9 points, 20% chance of 10..=19 points.
/// Pure function — production wraps with a `ProgressionRng` resource;
/// tests pass a seeded `ChaCha8Rng` directly.
pub fn roll_bonus_pool(rng: &mut (impl rand::Rng + ?Sized)) -> u32 {
    use rand::Rng;
    if rng.random::<f32>() < 0.8 {
        rng.random_range(5..=9)
    } else {
        rng.random_range(10..=19)
    }
}
```

### Combat-victory XP event + handler

```rust
// Source: NEW for #19. Hook location: turn_manager.rs:634 (verified).

/// Emitted when combat ends in victory. Consumed by progression.
#[derive(Message, Debug, Clone, Copy)]
pub struct CombatVictoryEvent {
    pub total_xp: u32,
    pub total_gold: u32,
}

// In turn_manager.rs, replace the `// 3. Victory.` branch:
//
//     if all_enemies_dead {
//         combat_log.push("Victory!".into(), input_state.current_turn);
//         victory_writer.write(CombatVictoryEvent {
//             total_xp: compute_xp_from_enemies(&enemies),    // NEW pure helper
//             total_gold: 0,                                   // gold-from-combat deferred
//         });
//         next_state.set(GameState::Dungeon);
//         return;
//     }

/// Split total XP among living party members and update `Experience.current_xp`.
///
/// Triggers a level-up check in a follow-up system that runs after this one.
pub fn award_combat_xp(
    mut reader: MessageReader<CombatVictoryEvent>,
    mut party: Query<(&mut Experience, &StatusEffects), With<PartyMember>>,
) {
    for event in reader.read() {
        let alive: Vec<_> = party
            .iter_mut()
            .filter(|(_, s)| !s.has(StatusEffectType::Dead))
            .collect();
        let n = alive.len() as u32;
        if n == 0 {
            continue;
        }
        let per_member = event.total_xp / n;  // truncating div; remainder is lost
        for (mut exp, _) in alive {
            exp.current_xp = exp.current_xp.saturating_add(per_member as u64);
        }
    }
}
```

### Guild creation painter (skeleton — recommended structure)

```rust
// Source: NEW for #19, pattern from guild.rs:137-244 (verified) and shop.rs.

pub fn paint_guild_create_race(
    mut contexts: EguiContexts,
    draft: Res<CreationDraft>,
    race_assets: Res<Assets<RaceTable>>,
    town_assets: Option<Res<TownAssets>>,
    guild_state: Res<GuildState>,
) -> Result {
    let ctx = contexts.ctx_mut()?;
    egui::TopBottomPanel::top("create_race_header").show(ctx, |ui| {
        ui.heading("New Character — Step 1/5: Pick a Race");
    });
    egui::CentralPanel::default().show(ctx, |ui| {
        let race_table = town_assets
            .as_ref()
            .and_then(|a| race_assets.get(&a.race_table));
        match race_table {
            None => ui.label("(loading races...)"),
            Some(table) => {
                for (idx, race_def) in table.races.iter().enumerate() {
                    let marker = if idx == guild_state.cursor { "> " } else { "  " };
                    ui.label(format!(
                        "{}{} ({})  STR{:+} INT{:+} PIE{:+} VIT{:+} AGI{:+} LCK{:+}",
                        marker,
                        race_def.display_name,
                        race_def.description,
                        race_def.stat_modifiers.strength as i16,
                        race_def.stat_modifiers.intelligence as i16,
                        race_def.stat_modifiers.piety as i16,
                        race_def.stat_modifiers.vitality as i16,
                        race_def.stat_modifiers.agility as i16,
                        race_def.stat_modifiers.luck as i16,
                    ));
                }
                ui.add_space(8.0);
                ui.label("[Up/Down] Pick  |  [Enter] Confirm  |  [Esc] Cancel creation")
            }
        }
    });
    Ok(())
}
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|---|---|---|---|
| Wizardry-1 fixed `BaseStats` from race-class table | Wizardry-style rolled bonus pool over baseline | Wizardry-1 → Wizardry V (1988) | Player agency at the cost of grind on bad rolls. |
| Hand-rolled XP curves per class (separate lookup tables) | Single formula `xp_to_level_2 * factor^(level-1)` per class | Modern indie DRPGs (Etrian Odyssey, Coromon) | Half the data; identical pacing tunability via two numbers. |
| Hardcoded stat-allocation in code | Asset-driven `ClassDef.starting_stats + growth_per_level` | Idiomatic Bevy / data-driven design | Already shipped in #11. |
| `Vec<RecruitDef>` indexed-by-position | Same, with optional unique-id field (deferred to #23) | Save-load era | Indexing-by-position is OK for #19; #23 will revisit. |

**Deprecated/outdated:**
- **`rand 0.8`** is the older API (`gen_range` was renamed to `random_range` in 0.9). Codebase is on 0.9 — use 0.9 names.
- **`rand::distributions`** (renamed to `rand::distr` in 0.9). The `WeightedIndex` is now `rand::distr::weighted::WeightedIndex` (verified `data/encounters.rs:92`).
- **`bevy::utils::HashMap`** is removed in Bevy 0.18 (verified in `data/classes.rs:39-40`). Use `std::collections::HashMap` only if a hot-path lookup is needed; a linear scan over 8 classes is fine.

---

## Validation Architecture

### Test Framework

| Property | Value |
|---|---|
| Framework | Cargo's built-in `#[test]` + `cargo test` |
| Config file | None — `[dev-dependencies] rand_chacha = "0.9"` in `Cargo.toml:40` is all that's needed |
| Quick run command | `cargo test --features dev progression` (filter to new file) |
| Full suite command | `cargo test` |
| Bevy app harness pattern | `MinimalPlugins + StatesPlugin + AssetPlugin::default() + init_state::<GameState>() + add_sub_state::<TownLocation>()` — verified pattern in `guild.rs:649-712` |

### Requirements → Test Map

| Requirement | Behavior | Test Type | Automated Command | File Exists? |
|---|---|---|---|---|
| `level_up` gains stats from `growth_per_level` | Pure-fn test: Fighter L1 → L2 yields STR+2 VIT+2 AGI+1 | unit | `cargo test level_up_fighter_l1_to_l2` | NO — new in `progression.rs` |
| `level_up` HP gain matches `hp_per_level` | Pure-fn test: Fighter +8 HP, Mage +4 HP | unit | `cargo test level_up_hp_gain_per_class` | NO |
| `xp_for_level` matches curve formula | Pure-fn test: Mage L2 = 100, L3 = 150 (factor 1.5), L4 = 225 | unit | `cargo test xp_curve_matches_formula` | NO |
| `xp_for_level` saturates on extreme inputs | Pure-fn test: factor=10, level=1000 → u64::MAX | unit | `cargo test xp_curve_saturates_at_u64_max` | NO |
| `roll_bonus_pool` is seed-deterministic | Pure-fn test: same seed → same pool | unit | `cargo test bonus_pool_roll_seeded_deterministic` | NO |
| `roll_bonus_pool` 80/20 distribution | Pure-fn test: 10000 samples, 80% in 5..=9 | unit | `cargo test bonus_pool_distribution_80_20` | NO |
| `allocate_bonus_pool` rejects over-allocation | Pure-fn test: sum > pool → Err | unit | `cargo test allocate_bonus_rejects_overflow` | NO |
| `can_create_class` enforces race-class matrix | Pure-fn test: Elf as Fighter → Err if disallowed | unit | `cargo test creation_race_class_matrix` | NO |
| `can_create_class` enforces stat minima | Pure-fn test: STR < class min → Err | unit | `cargo test creation_stat_minima_rejected` | NO |
| `can_change_class` graph eligibility (positive) | Pure-fn test: Mage L5 → Bishop eligible | unit | `cargo test class_change_bishop_eligible` | NO |
| `can_change_class` graph eligibility (negative) | Pure-fn test: Mage L4 → Bishop rejected | unit | `cargo test class_change_bishop_below_level` | NO |
| `apply_combat_xp_handler` splits XP among alive | Integration: 3 alive + 1 dead, 300 XP → 100 per alive | integration | `cargo test combat_xp_splits_among_alive` | NO |
| `apply_level_up_threshold_system` triggers level-up | Integration: bump XP past threshold → level += 1, recompute stats | integration | `cargo test xp_threshold_triggers_level_up` | NO |
| Creation Confirm pushes to RecruitPool | Integration: complete wizard → `pool.recruits.len() += 1` | integration | `cargo test creation_confirm_appends_to_pool` | NO |

### Gaps (files to create before implementation)

- [ ] `src/plugins/party/progression.rs` — pure functions + handlers + tests (~250 LOC)
- [ ] `src/plugins/town/guild_create.rs` (OR extend `guild.rs`) — creation wizard painters + handlers (~300 LOC)
- [ ] `src/data/races.rs` — `RaceData`, `RaceTable` asset + RON loader registration (~80 LOC)
- [ ] `assets/races/core.races.racelist.ron` — 5 race definitions (~50 lines)
- [ ] Extension to `src/data/classes.rs` — add `min_stats`, `allowed_races`, `advancement_requirements`, `bonus_pool_min/max` fields to `ClassDef` (~40 LOC including round-trip test fixture update)
- [ ] Extension to `assets/classes/core.classes.ron` — populate new fields for Fighter/Mage/Priest (~20 LOC)
- [ ] Extension to `src/plugins/loading/mod.rs` — register `RonAssetPlugin::<RaceTable>` + `TownAssets.race_table` (~10 LOC)
- [ ] Extension to `src/plugins/combat/turn_manager.rs` — emit `CombatVictoryEvent` (~5 LOC) at line 634

**Recommended file split:**
- Creation UI lives in a new `guild_create.rs` sibling rather than ballooning `guild.rs`. `mod.rs` re-exports.
- This keeps `guild.rs` at ~1150 LOC (current) and isolates the wizard's complexity, matching the codebase precedent of separating combat into `damage.rs`, `targeting.rs`, `ai.rs`, etc.

---

## What to Reuse vs Build New

| Need | Reuse / Build |
|---|---|
| `Race`, `Class` enums | **Reuse** — both fully declared in `character.rs:39,62`. No additions. |
| `BaseStats`, `Experience`, `PartyMemberBundle` | **Reuse** — `character.rs:98,159,321`. No additions. |
| `ClassDef`, `ClassTable` | **Extend** — add `min_stats`, `allowed_races`, `advancement_requirements`, `bonus_pool_min`, `bonus_pool_max` to `ClassDef`. `core.classes.ron` updates. |
| `RecruitDef`, `RecruitPool` | **Reuse** — `data/town.rs:133,151`. Creation flow pushes onto `RecruitPool.recruits`. |
| `RecruitedSet` | **Reuse** — `guild.rs:87`. Creation flow's final-step inserts the new index immediately. |
| `derive_stats` | **Reuse** — `character.rs:380`. Called by level-up handler post-mutation. |
| Painter/handler split + `distributive_run_if(in_state(GameState::Town))` | **Reuse** — pattern enforced project-wide. New systems follow it. |
| `MAX_RECRUIT_POOL`, `MAX_INVENTORY_PER_CHARACTER` | **Reuse** — `data/town.rs:46`, `town/shop.rs:45`. |
| RNG (`ProgressionRng` + `ChaCha8Rng` in tests) | **Build new resource** — mirror `EncounterRng` pattern. |
| `RaceData`, `RaceTable` asset | **Build new** — does not exist. Mirror `ClassDef`/`ClassTable` exactly. |
| `progression.rs` | **Build new** — new file in `src/plugins/party/`. |
| `CombatVictoryEvent` | **Build new** — `Message` derive, registered in `PartyPlugin::build`. |
| `CreationDraft` resource | **Build new** — `Resource` derive, init in `TownPlugin::build`. |
| `GuildMode::CreateXxx` variants | **Extend** — `guild.rs:104-111`. |
| Multi-step UI patterns (`egui::ComboBox`, `egui::Slider`, `egui::TextEdit::singleline`) | **Build new** — first uses in the codebase. |
| Stat allocation buttons (+/- per stat) | **Build new** — but: use `egui::Slider` from the start. |
| `level_up` function | **Build new** — pure fn in `progression.rs`. |
| `xp_for_level` function | **Build new** — pure fn in `progression.rs`. |
| `can_create_class`, `can_change_class` functions | **Build new** — pure fns in `progression.rs`. |
| `roll_bonus_pool`, `allocate_bonus_pool` functions | **Build new** — pure fns in `progression.rs`. |
| `MenuAction` enum | **Reuse** — `Up/Down/Left/Right/Confirm/Cancel` sufficient. No new variants. |
| `Inventory::default()` for new characters | **Reuse** — same as `handle_guild_recruit:376`. |
| `Toasts::push` for feedback | **Reuse** — `toast.rs:42`. |
| Saturating arithmetic | **Reuse pattern** — every stat-mutation in `progression.rs` uses `saturating_*` (matches `derive_stats` discipline). |

---

## Open Questions for Planner

Surface these as a batch for the user before implementation:

### Q1 — Stat allocation flavor

Three flavors to choose from:

| Option | Description | UX | Authenticity (Wizardry-1) |
|---|---|---|---|
| **1A** Pure Wizardry bonus-pool | Roll 5-9 (80%) or 10-19 (20%) once; distribute among 6 stats subject to class minima; NO re-roll | Punishing on bad rolls (waiting for a high-roll session) | Highest |
| **1B** Bonus-pool with re-roll (RECOMMENDED MVP) | Same roll mechanic; player can spam a "Re-Roll" button before allocation | Friendlier; preserves rolled-bonus tension | High |
| **1C** Pure point-buy | Fixed budget (e.g., 12 points) per class with no RNG | Most predictable; loses Wizardry feel | Lowest (modern DRPG QoL) |

**Recommendation:** 1B. Preserves Wizardry's roll-tension without trapping the player. If the user picks 1A, an "I want to abandon this character" early-exit prevents lock-out.

### Q2 — MVP class roster

| Option | Classes |
|---|---|
| **2A** Three classes (RECOMMENDED) | Fighter, Mage, Priest |
| **2B** Four classes (Wizardry's "base four") | Fighter, Mage, Priest, **Thief** |
| **2C** All eight | Fighter, Mage, Priest, Thief, Bishop, Samurai, Lord, Ninja |

**Recommendation:** 2A — matches the roadmap warning verbatim and `core.classes.ron`'s existing state. Day-two PR adds Thief once a thief-relevant combat verb exists (lockpick / steal — neither shipped). Class enum has all 8 variants declared; the `ClassTable::get` returns `None` for unauthored ones, so the creation UI naturally filters them out.

### Q3 — Race set day-one

| Option | Races | Race modifier authoring |
|---|---|---|
| **3A** (RECOMMENDED) | Human, Elf, Dwarf, Gnome, Hobbit | Author balanced (-2..=+2) modifiers per stat, displayed in creation UI |
| **3B** | Human only (single-race MVP) | No modifier system needed |
| **3C** | Human + 1 fantasy (e.g. Elf) | Half-cost authoring |

**Recommendation:** 3A — `Race` enum is already declared with all 5 variants. `RaceData` authoring is ~50 RON lines for the full set. The cost difference between 3A and 3B is minimal.

### Q4 — Creation destination

| Option | Destination |
|---|---|
| **4A** (RECOMMENDED) | `RecruitPool.recruits.push(...)` — auto-recruit on Confirm |
| **4B** | Direct `PartyMemberBundle` spawn with next-free `PartySlot` |

**Recommendation:** 4A. Reuses `handle_guild_recruit` byte-for-byte, eliminates parallel pipelines, and matches the codebase's plan-time anticipation (`guild.rs:14-19`).

### Q5 — XP curve

| Option | Description | LOC | Tunability |
|---|---|---|---|
| **5A** (RECOMMENDED) | Per-class formula already in `ClassDef`: `xp_to_level_2 * curve_factor^(level-1)` | 0 (data already shipped) | High — tune two numbers per class |
| **5B** | Per-class lookup table (`Vec<u64>` of size 13) | +200 LOC of authoring | Highest |
| **5C** | Shared curve with per-class multiplier | -50 LOC | Lowest |

**Recommendation:** 5A. The data is already in `core.classes.ron`. Just write the function that consumes it.

### Q6 — Class-change stat penalty

| Option | Description |
|---|---|
| **6A** | Full reset to class minima (Wizardry-style; brutally punishing) |
| **6B** | Partial reduction (e.g., -3 to every stat) |
| **6C** (RECOMMENDED MVP) | None — player keeps stats, just gains access to new class growth |

**Recommendation:** 6C. Eliminates the "Pitfall 1 locked-out character" risk almost entirely. A future polish PR can re-introduce penalty if the leveling system feels too easy.

### Q7 — Level cap

| Option | Cap |
|---|---|
| **7A** | 13 (Wizardry-1's soft cap; level 13+ XP went to gold) |
| **7B** (RECOMMENDED MVP) | 99 (modern DRPG convention; matches floor scaling) |
| **7C** | 50 (mid-tier compromise) |

**Recommendation:** 7B. Floor-1 enemies in `core.enemies.ron` are scaled to early levels; if combat balance lifts to floor-30+ later, level 99 gives headroom. `xp_for_level(99, fighter_def)` with factor 1.5 saturates to u64::MAX — harmless (the player will plateau).

---

## Risks / Pitfalls (summary)

1. **Class-change graph "locked-out character"** — pitfall #1 above. Mitigation: pick Q6 = Option C (no penalty) for MVP + comprehensive pair-wise eligibility tests.
2. **Rolled-RNG test flakiness** — pitfall #2. Mitigation: pure functions take `rng: &mut impl Rng`; tests pass `ChaCha8Rng::seed_from_u64(42)` directly; matches `damage.rs` pattern (verified).
3. **`xp_to_next_level` cache drift on class-change** — pitfall #3. Mitigation: single `recompute_xp_to_next_level` helper called by all mutators.
4. **`RecruitPool` indexing drift across save/load** — pitfall #4. Mitigation: documented limitation, deferred to #23.
5. **Race modifier underflow on `u16`** — pitfall #5. Mitigation: `RaceData` modifies the *bonus_pool* range, not `starting_stats` directly (cleanest). OR cast to `i16`, clamp `[3, 18]`, cast back.
6. **Premature 8-class ship** — pitfall #6. Mitigation: Q2 = Option A (3 classes only).
7. **egui per-frame allocation cost in creation wizard** — pitfall #7. Mitigation: `ScrollArea::show_rows` for any list > 10 items.
8. **`CreationDraft` leak between Guild visits** — covered in Pattern 2. Mitigation: reset on `OnExit(TownLocation::Guild)`.
9. **Crafted-RON denial-of-service in `classes.ron`** — covered in Security. Mitigation: trust-boundary clamps on all numeric fields.
10. **Eligibility bypass via state manipulation** — covered in Security. Mitigation: re-validate in `handle_create_confirm`.

---

## LOC Estimate Breakdown

Targeted against the roadmap budget of **+500 to +900 LOC** (line 1059):

| File | Action | Estimated LOC |
|---|---|---|
| `src/plugins/party/progression.rs` | NEW | ~270 (pure fns 100, handlers 60, tests 110) |
| `src/plugins/town/guild_create.rs` | NEW | ~280 (5 painters + 5 handlers + helpers) |
| `src/data/races.rs` | NEW | ~75 (data structs + 1 RON round-trip test) |
| `assets/races/core.races.racelist.ron` | NEW | ~50 |
| `src/data/classes.rs` | EXTEND (new fields + test update) | ~40 |
| `assets/classes/core.classes.ron` | EXTEND (populate new fields) | ~30 |
| `src/plugins/loading/mod.rs` | EXTEND (register RaceTable + TownAssets.race_table) | ~10 |
| `src/plugins/town/mod.rs` | EXTEND (wire new systems, init CreationDraft) | ~25 |
| `src/plugins/town/guild.rs` | EXTEND (extend GuildMode enum, dispatch from Roster) | ~30 |
| `src/plugins/combat/turn_manager.rs` | EXTEND (emit CombatVictoryEvent) | ~10 |
| `src/plugins/party/mod.rs` | EXTEND (re-exports, register CombatVictoryEvent) | ~15 |
| **Total** | | **~835 LOC** |

**Roadmap fit:** 835 LOC is in the 500-900 budget — toward the high end but within. If the planner picks Q1 = Option A (no re-roll button) instead of 1B, save ~30 LOC on the re-roll handler. If Q2 = Option B (add Thief), add ~80 LOC for Thief authoring + tests.

**Test count target:** roadmap line 1061 says `+8-12 tests`. The Validation Architecture table above lists 14 tests; comfortably above the target. If you trim the 80/20 distribution test and the saturation test (both arguably belt-and-suspenders), you're at 12.

---

## Sources

### Primary (HIGH confidence — codebase or local Bevy source)

- [`src/plugins/party/character.rs`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/party/character.rs) — `Race`, `Class`, `BaseStats`, `Experience`, `PartyMemberBundle`, `derive_stats` definitions and tests. Verified lines 39-829.
- [`src/data/classes.rs`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/data/classes.rs) — `ClassDef`, `ClassTable`, `get` linear-scan rationale. Verified lines 17-160.
- [`assets/classes/core.classes.ron`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/assets/classes/core.classes.ron) — Fighter/Mage/Priest fully authored, including XP curve fields. Verified 1-79.
- [`src/plugins/town/guild.rs`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/town/guild.rs) — `GuildMode`, `handle_guild_recruit`, `RecruitedSet`, painter/handler patterns. Verified lines 47-1145.
- [`src/data/town.rs`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/data/town.rs) — `RecruitDef`, `RecruitPool`, `MAX_RECRUIT_POOL=32`. Verified 1-447.
- [`src/plugins/combat/turn_manager.rs:596-646`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/combat/turn_manager.rs) — `check_victory_defeat_flee`, exact XP-hook insertion site. Verified.
- [`src/plugins/combat/damage.rs:62-67`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/combat/damage.rs) — canonical `rng: &mut (impl Rng + ?Sized)` signature pattern.
- [`src/data/encounters.rs:78-96`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/data/encounters.rs) — `pick_group` weighted-random pattern using `rand::distr::weighted::WeightedIndex`.
- [`src/plugins/combat/encounter.rs:85-96`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/combat/encounter.rs) — `EncounterRng` resource pattern (`Box<dyn rand::RngCore + Send + Sync>` + `SmallRng::from_os_rng()`).
- [`Cargo.toml`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/Cargo.toml) — dep versions: `bevy 0.18.1`, `bevy_egui 0.39.1`, `rand 0.9`, `rand_chacha 0.9` (dev), `ron 0.12`. Verified 1-56.
- [`src/plugins/loading/mod.rs`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/loading/mod.rs) — `RonAssetPlugin<T>` registration with double-dot extension naming. Verified 113-165.
- [`src/plugins/state/mod.rs`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/state/mod.rs) — `GameState::Town`, `TownLocation::Guild`. Verified 7-46.
- [`project/roadmaps/20260429-01-bevy-dungeon-crawler-roadmap.md`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/project/roadmaps/20260429-01-bevy-dungeon-crawler-roadmap.md) — Feature #19 spec, lines 1029-1076.

### Secondary (MEDIUM confidence — pattern memory, not re-verified online for this report)

- Bevy 0.18 `Message` API (replaces 0.17 `Event` for many use cases) — corroborated by `MessageReader`/`MessageWriter` usage in `temple.rs:274` and saved memory entry `feedback_bevy_0_18_event_message_split.md`.
- bevy_egui 0.39 `EguiContexts::ctx_mut()` returns `Result` — corroborated by `guild.rs:147` and other Town painters.
- `rand 0.9` API renames (`gen_range` → `random_range`, `rand::distributions` → `rand::distr`) — verified in `encounters.rs:92` and `damage.rs`.

### Tertiary (LOW confidence — historical/external — flagged for planner validation)

- Wizardry-1 (1981) bonus-pool roll: 80% chance of 5-9, 20% chance of 10-19. **Source:** StrategyWiki Wizardry/Character Creation page + community wiki digitisations of the 1981 manual. Treat the exact percentages as a balanced *starting point* the user signs off on, not a religious requirement.
- Wizardry-1 class minimum stats: Mage IQ≥11, Priest PIE≥11, Bishop IQ≥12 PIE≥12, Samurai STR≥15 IQ≥11 PIE≥10 VIT≥14 AGI≥10, Lord STR≥15 PIE≥12 VIT≥15 AGI≥14 LCK≥15, Ninja all ≥17. **Source:** Wizardry-1 Apple II manual via community wikis. Treat as design starting point.
- Wizardry-1 class advancement graph: Bishop requires Mage+Priest, Samurai requires Fighter+Mage, Lord requires Fighter+Priest, Ninja requires all four base classes. **Source:** same. Day-one MVP omits all four advanced classes anyway.
- Race modifiers (Elf +0/+2/-2/+0/+1/-1 style flat ranges): **historical Wizardry tables**. Recommend the planner ask the user what feels right for the Druum tone rather than mechanically transcribing Wizardry. The cleanest MVP: **race modifiers shift the rolled-bonus-pool range, NOT the starting stats**, sidestepping the u16-underflow pitfall.

---

## Metadata

**Confidence breakdown:**

- Existing codebase patterns (Race/Class/ClassDef/RecruitPool/RNG/painter/handler): **HIGH** — verified line-by-line.
- Combat XP hook location (`turn_manager.rs:634`): **HIGH** — verified, no XP code exists today.
- Bevy 0.18.1 / bevy_egui 0.39.1 patterns: **HIGH** — corroborated in tree + saved memory.
- `rand 0.9` API usage and seeded-determinism pattern: **HIGH** — corroborated in tree (5+ files).
- Wizardry-1 mechanics (bonus-pool roll, class minima, advancement graph): **MEDIUM** — community-corroborated historical data; recommend treating exact numbers as MVP starting points, not specifications.
- Race modifier design: **LOW** — multiple valid approaches; the planner should let the user choose between (a) modify bonus-pool range, (b) modify starting_stats with i16-arithmetic, (c) skip race modifiers in MVP. Recommend (a).
- Stat-allocation flavor (Q1): **LOW** — design decision, surface to user.
- Class-change stat penalty (Q6): **LOW** — design decision, surface to user. Recommend C (none) to dodge the lock-out pitfall.

**Research date:** 2026-05-13
**Working-tree state at research time:** `gitbutler/workspace` with uncommitted hunks in `src/plugins/town/{guild,inn,shop,temple,gold,mod}.rs` and a new `src/plugins/town/toast.rs`. The toast module is `Resource<Toasts>` (verified) used by guild/inn/shop/temple for user feedback — Feature #19 should reuse it. The other uncommitted hunks are the #18b PR-review response (per `project/reviews/20260513-000000-feature-18b-town-temple-guild-pr-review.md`) and do not block #19.
**Δ Cargo.toml:** 0 — Feature #19 introduces no new direct dependencies. (`rand 0.9` and `rand_chacha 0.9` already declared.)
**Δ deps total:** 0 — also no transitive additions.
