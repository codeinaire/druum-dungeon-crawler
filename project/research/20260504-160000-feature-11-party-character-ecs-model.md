# Feature #11 — Party & Character ECS Model — Research

**Researched:** 2026-05-04
**Domain:** Druum / Bevy 0.18.1 / DRPG character & party data layer
**Confidence:** HIGH on Bevy 0.18 API shape, ECS patterns, project conventions, and dep delta. MEDIUM on AD&D-derived stat conventions (genre training data; no live source extraction). HIGH on the eight Decision recommendations because each has independent precedent in the existing codebase.

## Summary

Feature #11 adds the character data layer that every later feature reads from. Twelve components, one bundle, one pure `derive_stats(...)` function, one `PartySize` resource, one dev-gated `spawn_default_debug_party` system, one new RON asset (the schema for `ClassTable` — the type already exists as a stub from #3), and zero new dependencies. The roadmap's `assets/data/classes.ron` line is wrong about the path — the existing `RonAssetPlugin::<ClassTable>::new(&["classes.ron"])` registration in `src/plugins/loading/mod.rs:100` and the stub asset at `assets/classes/core.classes.ron` lock the convention.

The whole feature is "components-as-data" (master research §Pattern 3 / §Anti-Patterns line 973): every type is `#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq)]` from the start (master research §Pitfall 5), every `derive_stats` input is plain data, equipment effects stack additively in v1 (genre-canonical, easy to validate, easy to hand-tune), status effects modify the result, and the function is pure (no `Mut<T>`, no entity lookups, no resource reads). The only meaningful design choice with downstream blast radius is **Decision 3 (Equipment storage: `Entity` vs `Handle<ItemAsset>`)** because it determines whether Feature #23 (save/load) needs the `MapEntities` trait dance or can serialize the field as-is.

`PartyPlugin` already exists as an empty stub at `src/plugins/party/mod.rs` and is already registered in `src/main.rs:32`. The end-state goal is **`src/main.rs` byte-unchanged** — same cleanest-ship signal as Features #9 and #10. `ClassTable` already exists as a stub at `src/data/classes.rs` and is already wired to the loader. This means Feature #11 is almost entirely additive: new code in `src/plugins/party/` and a fleshed-out `src/data/classes.rs`. Cargo.toml is byte-unchanged; Cargo.lock is byte-unchanged.

**Primary recommendation:** Implement the 12 components + bundle + `derive_stats` in a single `src/plugins/party/character.rs` file (Decision 4: deviate from the multi-file roadmap split, follow the #9 single-file precedent). Use `Handle<ItemAsset>` for `Equipment` slots NOT `Entity` (Decision 3: kills the dangling-reference + `MapEntities` retrofit risk for Feature #23 — but defers item-instance state like enchantment level until Feature #12). Spawn the debug party on `OnEnter(GameState::Loading)` under `#[cfg(feature = "dev")]` (Decision 5: matches roadmap line 596 and the project's `dev` feature gating from `cycle_game_state_on_f9`). Ship 3 classes (Fighter/Mage/Priest), 1 race (Human), `PartySize: Resource(usize)` defaulting to 4 as a hard cap, 5 status effects (Poison/Sleep/Paralysis/Stone/Dead). Surface all 8 decisions for plan-approval before implementation.

---

## Standard Stack

### Core (already in deps — no Δ)

| Library | Version | Purpose | License | Maintained? | Why Standard |
|---------|---------|---------|---------|-------------|--------------|
| [bevy](https://crates.io/crates/bevy) | =0.18.1 | `#[derive(Component)]`, `Bundle`, `Resource`, `Plugin`, state hooks | MIT/Apache-2.0 | Active | Engine — already in deps |
| [serde](https://crates.io/crates/serde) | 1 (`derive` feature) | `#[derive(Serialize, Deserialize)]` on every component | MIT/Apache-2.0 | Active | Already in `[dependencies]` (Rust 2024 edition mandates explicit re-declare even though Bevy pulls it transitively) |
| [ron](https://crates.io/crates/ron) | 0.12 | Pure-stdlib round-trip tests for `ClassTable` and per-component shapes | MIT/Apache-2.0 | Active | Already in `[dependencies]` for the same Rust 2024 reason |
| [bevy_common_assets](https://crates.io/crates/bevy_common_assets) | =0.16.0 (ron 0.11 internally) | `RonAssetPlugin::<ClassTable>` — already registered in `src/plugins/loading/mod.rs:100` | MIT/Apache-2.0 | Active | Already in deps; loads `*.classes.ron` files |
| [bevy_asset_loader](https://crates.io/crates/bevy_asset_loader) | =0.26.0 | `class_table: Handle<ClassTable>` already in `DungeonAssets` | Apache-2.0 | Active | Already in deps; the only edit is filling out `ClassTable`'s body, no new asset entry |

### Supporting (NOT used in #11)

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| [smallvec](https://crates.io/crates/smallvec) | (transitive) | `SmallVec<[ActiveEffect; 4]>` for cache-friendly inline status-effects vec | Not required for #11; defer to perf tuning round. `Vec<ActiveEffect>` is sufficient at <10 effects per character. |
| [moonshine-save](https://crates.io/crates/moonshine-save) | (deferred to #23) | Selective ECS save | Feature #23 — the components shipped here MUST already be `Serialize`/`Deserialize` so #23 doesn't need a retrofit |
| [bevy_egui](https://crates.io/crates/bevy_egui) | =0.39.1 | Character-sheet UI | Feature #19 / #25 — not needed for #11 |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|-----------|-----------|----------|
| `Vec<ActiveEffect>` for `StatusEffects` | `SmallVec<[ActiveEffect; 4]>` (transitively available) | Saves heap allocation when `<= 4` effects; requires a one-line dep declaration to use the type directly (or use it through `bevy::utils::synccell` re-export which moved in 0.18). YAGNI for v1; a 4-character party with 1-2 statuses each = 4-8 total effects, well under the typical Vec inline reserve. Revisit if combat profiling shows allocation churn. |
| `Handle<ItemAsset>` for Equipment slots | `Entity` references (master research §Pattern 3) | The §Pattern 3 example uses `Entity`, but the example is from a generic Bevy reference. For Druum's save/load (#23) trajectory, `Entity` requires implementing `MapEntities` on `Equipment` to remap on load — non-trivial. `Handle<ItemAsset>` serializes as `AssetPath` (bevy_asset 0.18 path) cleanly. Tradeoff: `Handle<ItemAsset>` cannot represent per-instance state (enchantment, durability, custom name); for those, Feature #12 wraps the handle in an `ItemInstance` component on a child entity. Recommended: `Handle<ItemAsset>` now; revisit when Feature #12 requires per-instance fields. (See Decision 3.) |
| Stat tuple `(STR, INT, PIE, VIT, AGI, LUK)` | Single `BaseStats { str, int, pie, vit, agi, luk }` struct | Master research line 533 uses the struct shape with named fields; named fields kill the "which index is INT" footgun. Recommended: struct. |
| `derive_stats(...) -> DerivedStats` taking owned values | `derive_stats(&BaseStats, &Equipment, &StatusEffects, level: u32)` taking refs | Refs avoid copies in the hot path (recompute on equip change). Recommended: refs. The function is pure either way. |

**Installation:** No new dependencies. The relevant lines in `Cargo.toml` are already there:
```toml
serde = { version = "1", features = ["derive"] }
ron   = "0.12"
bevy_common_assets = { version = "=0.16.0", features = ["ron"] }
bevy_asset_loader  = "=0.26.0"
```

---

## Architecture Options

Three fundamentally different ways to structure the character data layer. Pick one before writing code.

| Option | Description | Pros | Cons | Best When |
|--------|-------------|------|------|-----------|
| **A: Components-as-data with `Equity = Handle<ItemAsset>`** (RECOMMENDED) | Each character is one entity carrying 9-10 components. Equipment slots hold `Option<Handle<ItemAsset>>`. `derive_stats` is pure: `fn derive_stats(&BaseStats, &Equipment, &Assets<ItemAsset>, &StatusEffects, level: u32) -> DerivedStats`. Items are static `Asset` definitions; per-instance state (enchantment) lives on a child entity carrying `ItemInstance(Handle<ItemAsset>)`. | Save/load is automatic — `Handle<ItemAsset>` serde-serializes as a path, no `MapEntities` dance. Every component derives `Serialize`/`Deserialize` cleanly. Aligns with master research §Pattern 3 in spirit (components as data) while sidestepping the dangling-`Entity` trap. Pure function trivially unit-testable. | Per-instance item state requires a separate `ItemInstance` entity model in #12; a "+3 Sword of Sharpness" cannot be expressed by `Handle<ItemAsset>` alone — must layer on top. | Save/load is a near-term concern (Feature #23), the genre's per-instance state is shallow (most items are not unique), and team prefers automatic serde over manual `MapEntities`. |
| **B: Components-as-data with `Equipment = Entity`** (master research §Pattern 3 verbatim) | Each character is one entity. Equipment slots hold `Option<Entity>` pointing to item entities. `derive_stats` reads stats from those entities via `Query<&ItemStats>`. | Maximum ECS flexibility — items can have arbitrary components and per-instance state (enchantment, durability) directly. Matches the master research code example line 577-587 verbatim. | Save/load (Feature #23) requires implementing `MapEntities` on `Equipment` to remap entity IDs on load — see `bevy_ecs-0.18.1/src/entity/map_entities.rs:22-55` for the trait. The official Bevy docs explicitly call out this risk: `bevy_ecs-0.18.1/src/entity/mod.rs:45-49`: "Note that this means an Entity id may refer to an entity that has since been despawned!" Dangling references on item-despawn produce silent bugs. `derive_stats` is no longer pure — it needs world access. | Items are highly individuated (every dropped item is unique, like Diablo), per-instance state is mandatory from day one, and the team is willing to invest in `MapEntities` plumbing for #23. |
| **C: Single fat `Character` struct as one Component** | One `Character` component per entity holding all the data: name, race, class, stats, equipment refs, status effects, level. | Simplest spawn pattern: `commands.spawn(Character { ... })`. Easy to clone, easy to serialize. | Defeats Bevy's archetype-based query optimizations — every query must read the full struct. Change detection fires on any field change (stats recompute on inventory toggle). Master research §Anti-Patterns explicitly cautions against this: "Storing game state in resources instead of components: Party member stats, inventory, etc. should be components on entities, not fields in a giant `GameState` resource." (line 973 — same logic applies to a single fat component.) Breaks UI's ability to query just the stats it needs. | Never — for this genre. The cost shows up in #19 (UI) and #25 (polish) when the renderer needs `&BaseStats` without pulling `&StatusEffects`. |

**Recommended: Option A — Components-as-data with `Handle<ItemAsset>`**

Rationale: it preserves master research §Pattern 3's "every component is a value type" intent, while sidestepping the dangling-`Entity` and `MapEntities` retrofit risks documented in Bevy 0.18's own ECS source. Save/load (#23) gets a Handle that serializes as an asset-path string out of the box. The "no per-instance state" cost is real but landed in the right place: item-instance state is Feature #12's job, layered on top of Feature #11's class-and-base-stats foundation.

### Counterarguments

Why someone might NOT choose Option A:

- **"Master research §Pattern 3 explicitly uses `Entity`."** — Response: The master research code is illustrative, not normative. The same research, in §Pitfall 5, prioritizes save/load-friendly choices ("Design components as serializable from day one") and in §Anti-Patterns warns about tight-coupling rendering to game logic. `Handle<ItemAsset>` is more aligned with both pitfalls than `Entity` is. Decision 3 surfaces this for explicit user approval.

- **"`Handle<ItemAsset>` cannot represent enchantment / durability."** — Response: True for the handle alone. The pattern is to use `Handle<ItemAsset>` for the *base item definition* and a separate `ItemInstance(Handle<ItemAsset>, EnchantmentLevel, Durability)` component for per-instance state. Equipment then references either the static asset or an `Entity` carrying `ItemInstance` — a hybrid. This is Feature #12's territory; #11 only needs the equipped-item-base reference. Surfaced as a sub-question in Decision 3.

- **"`Entity` is more idiomatic for ECS."** — Response: Idiomatic-for-pure-ECS yes, idiomatic-for-Bevy-with-save/load no. The Bevy 0.18 ECS docs themselves point to `Relationship` trait (not raw `Entity` fields) when storing entity refs in components: `bevy_ecs-0.18.1/src/component/mod.rs:333-347`. For this feature, neither pattern is the relationship trait — `Equipment` is a one-to-many ownership pattern, not a relationship. `Handle` sidesteps the question entirely.

---

## Architecture Patterns

### Recommended Project Structure

Per Decision 4 (single-file vs multi-file split), the recommendation is **single file** matching the #9 precedent. The roadmap line 609 lists `src/plugins/party/{character.rs, inventory.rs, progression.rs}` — three files with `inventory` and `progression` mostly stubs. Project precedent (Feature #9, Feature #10) is to NOT pre-split files until the second producer of code lands.

```
src/
├── data/
│   └── classes.rs              # FROZEN-from-day-one — fleshed out from stub
├── plugins/
│   └── party/
│       ├── mod.rs              # PartyPlugin: messages, debug-party system, plugin impl
│       ├── character.rs        # 12 components + Bundle + derive_stats + tests
│       └── tests.rs            # (optional) Layer-2 App-driven tests if char.rs gets too long
```

If implementer reaches >800 LOC in `character.rs`, split out `tests.rs` first (Layer 2 tests with an App), then revisit `inventory.rs` / `progression.rs` only when their first system arrives in #12 / #14.

`assets/classes/core.classes.ron` already exists as a 3-line stub. Replace the body with the 3-class table per Decision 8.

### Pattern 1: Components-as-data (the canonical §Pattern 3 shape, hardened for Druum)

Every component is a **value type** — no methods that mutate other components, no `&mut` cross-component access. The only methods on these types are pure helpers (e.g., `BaseStats::ZERO`, `Experience::level_for_xp`).

```rust
// src/plugins/party/character.rs
//
// Source: master research §Pattern 3 (research/20260326-01-bevy-first-person-dungeon-crawler-rpg.md:498-635)
// adapted for Druum project conventions (#4 serde-from-start, #9/#10 single-file, Bevy 0.18 idioms)

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::data::ItemDb;  // Handle<ItemAsset> resolves through ItemDb in #12

// --- Identity --------------------------------------------------------------

/// Character display name. Newtype around String so queries can target it
/// distinctly from any other String component.
#[derive(Component, Serialize, Deserialize, Default, Debug, Clone, PartialEq, Eq, Hash)]
pub struct CharacterName(pub String);

/// Race enum. Per Decision 2, ship Human-only for #11; declare other variants
/// to lock the discriminant order for save-format stability.
#[derive(Component, Serialize, Deserialize, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Race {
    #[default]
    Human,
    // Reserved for #19 character creation; kept in the enum so save format is stable.
    Elf,
    Dwarf,
    Gnome,
    Hobbit,
}

/// Class enum. Per Decision 1, ship 3 classes; declare the other 5 as future
/// reserved variants OR omit them (sub-question in Decision 1).
#[derive(Component, Serialize, Deserialize, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Class {
    #[default]
    Fighter,
    Mage,
    Priest,
    // Decision 1 sub-question: include or omit?
    // Thief, Bishop, Samurai, Lord, Ninja,
}
```

### Pattern 2: BaseStats / DerivedStats split

`BaseStats` are character-intrinsic and slow-changing (level-up, race bonus). `DerivedStats` are recomputed on equipment-change or status-change. The split keeps recompute cheap.

```rust
// AD&D-derived 6-stat array. Wizardry uses the same 6 as the SRD.
// u16 covers values to 65535 — overkill for stats capped at ~99 (SNES Wizardry was 18+).
// u8 would be tighter but u16 leaves headroom for buff stacking without overflow.
#[derive(Component, Serialize, Deserialize, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BaseStats {
    pub strength: u16,      // STR — physical attack, carry capacity
    pub intelligence: u16,  // INT — arcane MP/spell power
    pub piety: u16,         // PIE — divine MP/spell power
    pub vitality: u16,      // VIT — HP, poison resistance
    pub agility: u16,       // AGI — speed, evasion, init
    pub luck: u16,          // LUK — crit, treasure roll
}

impl BaseStats {
    pub const ZERO: Self = Self {
        strength: 0, intelligence: 0, piety: 0, vitality: 0, agility: 0, luck: 0,
    };
}

// DerivedStats are CACHED — recomputed on (equipment-change | status-change | level-up).
// `current_*` fields track in-combat depletion; `max_*` are the recomputed caps.
#[derive(Component, Serialize, Deserialize, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct DerivedStats {
    pub max_hp: u32,
    pub current_hp: u32,
    pub max_mp: u32,
    pub current_mp: u32,
    pub attack: u32,
    pub defense: u32,
    pub magic_attack: u32,
    pub magic_defense: u32,
    pub speed: u32,
    pub accuracy: u32,
    pub evasion: u32,
}
```

### Pattern 3: `derive_stats` — the canonical pure function

Master research §Pitfall 5 + §Anti-Patterns: this function is the testable contract. Pure inputs → pure output. No `Mut<T>`, no entity lookups, no resource reads.

**Recommended signature:**

```rust
/// Recompute DerivedStats from BaseStats + equipment item-stats + active statuses + level.
///
/// **Pure function.** No global state. No I/O. No randomness. Same inputs → same output.
/// **Equipment effects stack additively** in v1 (Decision 8 allows revisit when balancing).
/// **Status effects modify the result post-equipment** — applied as a final pass so a
/// "AttackUp" buff multiplies the equipped weapon's contribution, not just base STR.
///
/// `equip_stats` is the equipment contribution flattened into a `Vec<ItemStatBlock>`
/// — the caller reads `Equipment` slots, looks each `Handle<ItemAsset>` up in
/// `Assets<ItemAsset>`, extracts the per-item stat block, and passes the slice. This
/// keeps `derive_stats` testable without asset access.
pub fn derive_stats(
    base: &BaseStats,
    equip_stats: &[ItemStatBlock],   // Sum of all equipped item stat contributions
    status: &StatusEffects,
    level: u32,
) -> DerivedStats {
    // 1. HP/MP from base VIT/PIE/INT scaled by level.
    let base_hp = (base.vitality as u32) * 10 + level * 4;
    let base_mp_arcane = (base.intelligence as u32) * 4 + level * 2;
    let base_mp_divine = (base.piety as u32) * 4 + level * 2;
    let max_mp = base_mp_arcane.max(base_mp_divine);  // Caster classes pick the larger pool

    // 2. Attack/defense from base STR + equipped weapon/armor.
    let mut attack = base.strength as u32;
    let mut defense = 0u32;
    let mut magic_attack = base.intelligence.max(base.piety) as u32;
    let mut magic_defense = 0u32;
    let mut accuracy = base.agility as u32;
    let mut evasion = base.agility as u32;
    let mut hp_bonus = 0u32;
    let mut mp_bonus = 0u32;

    // 3. ADDITIVE equipment stacking (Decision 8 — revisit if balance breaks).
    for s in equip_stats {
        attack += s.attack;
        defense += s.defense;
        magic_attack += s.magic_attack;
        magic_defense += s.magic_defense;
        accuracy = accuracy.saturating_add(s.accuracy);
        evasion = evasion.saturating_add(s.evasion);
        hp_bonus += s.hp_bonus;
        mp_bonus += s.mp_bonus;
    }

    let mut max_hp = base_hp + hp_bonus;
    let mut max_mp = max_mp + mp_bonus;

    // 4. Status-effect post-pass. Applied as percentage modifiers AFTER equipment
    // so buffs scale with gear (the genre expectation).
    for effect in &status.effects {
        match effect.effect_type {
            StatusEffectType::AttackUp => {
                attack = (attack as f32 * (1.0 + effect.magnitude)) as u32;
            }
            StatusEffectType::Poison => {
                // No stat change at derive time; tick happens in #15 combat.
            }
            StatusEffectType::Sleep | StatusEffectType::Paralysis | StatusEffectType::Stone => {
                // Disabling effects don't change derived stats; gating happens in #15.
            }
            StatusEffectType::Dead => {
                // Dead character contributes nothing — caller should skip them.
                max_hp = 0;
                max_mp = 0;
            }
        }
    }

    DerivedStats {
        max_hp,
        current_hp: max_hp,  // Caller chooses whether to clamp current_hp to old value
        max_mp,
        current_mp: max_mp,
        attack,
        defense,
        magic_attack,
        magic_defense,
        speed: base.agility as u32 + accuracy.saturating_sub(evasion),  // placeholder
        accuracy,
        evasion,
    }
}
```

**Why this signature:**
- `&BaseStats` not `BaseStats` — avoid copy in hot path.
- `&[ItemStatBlock]` not `&Equipment + &Assets<ItemAsset>` — keeps `derive_stats` pure (no asset access). The caller flattens.
- `&StatusEffects` — same reason.
- `level: u32` — already a primitive, by-value is fine.
- Returns `DerivedStats` by value — the caller writes it back to the entity.

**The reset semantics.** When `derive_stats` returns, the caller must decide what to do with `current_hp` / `current_mp`:
- On level-up: reset both to max (heal to full).
- On equipment-change: keep `current_*` at `min(old_current, new_max)`.
- On status-change: same as equipment-change.

The function returns `current_hp = max_hp` as a sane default; the caller overwrites the `current_*` fields if they need different semantics. This keeps the function pure.

### Pattern 4: PartyMemberBundle — and the Bevy 0.18 `#[require]` alternative

Master research §Pattern 3 line 622-634 specs a manual `Bundle` derive. Bevy 0.18 introduces `#[require(...)]` (ref: `bevy_ecs-0.18.1/src/component/mod.rs:99-281`) — putting required components on a marker auto-attaches them. Both approaches work for #11; the manual `Bundle` is more explicit about what gets spawned.

**Recommended: explicit `#[derive(Bundle)]`** (matches master research, more obvious in code review):

```rust
#[derive(Bundle, Default)]
pub struct PartyMemberBundle {
    pub marker: PartyMember,
    pub name: CharacterName,
    pub race: Race,
    pub class: Class,
    pub base_stats: BaseStats,
    pub derived_stats: DerivedStats,
    pub experience: Experience,
    pub party_row: PartyRow,
    pub party_slot: PartySlot,
    pub equipment: Equipment,
    pub status_effects: StatusEffects,
}

/// Zero-sized marker for query targeting: `Query<&CharacterName, With<PartyMember>>`.
/// Distinguishes party characters from NPCs (#18) and enemies (#15) which may share
/// CharacterName, BaseStats, etc.
#[derive(Component, Serialize, Deserialize, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PartyMember;
```

The `PartyMember` marker is critical — without it, every query for party characters has to hard-code something like `With<PartySlot>` which couples to a non-marker component. Same pattern as `PlayerParty` in `src/plugins/dungeon/mod.rs:86`, `DungeonGeometry` line 160, `Torch` line 174.

### Pattern 5: `PartySize` resource as a hard cap

Master research line 596 + roadmap line 628 specify "PartySize: Resource capping at 4-6". Decision 6 splits this into value (4 vs 6) and semantics (hard cap vs soft default).

**Recommended:**

```rust
/// Maximum simultaneously-active party members. Defaults to 4 (Wizardry-canonical
/// for non-extended parties; matches the debug-party hardcoded count). Hard cap:
/// `spawn_default_debug_party` and any future spawn must check this.
#[derive(Resource, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PartySize(pub usize);

impl Default for PartySize {
    fn default() -> Self {
        Self(4)
    }
}
```

Hard cap means: the spawn system reads `Res<PartySize>` and refuses to spawn the (n+1)th character. Soft default means: the resource is the *default* value, but UI / spawners may override per-spawn-call. Hard cap is recommended for v1 because it makes the debug-party predictable; soft default is correct once #19 character creation lets the player choose 1-4 members.

### Anti-Patterns to Avoid (Druum-specific to #11)

- **DO NOT mutate `DerivedStats` outside of `derive_stats` (or its callers).** Combat (#15) writes to `current_hp`/`current_mp` only; everything else is recomputed. A spell that "boosts attack" goes through `StatusEffects`, not direct `DerivedStats` mutation.

- **DO NOT take `&mut World` in `derive_stats`.** It MUST stay pure. If a future need wants a "consult the dungeon's anti_magic_zone" check, that goes in the caller (the system that schedules the recompute), not in the function.

- **DO NOT add fields to `BaseStats` without a discriminant-stable plan.** `BaseStats` will be in every save file; adding a 7th stat later requires `#[serde(default)]` on the new field or a migration. Prefer `#[serde(default)]` from day one.

- **DO NOT use `Entity` references in `Equipment` slots without committing to `MapEntities`.** Per Decision 3 — `Handle<ItemAsset>` sidesteps this. If team overrides to `Entity`, the plan MUST include a `MapEntities` impl for `Equipment` and a save/load round-trip test that exercises the remap.

- **DO NOT pre-create `inventory.rs` and `progression.rs` empty files.** Per Decision 4 — the #9 precedent is to NOT pre-split. Each file should land with its first system, not as a stub.

- **DO NOT bypass `cfg(feature = "dev")` for the debug party.** Per Decision 5 — the debug party must NOT ship in release builds. The roadmap line 629 says `cfg(debug_assertions)`; the project precedent (`cycle_game_state_on_f9` at `src/plugins/state/mod.rs:62`) is `#[cfg(feature = "dev")]`. Match the precedent.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| RON loading for `ClassTable` | Custom serde RON loader | `bevy_common_assets::RonAssetPlugin::<ClassTable>` (already wired) | Already done in #3; just flesh out the type |
| Asset-loading state integration | Custom `LoadingState` machinery | `bevy_asset_loader` (already wired) | Already done in #3; `class_table: Handle<ClassTable>` field already in `DungeonAssets` |
| Resource for spawn capping | Per-system party-size constant | `PartySize: Resource(usize)` | Genre expectation (Wizardry's slot system); also enables #19 character creation to mutate it |
| Save/load entity-ref remapping | Manual ID rewrite | `MapEntities` trait — but only IF Decision 3 picks `Entity` | If `Handle<ItemAsset>`, sidestepped entirely |
| Status-effect duration tick | Custom timer per effect | Combat-turn tick in #15 | #11 ships the data; #15 ticks it. Don't add a real-time `Time::delta`-driven decay |

---

## Common Pitfalls

### Pitfall 1: `cfg(debug_assertions)` vs `cfg(feature = "dev")` mismatch

**What goes wrong:** Roadmap line 629 says "gated on `cfg(debug_assertions)`". Project precedent says `#[cfg(feature = "dev")]`. The two are NOT equivalent: `debug_assertions` is on for `cargo build` (debug profile); `feature = "dev"` requires explicit `--features dev`. A debug-party gated on `debug_assertions` would auto-spawn for any `cargo run` user, including a release-mode build with `--profile release`-but-debug-asserts left on.

**Why it happens:** Roadmap was written assuming Cargo conventions; the project picked `feature = "dev"` in Feature #2 to allow `cargo run --release --features dev` for performance-sensitive debugging. The two have diverged.

**How to avoid:** Surface as Decision 5. Default to `feature = "dev"` per project precedent. Check `src/plugins/state/mod.rs:62` for the canonical pattern (`cycle_game_state_on_f9` is registered under `#[cfg(feature = "dev")]`).

### Pitfall 2: `assets/data/classes.ron` path is wrong

**What goes wrong:** Roadmap §11 says "Author `classes.ron` with stat growth ranges". The user-task spec says `assets/data/classes.ron`. The actual loader in `src/plugins/loading/mod.rs:37,100` says `assets/classes/core.classes.ron`. Following the roadmap path would require either editing the loader (touches Frozen-by-#3 code) or adding a second loader entry.

**Why it happens:** The roadmap was authored before #3 locked the path. #3's `core.classes.ron` is already loaded as `Handle<ClassTable>` and exists as a stub.

**How to avoid:** Use `assets/classes/core.classes.ron` (the existing path). Plan should call this out so the implementer doesn't move it. NO loader changes — `LoadingPlugin` is frozen post-#3.

### Pitfall 3: Spawning the debug party before assets load

**What goes wrong:** `OnEnter(GameState::Loading)` is the loading-screen entry. If `spawn_default_debug_party` reads `Assets<ClassTable>` at that moment, the asset isn't loaded yet — `floors.get(&handle)` returns `None`. Same trap as `spawn_party_and_camera` in `src/plugins/dungeon/mod.rs:340-347`.

**Why it happens:** `OnEnter(Loading)` runs before `bevy_asset_loader` has finished polling. The assets resolve later, triggering `OnExit(Loading) -> OnEnter(TitleScreen)`.

**How to avoid:** Two paths:
1. **Defer the spawn to `OnEnter(GameState::TitleScreen)` or `OnEnter(GameState::Dungeon)`** — assets are guaranteed loaded by then. (Roadmap line 596 says `OnEnter(Loading)` but this is questionable; surface as a Decision 5 sub-question.)
2. **Use the same asset-tolerant pattern** as `spawn_party_and_camera`: `Option<Res<DungeonAssets>>` + `Some(assets) = ... else { warn!(...); return; };`.

Recommended: **`OnEnter(GameState::Dungeon)` for the debug party**. The Loading screen has no need for character data; the moment it's needed is when the player enters a dungeon for testing combat/UI.

### Pitfall 4: `bevy::utils::HashMap` no longer exists in Bevy 0.18

**What goes wrong:** Importing `bevy::utils::HashMap` (which was the convention in Bevy 0.16 and earlier) produces a compile error in 0.18.

**Why it happens:** The re-export was removed during the `bevy_platform` reshuffle. Verified during Feature #10 (per researcher memory).

**How to avoid:** Use `std::collections::HashMap` directly in any class-table-lookup helpers. If `ClassTable` exposes a per-class lookup by name, use `HashMap<String, ClassDef>` from std.

### Pitfall 5: Status effect Vec ordering creates non-deterministic stat output

**What goes wrong:** `StatusEffects { effects: Vec<ActiveEffect> }` is iterated in insertion order in `derive_stats`. If two buffs are added in different orders ("AttackUp 50%" then "AttackUp 100%" vs reverse), the resulting attack value differs (additive of percentages of different bases vs multiplicative of original).

**Why it happens:** Floating-point arithmetic isn't associative when intermixed with integer truncation.

**How to avoid:** Pick ONE composition rule and document it. Recommended: status effects compose by **summing magnitudes within an effect type, then applying the sum once** (e.g. two AttackUp 50% effects = one AttackUp 100% effect). This is order-independent. Add a unit test: `derive_stats(..., status_a_then_b) == derive_stats(..., status_b_then_a)`.

### Pitfall 6: `current_hp` reset behavior on equip-change is genre-controversial

**What goes wrong:** When a character equips a +20HP amulet, does their `current_hp` jump by 20 (heal as a side-effect of equipping) or stay the same (only `max_hp` changes)? Wizardry: stays same. Etrian Odyssey: stays same. Modern JRPGs: varies.

**Why it happens:** The spec doesn't say.

**How to avoid:** Decision: `current_*` is preserved across recompute (clamped to new max). This is genre-canonical. Document inline in `derive_stats` and the calling system. Recommended caller pattern:
```rust
let new = derive_stats(...);
derived.max_hp = new.max_hp;
derived.current_hp = derived.current_hp.min(new.max_hp);  // clamp, don't reset
// (same for max_mp / current_mp)
```

### Pitfall 7: AssetEvent vs MessageReader mismatch

**What goes wrong:** If a future system wants to recompute stats when `Handle<ItemAsset>` is hot-reloaded (e.g., balance-tweaking via `--features dev`), it needs `AssetEvent<ItemAsset>`. In Bevy 0.18, `AssetEvent` is a `Message`, not an `Event` (per researcher memory: "feedback_bevy_0_18_event_message_split"). Use `MessageReader<AssetEvent<ItemAsset>>`, NOT `EventReader<AssetEvent<ItemAsset>>`.

**Why it happens:** Bevy 0.18 family rename — same trap as `MovedEvent` in #7 and `StateTransitionEvent` in #2.

**How to avoid:** Not a Feature #11 concern (#11 doesn't need hot-reload of stats), but flag for #12/#19. Plan: NO hot-reload system in #11; defer.

### Pitfall 8: `ItemDb` stub's empty struct breaks if treated as a HashMap key

**What goes wrong:** `ItemDb` at `src/data/items.rs` is `pub struct ItemDb { /* empty */ }`. Code that does `Equipment { weapon: Some(handle), ... }` where `handle: Handle<ItemAsset>` cannot work because `ItemAsset` doesn't exist yet — only `ItemDb` does.

**Why it happens:** The roadmap defers `ItemAsset` to Feature #12.

**How to avoid:** For Feature #11, `Equipment` slots hold `Option<Handle<ItemDb>>` as a placeholder type. (Or skip the type entirely with `Option<()>` and resolve the type in #12.) Alternatively: declare `ItemAsset` (empty stub) here and let #12 fill it. Recommended: declare `ItemAsset` as an empty stub in `src/data/items.rs` (alongside `ItemDb`) so `Handle<ItemAsset>` is a valid type for `Equipment`. This is the same pattern as `ClassTable` in #3.

---

## Security

### Known Vulnerabilities

| Library | CVE / Advisory | Severity | Status | Action |
|---------|---------------|----------|--------|--------|
| serde 1.x | None found | — | — | Continue using |
| ron 0.12 | None found | — | — | Continue using |
| bevy 0.18.1 | None found | — | — | Continue using |
| bevy_common_assets 0.16 | None found | — | — | Continue using |
| bevy_asset_loader 0.26 | None found | — | — | Continue using |

No known CVEs as of 2026-05-04 for any library used in #11. (Re-verified via researcher memory; same status as in master research §Security.)

### Architectural Security Risks

| Risk | Affected Architecture | How It Manifests | Secure Pattern | Anti-Pattern |
|------|----------------------|------------------|----------------|--------------|
| Malicious save files | `StatusEffects::Vec<ActiveEffect>` | A crafted save with `Vec` length 1B causes OOM during deserialize | RON parser caps recursion + length implicitly via stack; for #23 explicitly pass `ron::Options::limit_depth(...)` and bound vec length on load | Trust save file size from disk |
| Malicious `classes.ron` | `ClassTable` | A class def with `hp_per_level: u32::MAX` overflows in `derive_stats` | Use `saturating_add` / `saturating_mul` in `derive_stats`; add a clamp on `BaseStats` fields after deserialize | Direct addition with no overflow guard |
| Negative-magnitude status effects | `ActiveEffect::magnitude: f32` | `magnitude: -1.0` on AttackUp produces 0 attack; `magnitude: -100.0` underflows on `as u32` cast | Clamp magnitude to `[-1.0, 10.0]` in `derive_stats` or on `ActiveEffect` constructor | `unchecked` casting |
| Status-effect amplification loop | `derive_stats` recompute trigger | If recompute schedules itself on stat change, an infinite loop | Recompute is event-driven (one-shot) not change-detection-driven | Recompute on every Update |

### Trust Boundaries

- **`classes.ron` from disk:** assumed-developer-authored. If modding is supported in the future, schema-validate before loading.
- **Save files (#23 territory):** out of scope for #11, but the `Serialize`/`Deserialize` derives shipped here are the trust-boundary surface for #23. Do NOT add custom `Serialize` impls; let serde derive handle it so the attack surface is bounded.
- **No network input.** Single-player game.

---

## Performance

| Metric | Value / Range | Source | Notes |
|--------|--------------|-------|-------|
| `derive_stats` call cost | <1µs per call (pure function over ~10 fields) | Rust release-mode estimate | Recompute on equip-change is fast enough to not need throttling |
| 12 components per character × 4 characters | ~48 entity-component pairs | Bevy ECS overhead | Master research line 1121: "ECS handles hundreds of thousands; a DRPG needs <1000 entities typically" |
| `Vec<ActiveEffect>` heap allocation | One per character per active-status set | std heap | <10 effects per character; alloc churn negligible. SmallVec optimization is YAGNI for v1 |
| `Handle<ItemAsset>` lookup in `Assets<ItemAsset>` | O(1) via `AssetId` slotmap | Bevy asset internals | Caller flattens equipment into `&[ItemStatBlock]` once per recompute, not per-stat-read |
| `ClassTable` deserialize | <10ms for 3-class table | RON parser benchmarks | Loaded once at startup via `bevy_asset_loader`, not per-frame |

**Performance is NOT a concern for Feature #11.** The data layer is small, the function is pure, and the recompute frequency is low (equip-change, status-change, level-up — all human-input rates).

---

## Code Examples

Verified-against-Bevy-0.18 patterns for #11.

### Spawning a `PartyMemberBundle`

```rust
// Source: pattern from src/plugins/dungeon/mod.rs:356-396 (spawn_party_and_camera)
// adapted for character spawn

fn spawn_default_debug_party(mut commands: Commands, party_size: Res<PartySize>) {
    let count = party_size.0.min(4);  // hard cap defensive
    for slot in 0..count {
        let (name, class) = match slot {
            0 => ("Aldric", Class::Fighter),
            1 => ("Mira", Class::Mage),
            2 => ("Father Gren", Class::Priest),
            3 => ("Borin", Class::Fighter),
            _ => unreachable!(),
        };
        commands.spawn(PartyMemberBundle {
            marker: PartyMember,
            name: CharacterName(name.into()),
            race: Race::Human,
            class,
            base_stats: BaseStats { strength: 12, intelligence: 10, piety: 10,
                                    vitality: 12, agility: 10, luck: 10 },
            derived_stats: DerivedStats::default(),  // Recomputed on first Update tick
            experience: Experience { level: 1, current_xp: 0, xp_to_next_level: 100 },
            party_row: if slot < 2 { PartyRow::Front } else { PartyRow::Back },
            party_slot: PartySlot(slot),
            equipment: Equipment::default(),  // No starting gear in #11
            status_effects: StatusEffects::default(),
        });
    }
    info!("Spawned {} debug party members", count);
}
```

### `derive_stats` unit test (Layer 1, no App)

```rust
// Source: src/data/dungeon.rs:437-524 test pattern (pure stdlib, no Bevy App)

#[test]
fn derive_stats_returns_zero_for_zero_inputs() {
    let stats = derive_stats(&BaseStats::ZERO, &[], &StatusEffects::default(), 1);
    assert_eq!(stats.attack, 0);
    assert_eq!(stats.max_hp, 4); // Just the level bonus (level=1, +4 HP per level baseline)
    assert_eq!(stats.max_mp, 2); // Just the level bonus
}

#[test]
fn derive_stats_equipment_stacks_additively() {
    let base = BaseStats::ZERO;
    let sword = ItemStatBlock { attack: 10, ..Default::default() };
    let armor = ItemStatBlock { defense: 5, ..Default::default() };
    let stats = derive_stats(&base, &[sword, armor], &StatusEffects::default(), 1);
    assert_eq!(stats.attack, 10);
    assert_eq!(stats.defense, 5);
}

#[test]
fn derive_stats_attack_up_status_multiplies_total_attack() {
    let base = BaseStats { strength: 10, ..BaseStats::ZERO };
    let sword = ItemStatBlock { attack: 10, ..Default::default() };
    let status = StatusEffects {
        effects: vec![ActiveEffect {
            effect_type: StatusEffectType::AttackUp,
            remaining_turns: Some(3),
            magnitude: 0.5, // +50%
        }],
    };
    let stats = derive_stats(&base, &[sword], &status, 1);
    // base STR (10) + sword attack (10) = 20, × 1.5 = 30
    assert_eq!(stats.attack, 30);
}

#[test]
fn derive_stats_status_order_independent() {
    let base = BaseStats { strength: 10, ..BaseStats::ZERO };
    let a = ActiveEffect { effect_type: StatusEffectType::AttackUp, remaining_turns: Some(3), magnitude: 0.5 };
    let b = ActiveEffect { effect_type: StatusEffectType::AttackUp, remaining_turns: Some(3), magnitude: 1.0 };
    let s_ab = StatusEffects { effects: vec![a.clone(), b.clone()] };
    let s_ba = StatusEffects { effects: vec![b, a] };
    let r_ab = derive_stats(&base, &[], &s_ab, 1);
    let r_ba = derive_stats(&base, &[], &s_ba, 1);
    assert_eq!(r_ab.attack, r_ba.attack, "Status order must not affect derived stats");
}
```

### Layer 2 test: Bundle round-trips through ECS

```rust
// Source: src/plugins/dungeon/tests.rs:150-179 pattern (MinimalPlugins + App)

#[test]
fn party_member_bundle_spawns_with_all_components() {
    use bevy::state::app::StatesPlugin;

    let mut app = App::new();
    app.add_plugins((MinimalPlugins, StatesPlugin, StatePlugin, PartyPlugin));
    app.update();

    let entity = app.world_mut().spawn(PartyMemberBundle {
        marker: PartyMember,
        name: CharacterName("Test".into()),
        race: Race::Human,
        class: Class::Fighter,
        base_stats: BaseStats { strength: 15, ..BaseStats::ZERO },
        derived_stats: DerivedStats::default(),
        experience: Experience { level: 1, current_xp: 0, xp_to_next_level: 100 },
        party_row: PartyRow::Front,
        party_slot: PartySlot(0),
        equipment: Equipment::default(),
        status_effects: StatusEffects::default(),
    }).id();

    // Verify every component is present.
    let world = app.world();
    let e = world.entity(entity);
    assert!(e.get::<PartyMember>().is_some());
    assert_eq!(e.get::<CharacterName>().unwrap().0, "Test");
    assert_eq!(e.get::<Class>().unwrap(), &Class::Fighter);
    assert_eq!(e.get::<BaseStats>().unwrap().strength, 15);
}

#[test]
fn party_query_returns_only_party_members() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin, StatePlugin, PartyPlugin));
    app.update();

    // Spawn one party member and one non-party entity with overlapping components.
    app.world_mut().spawn((PartyMember, CharacterName("Hero".into())));
    app.world_mut().spawn(CharacterName("NPC".into()));  // No PartyMember marker

    let names: Vec<String> = app.world_mut()
        .query_filtered::<&CharacterName, With<PartyMember>>()
        .iter(app.world())
        .map(|n| n.0.clone())
        .collect();
    assert_eq!(names, vec!["Hero"]);
}
```

### Round-tripping `BaseStats` through RON (Feature #4 pattern)

```rust
// Source: src/data/dungeon.rs:438-455 round-trip pattern

#[test]
fn base_stats_round_trips_through_ron() {
    let original = BaseStats {
        strength: 15, intelligence: 12, piety: 10,
        vitality: 14, agility: 11, luck: 8,
    };
    let serialized = ron::ser::to_string_pretty(&original, ron::ser::PrettyConfig::default())
        .expect("serialize");
    let parsed: BaseStats = ron::de::from_str(&serialized).expect("deserialize");
    assert_eq!(original, parsed);
}
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|-------------|-----------------|--------------|--------|
| Bundle as the only spawn idiom | `#[require(Component)]` for auto-attach | Bevy 0.16+ | Bundles still work; #[require] is an alternative for marker-driven required components |
| `bevy::utils::HashMap` re-export | `std::collections::HashMap` directly | Bevy 0.18 | Removed re-export; use std (per #10 implementation) |
| Manual `EntityMapper` impl for save | `MapEntities` derive | Bevy 0.16+ | Optional path if Decision 3 picks `Entity` for Equipment |
| `Event` trait for everything | `Message` trait for engine-internal events | Bevy 0.18 family rename | `AssetEvent` and `StateTransitionEvent` are `Message`s; use `MessageReader` |
| `Camera3dBundle::default()` | `(Camera3d, Transform, ...)` component tuple | Bevy 0.16+ | Bundles for renderer types removed; use `#[require(...)]` shape |

**Deprecated patterns to avoid:**
- `bevy::utils::HashMap` — gone, use std.
- `EventReader<AssetEvent<T>>` — use `MessageReader<AssetEvent<T>>` in 0.18.
- Storing all party data in a `Resource` — use components per master research §Anti-Patterns.

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[cfg(test)] mod tests` + `cargo test` |
| Config file | None (Cargo.toml conventions) |
| Quick run command | `cargo test plugins::party::character` |
| Full suite (verifies all 7 commands per #10 precedent) | `cargo check && cargo check --features dev && cargo clippy --all-targets -- -D warnings && cargo clippy --all-targets --features dev -- -D warnings && cargo fmt --check && cargo test && cargo test --features dev` |

### Layer split (per researcher memory `feedback_bevy_input_test_layers.md`)

- **Layer 1 — pure functions (no App):** `derive_stats` tests, `BaseStats` RON round-trip, status-order-independence. Run with stdlib only. Sub-1ms each. Paste into `src/plugins/party/character.rs::tests`.
- **Layer 2 — App-driven (no `InputPlugin`):** `PartyMemberBundle` spawn, `Query<&CharacterName, With<PartyMember>>` filtering, `PartySize` resource gating. Pattern matches `src/plugins/dungeon/tests.rs:150-179`. Use `MinimalPlugins + StatesPlugin + StatePlugin + PartyPlugin`. NOT `InputPlugin` — #11 has no input handling.
- **Layer 3 — `init_resource::<ButtonInput<KeyCode>>` bridge:** required only when `--features dev` registers a debug-party-spawn keyhotkey (the spec doesn't require one for #11, so skip).

### Requirements → Test Map

| Requirement | Behavior | Test Type | Automated Command | File Exists? |
|-------------|----------|-----------|-------------------|--------------|
| `BaseStats` serde round-trip | Deserialize == Serialize input | Layer 1 | `cargo test plugins::party::character::tests::base_stats_round_trips` | ❌ needs creating |
| `derive_stats` returns zero for zero inputs | Identity case | Layer 1 | `cargo test plugins::party::character::tests::derive_stats_zero` | ❌ needs creating |
| `derive_stats` equipment additive stacking | Two items add their attack | Layer 1 | `cargo test plugins::party::character::tests::derive_stats_equipment_stacks` | ❌ needs creating |
| `derive_stats` AttackUp status modifies total | Buff multiplies post-equipment attack | Layer 1 | `cargo test plugins::party::character::tests::derive_stats_attack_up` | ❌ needs creating |
| `derive_stats` status-order independence | Permuting status order yields same result | Layer 1 | `cargo test plugins::party::character::tests::derive_stats_order_indep` | ❌ needs creating |
| `derive_stats` Dead status zeros HP/MP | Dead character has max_hp = 0 | Layer 1 | `cargo test plugins::party::character::tests::derive_stats_dead_zeros_pools` | ❌ needs creating |
| `PartyMemberBundle` spawns with all 10 components | All components present after spawn | Layer 2 | `cargo test plugins::party::tests::bundle_spawns_with_all_components` | ❌ needs creating |
| `Query<With<PartyMember>>` filters NPCs | Only marked entities returned | Layer 2 | `cargo test plugins::party::tests::query_filters_only_party` | ❌ needs creating |
| `PartySize::default() == 4` | Default cap is 4 | Layer 1 | `cargo test plugins::party::tests::party_size_default_is_four` | ❌ needs creating |
| `spawn_default_debug_party` spawns exactly 4 (under `--features dev`) | Count == PartySize::default() | Layer 2 (`#[cfg(feature = "dev")]`) | `cargo test --features dev plugins::party::tests::debug_party_spawns_four` | ❌ needs creating |
| `spawn_default_debug_party` is NOT registered without `--features dev` | Bundle count == 0 in default build | Layer 2 (default features only) | `cargo test plugins::party::tests::debug_party_not_in_default` | ❌ needs creating |
| `ClassTable` RON round-trip | `core.classes.ron` deserializes; serialize matches | Layer 1 | `cargo test data::classes::tests::class_table_round_trips` | ❌ needs creating |
| `ClassTable` integration: load via `RonAssetPlugin` | App loads `core.classes.ron` and the asset is queryable | Integration (`tests/class_table_loads.rs`) | `cargo test --test class_table_loads` | ❌ needs creating (mirror `tests/dungeon_floor_loads.rs`) |

### Gaps (files to create before implementation)

- [ ] `src/plugins/party/character.rs` — 12 components + Bundle + `derive_stats` + Layer 1 tests
- [ ] `src/plugins/party/mod.rs` — replace empty stub with PartyPlugin registering `PartySize` resource and `spawn_default_debug_party` system
- [ ] `src/plugins/party/tests.rs` (optional, only if char.rs > 800 LOC) — Layer 2 App-driven tests
- [ ] `src/data/classes.rs` — replace stub with real `ClassTable` schema (3 classes per Decision 1, 8) + Layer 1 round-trip test
- [ ] `src/data/items.rs` — add empty `ItemAsset` stub alongside `ItemDb` so `Handle<ItemAsset>` resolves (per Pitfall 8)
- [ ] `assets/classes/core.classes.ron` — replace stub `()` with 3-class data (Fighter/Mage/Priest)
- [ ] `tests/class_table_loads.rs` — integration test mirroring `tests/dungeon_floor_loads.rs`

No edits to `src/main.rs`, `src/plugins/state/mod.rs`, `src/plugins/loading/mod.rs`, `src/plugins/dungeon/mod.rs`, `src/plugins/input/mod.rs`, or any other existing file outside `src/plugins/party/` and `src/data/{classes.rs, items.rs}`.

---

## The Eight Category B Decisions

Each decision below has options, recommendation, and explicit trade-offs. The planner should resolve each (or escalate to user) before writing the implementation plan.

### Decision 1: Class roster scope

**Question:** Ship 3 classes or all 8?

| Option | Pros | Cons |
|--------|------|------|
| **A: 3 classes (Fighter, Mage, Priest)** [RECOMMENDED] | Matches roadmap line 630 + master research §Pitfall 6. Validates the system without combinatorial balance work. Saves 2-3h of class authoring. | 5 classes deferred; #19 character creation will surface "only 3 options" UX. |
| B: 4 classes (add Thief) | Adds the canonical "rogue" archetype for combat variety. | One more class to balance; not in roadmap. |
| C: All 8 (per master research line 519) | Full canonical Wizardry roster. | Master research §Pitfall 6 explicitly cautions against this. Combinatorial balance explosion. Each later feature (combat, UI, items) multiplies in scope. |

**Sub-question:** For the 5 omitted classes, declare them as enum variants now (locks save format) or omit them entirely (smaller enum, but breaking change when added)?

- **Sub-A: Declare all 8 enum variants, only Fighter/Mage/Priest are used** [RECOMMENDED] — matches the Race pattern in Pattern 1 above; locks discriminant order so save files stay forward-compatible. The unused variants are a minor lint risk (some clippy rules flag unreachable arms in matches); document in the enum doc-comment.
- Sub-B: Omit unused variants — smaller enum but adding a variant later is a breaking change for save files (Bincode-style ordering matters for ron only if you use enum-variant-id; for ron's identifier-based variant encoding, adding variants is non-breaking).

**Recommendation:** A + Sub-A. Ship 3 classes, declare 8 enum variants.

### Decision 2: Race roster scope

**Question:** Human only, or declare all 5 races as enum variants?

| Option | Pros | Cons |
|--------|------|------|
| **A: Declare all 5 races (Human/Elf/Dwarf/Gnome/Hobbit), use Human only** [RECOMMENDED] | Matches roadmap line 634 ("pick one race for now (Human)"). Locks the enum for save format. Allows #19 to add support without altering the type. | Five enum variants, four unused. |
| B: Declare only Human | Smaller enum. | Adding races later requires a `Race` change — touches all save files (mitigated by ron's identifier encoding, but still annotation churn in tests). |

**Recommendation:** A. Declare 5 variants; default and use Human.

### Decision 3: Equipment storage type

**Question:** `Equipment` slots hold `Option<Entity>` or `Option<Handle<ItemAsset>>`?

| Option | Pros | Cons |
|--------|------|------|
| A: `Option<Entity>` (master research §Pattern 3 verbatim) | Maximum ECS flexibility — items are entities with arbitrary components. Per-instance state (enchantment) is native. | Save/load (#23) requires implementing `MapEntities` on `Equipment` (`bevy_ecs-0.18.1/src/entity/map_entities.rs:22-55`). Dangling-Entity risk when item entity is despawned (`bevy_ecs-0.18.1/src/entity/mod.rs:45-49`: "Note that this means an Entity id may refer to an entity that has since been despawned!"). `derive_stats` becomes non-pure (needs `Query<&ItemStats>`). |
| **B: `Option<Handle<ItemAsset>>`** [RECOMMENDED] | `Handle<T>` serializes cleanly as an asset path. No `MapEntities` dance for save/load. `derive_stats` stays pure (caller flattens via `Assets<ItemAsset>`). Aligns with §Pitfall 5 ("design components as serializable from day one"). | Cannot represent per-instance state (enchantment, durability, custom name). Per-instance state lands as a separate `ItemInstance` entity in #12 — a known additional pattern. |

**Sub-question:** If Option B, where does per-instance state live?

- **Sub-B.1: `Equipment` slots stay `Handle<ItemAsset>`; per-instance state is a child entity carrying `ItemInstance(Handle<ItemAsset>, EnchantmentLevel, ...)` queried by `ChildOf`-relation** [RECOMMENDED] — uses Bevy's `Relationship` pattern (`bevy_ecs-0.18.1/src/relationship/mod.rs:27-77`).
- Sub-B.2: `Equipment` slots become `Option<EquipmentRef>` enum: `EquipmentRef::Static(Handle<ItemAsset>) | Dynamic(Entity)` — flexible, but doubles the pattern matching surface in `derive_stats`.

**Recommendation:** B + Sub-B.1. Plan note: declare `ItemAsset` as an empty stub in `src/data/items.rs` so `Handle<ItemAsset>` resolves (per Pitfall 8).

**Plan-of-record cost if Option A is chosen instead:** add ~50 LOC for `MapEntities` impl on `Equipment` + a save/load round-trip integration test that exercises despawn + respawn entity remapping.

### Decision 4: File layout

**Question:** Multi-file (`character.rs`, `inventory.rs`, `progression.rs`) per roadmap, OR single `mod.rs` per #9 precedent?

| Option | Pros | Cons |
|--------|------|------|
| A: Multi-file split | Matches roadmap line 609. Clear future-home for #12 inventory and #14 progression code. | `inventory.rs` and `progression.rs` would land as empty stubs (no producer in #11). Project precedent (#9) is to NOT pre-split. |
| **B: Single `character.rs` + `mod.rs`** [RECOMMENDED] | Matches #9 / #10 precedent: pre-splitting was rejected as YAGNI. New file lands when its first system arrives. Easier to read end-to-end during code review. | Diverges from roadmap line 609 — needs explicit plan-note. |
| C: Everything in `mod.rs` (no `character.rs`) | Minimum file count. | `mod.rs` would top 800 LOC; harder to navigate. The #9 precedent splits into `mod.rs` + `tests.rs` only. |

**Recommendation:** B. Plan note: roadmap line 609 deviation, follows #9/#10 precedent.

### Decision 5: Debug-party gate

**Question (a):** `#[cfg(debug_assertions)]` (per roadmap) or `#[cfg(feature = "dev")]` (per project precedent)?

| Option | Pros | Cons |
|--------|------|------|
| A: `cfg(debug_assertions)` (roadmap line 629) | Auto-on for `cargo run` (debug profile). | Diverges from project precedent (`src/plugins/state/mod.rs:62` uses `#[cfg(feature = "dev")]`). Auto-spawns in `cargo run --release` if debug-asserts left on — surprising. |
| **B: `cfg(feature = "dev")`** [RECOMMENDED] | Matches project precedent. Explicit opt-in via `--features dev`. Same gate as `cycle_game_state_on_f9`. | Roadmap line 629 deviation — needs plan-note. Requires `--features dev` to test the debug party. |

**Question (b):** Spawn on `OnEnter(GameState::Loading)` (per roadmap) or `OnEnter(GameState::Dungeon)`?

| Option | Pros | Cons |
|--------|------|------|
| A: `OnEnter(Loading)` (roadmap line 596) | Party exists from the very first frame. | At `OnEnter(Loading)`, `Assets<ClassTable>` is NOT loaded yet. Either skip class-data lookups (just use defaults) or use the asset-tolerant pattern from `spawn_party_and_camera`. |
| **B: `OnEnter(Dungeon)`** [RECOMMENDED] | Assets are guaranteed loaded. The debug party is needed only for combat/UI testing, which happens in Dungeon. Aligns with roadmap line 631 ("Smoke-test by querying"). | Diverges from roadmap line 596 — needs plan-note. Party doesn't exist in TitleScreen. |
| C: `OnEnter(TitleScreen)` | Party exists once assets load. | Party persists across F9 cycles, which may produce duplicates if the system runs again on re-entry. Mitigated by checking "if no party exists yet" gate. |

**Recommendation:** Question (a): B. Question (b): B. Combined gate: `#[cfg(feature = "dev")]` system on `OnEnter(GameState::Dungeon)` with an idempotence guard (`if Query<&PartyMember>.is_empty()`).

### Decision 6: `PartySize` value and semantics

**Question (a):** Default value 4 or 6?

| Option | Pros | Cons |
|--------|------|------|
| **A: 4** [RECOMMENDED] | Matches the debug-party hardcoded count (roadmap line 596 says "4 hardcoded characters"). Wizardry default for non-extended parties. Grimrock standard. Faster combat turn cycling. | Etrian Odyssey uses 5; Wizardry classic uses 6 — some genre tradition differs. |
| B: 6 | Wizardry classic. More tactical depth. | Combat UI in #19 must accommodate 6 panels. Combat balancing harder. Master research line 47 lists Wizardry as 6, Etrian as 5, Grimrock as 4. |

**Question (b):** Hard cap or soft default?

| Option | Pros | Cons |
|--------|------|------|
| **A: Hard cap** [RECOMMENDED for v1] | Spawn systems read `Res<PartySize>` and refuse to spawn the (n+1)th. Predictable. | #19 character creation must mutate the resource to allow 1-4 player choice. |
| B: Soft default | The resource is just a default; per-spawn callers can override. | Less predictable. Easy to spawn 5 by accident. |

**Recommendation:** Question (a): A (4). Question (b): A (hard cap). Plan note: change to soft default in #19 if/when needed.

### Decision 7: `StatusEffectType` initial variants

**Question:** Which 4-6 status types ship in #11?

The master research line 603-618 lists 12 (Poison/Paralysis/Sleep/Silence/Blind/Confused/Stone/Dead + AttackUp/DefenseUp/SpeedUp/Regen). #11 is the data layer; the actual application happens in #15 (combat). Picking a minimal-but-coherent set keeps `derive_stats` test surface bounded.

| Option | Includes | Pros | Cons |
|--------|----------|------|------|
| **A: 5 variants — Poison, Sleep, Paralysis, Stone, Dead** [RECOMMENDED] | Wizardry-canonical "negative" set. Stone and Dead are status-as-permadeath gates (genre signature). | No buffs; #15 will need to add at least one. |
| B: 5 variants — Poison, Sleep, Paralysis, AttackUp, Regen | One sample buff exercises `derive_stats`'s status-modifier branch. | Stone + Dead deferred — but those are #15's combat-state needs. |
| C: 6 variants — A + AttackUp | Both negative set and one buff. | Six match arms in `derive_stats`; slightly larger test surface. |

**Sub-question:** Should `Dead` be a `StatusEffectType` variant or a separate `Dead` marker component?

- **Sub-A: Variant** [RECOMMENDED] — matches master research line 612. `derive_stats` handles "max_hp = 0 if Dead" as a single branch. Combat (#15) checks `status.has(StatusEffectType::Dead)`.
- Sub-B: Marker component — cleaner conceptually (Dead is a state, not a buff). But adds a 13th component to track and another With<Dead> query everywhere.

**Recommendation:** A + Sub-A. Plan note: Stone and Dead are non-tickable (no `remaining_turns` decrement); the cure is a temple visit (#21) or specific spell.

### Decision 8: `classes.ron` schema shape

**Question:** Deterministic per-level growth or random-roll range? What fields per class?

The roadmap line 630 says "stat growth ranges". "Range" implies random rolls; deterministic is also a valid genre choice (Etrian Odyssey is mostly deterministic, classic Wizardry uses dice rolls).

| Option | Schema | Pros | Cons |
|--------|--------|------|------|
| A: Random-roll ranges (`hp_per_level: (min, max)`) | RNG required; less reproducible without seeding | Wizardry-canonical | RNG via `rand` crate is not in deps yet (master research line 101 lists `rand` as supporting; not in Druum Cargo.toml). Adds a dep delta. |
| **B: Deterministic per-level (`hp_per_level: u32`)** [RECOMMENDED for v1] | Easier to balance; reproducible without RNG; no dep delta. | Less "feel" of classic Wizardry but matches Etrian. Can swap to ranges in #14 (progression) when `rand` lands for encounter rolls anyway. |
| C: Skip entirely; ship empty `ClassTable` | Keeps the loader plumbing in place. | Defeats the purpose of having a real schema. The 3-class data is part of #11's deliverable per roadmap. |

**Recommended schema:**

```rust
// src/data/classes.rs (replaces stub)

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::plugins::party::character::{BaseStats, Class};

/// Per-class growth and starting parameters, loaded from
/// `assets/classes/core.classes.ron` via `RonAssetPlugin::<ClassTable>`.
///
/// FROZEN-from-day-one: change with the same care as `src/data/dungeon.rs`.
#[derive(Asset, Reflect, Serialize, Deserialize, Default, Debug, Clone, PartialEq)]
pub struct ClassTable {
    pub classes: Vec<ClassDef>,
}

impl ClassTable {
    /// Returns the class def for the given Class enum, or None if not authored.
    pub fn get(&self, class: Class) -> Option<&ClassDef> {
        self.classes.iter().find(|c| c.id == class)
    }
}

#[derive(Reflect, Serialize, Deserialize, Default, Debug, Clone, PartialEq)]
pub struct ClassDef {
    pub id: Class,                  // Discriminator
    pub display_name: String,
    pub starting_stats: BaseStats,  // Level 1 base stats
    pub growth_per_level: BaseStats,// Per-level deterministic growth
    pub hp_per_level: u32,          // Added to max_hp on level-up
    pub mp_per_level: u32,          // Added to max_mp on level-up
    pub xp_to_level_2: u64,         // Base XP requirement; later levels scale
    pub xp_curve_factor: f32,       // Multiplier per level
}
```

**Recommended `core.classes.ron`:**

```ron
(
    classes: [
        (
            id: Fighter,
            display_name: "Fighter",
            starting_stats: (strength: 14, intelligence: 8, piety: 8, vitality: 14, agility: 10, luck: 9),
            growth_per_level: (strength: 2, intelligence: 0, piety: 0, vitality: 2, agility: 1, luck: 0),
            hp_per_level: 8,
            mp_per_level: 0,
            xp_to_level_2: 100,
            xp_curve_factor: 1.5,
        ),
        (
            id: Mage,
            display_name: "Mage",
            starting_stats: (strength: 7, intelligence: 14, piety: 7, vitality: 8, agility: 10, luck: 10),
            growth_per_level: (strength: 0, intelligence: 2, piety: 0, vitality: 1, agility: 1, luck: 1),
            hp_per_level: 4,
            mp_per_level: 6,
            xp_to_level_2: 100,
            xp_curve_factor: 1.5,
        ),
        (
            id: Priest,
            display_name: "Priest",
            starting_stats: (strength: 9, intelligence: 8, piety: 14, vitality: 11, agility: 9, luck: 9),
            growth_per_level: (strength: 1, intelligence: 0, piety: 2, vitality: 1, agility: 0, luck: 1),
            hp_per_level: 6,
            mp_per_level: 5,
            xp_to_level_2: 100,
            xp_curve_factor: 1.5,
        ),
    ],
)
```

**Recommendation:** B (deterministic per-level). The schema above. The values are starting points for #14 balancing — the goal here is to validate the loader path, not to ship final balance numbers.

---

## Open Questions

1. **Should `derive_stats` clamp `current_hp` to `new_max_hp`, or leave it to the caller?**
   - What we know: Pattern 3 above recommends caller-clamps. The function returns `current_hp = max_hp` as a sane default.
   - What's unclear: Whether ergonomics suffer when 5+ callers all need the same clamp pattern.
   - Recommendation: Ship the caller-clamp pattern in #11; revisit if #14/#15 reveal a single canonical recompute system that handles all clamp logic.

2. **Should `Experience::xp_to_next_level` be cached or recomputed?**
   - What we know: A pure `xp_for_level(level: u32, curve: f32) -> u64` function would make this redundant.
   - What's unclear: Whether the cache is needed for UI display (avoiding recompute every frame).
   - Recommendation: Cache for v1 (matches master research line 561). Optimize later if dev-tools reveal a bug from cache drift.

3. **Should `BaseStats` and `DerivedStats` derive `Reflect`?**
   - What we know: All RON-loaded data types in Druum derive Reflect (per `src/data/dungeon.rs`). Components don't HAVE to.
   - What's unclear: Whether #19 / #25 UI needs to use `bevy_egui_inspector` or similar reflection-driven tools.
   - Recommendation: Add `Reflect` to be safe — it's free at runtime and unlocks future tooling. Consistent with the "components-as-data" principle.

4. **Is `ItemStatBlock` defined in #11 or #12?**
   - What we know: `derive_stats` needs an `&[ItemStatBlock]` slice. The block has fields like `attack`, `defense`, etc.
   - What's unclear: Whether #11 should declare `ItemStatBlock` (because `derive_stats` needs it) or defer to #12 (which owns items).
   - Recommendation: Declare a minimal `ItemStatBlock` in #11 alongside `ItemAsset` (in `src/data/items.rs`). #12 fleshes it out. The shape is needed NOW for `derive_stats`'s test suite. The empty `ItemAsset` stub can hold an `ItemStatBlock` field for the same dependency reason.

5. **Should `PartyMember` be a marker `()` component or carry the slot index?**
   - What we know: Pattern 4 above declares both `PartyMember` (marker) and `PartySlot(usize)` (slot). Master research line 572 declares `PartySlot(pub usize)` separately.
   - What's unclear: Whether to combine them (`PartyMember(usize)`) or keep separate.
   - Recommendation: Keep separate. The marker is invariant (every party character has one); the slot can change (formation reorder in #19). Conflating them couples two separately-mutating concerns.

---

## Sources

### Primary (HIGH confidence)

- [Bevy 0.18.1 local source — bevy_ecs Component required-components docs](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_ecs-0.18.1/src/component/mod.rs) — lines 99-281 verify `#[require(...)]` syntax and constructor variants
- [Bevy 0.18.1 local source — bevy_ecs Bundle derive](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_ecs-0.18.1/src/bundle/mod.rs) — lines 21-72 verify `#[derive(Bundle)]` syntax and `#[bundle(ignore)]` attribute
- [Bevy 0.18.1 local source — Entity lifecycle docs](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_ecs-0.18.1/src/entity/mod.rs) — lines 19-58 confirm "Entity id may refer to an entity that has since been despawned" (the dangling-Entity risk for Decision 3)
- [Bevy 0.18.1 local source — MapEntities trait](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_ecs-0.18.1/src/entity/map_entities.rs) — lines 22-55 confirm the save/load entity-remapping path required if Decision 3 picks `Entity`
- [Bevy 0.18.1 local source — Relationship trait](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_ecs-0.18.1/src/relationship/mod.rs) — lines 27-77 confirm the official Bevy answer for "Component holding Entity references"
- [Project source — Druum's existing party stub](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/party/mod.rs) — confirms PartyPlugin already exists, registered in main.rs:32
- [Project source — Druum's frozen ClassTable stub](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/data/classes.rs) — confirms ClassTable is wired through LoadingPlugin, asset path is `assets/classes/core.classes.ron`
- [Project source — LoadingPlugin RonAssetPlugin registrations](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/loading/mod.rs) — line 100 confirms `RonAssetPlugin::<ClassTable>::new(&["classes.ron"])` already in place
- [Project source — Feature #4 dungeon data file (frozen-from-day-one precedent)](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/data/dungeon.rs) — pattern for the new src/data/classes.rs to mirror
- [Project source — Feature #4 plan](file:///Users/nousunio/Repos/Learnings/claude-code/druum/project/plans/20260501-230000-feature-4-dungeon-grid-data-model.md) — serde-from-start + RonAssetPlugin precedent
- [Project source — Feature #10 implementation summary](file:///Users/nousunio/Repos/Learnings/claude-code/druum/project/implemented/20260504-150000-feature-10-auto-map-minimap.md) — most recent precedent for src/main.rs byte-unchanged ship pattern, std::collections::HashMap requirement
- [Master research §Pattern 3](file:///Users/nousunio/Repos/Learnings/claude-code/druum/research/20260326-01-bevy-first-person-dungeon-crawler-rpg.md) — lines 498-635 source the 12-component shape; §Pitfall 5 line 1054, §Pitfall 6 line 1066, §Anti-Patterns line 969-981
- [Roadmap §11](file:///Users/nousunio/Repos/Learnings/claude-code/druum/project/roadmaps/20260429-01-bevy-dungeon-crawler-roadmap.md) — lines 590-636

### Secondary (MEDIUM confidence)

- [Researcher memory — feedback_third_party_crate_step_a_b_c_pattern.md](file:///Users/nousunio/Repos/Learnings/claude-code/druum/.claude/agent-memory/researcher/feedback_third_party_crate_step_a_b_c_pattern.md) — confirmation that #11 has zero crate additions, so Step A/B/C gate is N/A
- [Researcher memory — feedback_bevy_input_test_layers.md](file:///Users/nousunio/Repos/Learnings/claude-code/druum/.claude/agent-memory/researcher/feedback_bevy_input_test_layers.md) — confirms Layer 1/2/3 test patterns. #11 uses Layer 1 (pure derive_stats) + Layer 2 (App-driven bundle spawn)
- [Researcher memory — reference_bevy_reflect_018_derive.md](file:///Users/nousunio/Repos/Learnings/claude-code/druum/.claude/agent-memory/researcher/reference_bevy_reflect_018_derive.md) — confirms Reflect derives auto-derive on the component shapes #11 uses
- [Researcher memory — feedback_bevy_0_18_event_message_split.md](file:///Users/nousunio/Repos/Learnings/claude-code/druum/.claude/agent-memory/researcher/feedback_bevy_0_18_event_message_split.md) — flagged for Pitfall 7 (AssetEvent is Message in 0.18); not used by #11 itself

### Tertiary (LOW confidence — flagged for validation)

- AD&D 6-stat convention (STR/INT/PIE/VIT/AGI/LUK) — sourced from training data on Wizardry SNES manuals and SRD; no live verification in this session. The exact stat names ("piety" vs "wisdom", "vitality" vs "constitution") are conventional, not normative. Plan can rename without breaking anything if the user prefers different terms.
- Wizardry party-size convention (6 default, 4-6 typical for non-extended) — master research line 47 cites this; not independently re-verified in this session.

---

## Metadata

**Confidence breakdown:**

- Standard stack: HIGH — every dep is already in deps; verified against `Cargo.toml`. Zero deltas required.
- Architecture options A/B/C: HIGH — Option A vs B trade-off is grounded in Bevy 0.18 source-code-verified API behavior (`MapEntities` trait, Entity lifecycle docs).
- Component set + `derive_stats` shape: HIGH for the Bevy/Rust integration; MEDIUM for the specific stat names and growth values (genre convention, not source-verified).
- Pitfalls 1-8: HIGH — each is grounded in either Druum-precedent (Pitfalls 1, 2, 4, 5, 7) or master research (Pitfalls 3, 6, 8).
- 8 Decisions: HIGH on the recommendation rationale; the planner should still surface to the user because each is a Category B preference call.
- Tests + validation architecture: HIGH — patterns are direct copies of #4 (Layer 1) and #10 (Layer 2); no new test infrastructure needed.

**Research date:** 2026-05-04

**Dep delta:** 0. `Cargo.toml` is byte-unchanged. `Cargo.lock` is byte-unchanged. The cleanest-ship signal applies.

**Files touched estimate (per Decision 4 = single-file):**
- `src/plugins/party/mod.rs` — modify (replace empty stub with PartyPlugin)
- `src/plugins/party/character.rs` — NEW (~500-700 LOC)
- `src/data/classes.rs` — modify (replace stub with real schema, freeze from day one)
- `src/data/items.rs` — modify (add `ItemAsset` and `ItemStatBlock` stubs alongside existing `ItemDb`)
- `assets/classes/core.classes.ron` — modify (replace `()` stub with 3-class data)
- `tests/class_table_loads.rs` — NEW (integration test, ~50 LOC)
- `src/data/mod.rs` — modify (re-export `ItemAsset`, `ItemStatBlock`, `ClassDef`)

**Files NOT touched:** `src/main.rs`, `src/plugins/state/mod.rs`, `src/plugins/loading/mod.rs`, `src/plugins/dungeon/mod.rs`, `src/plugins/input/mod.rs`, `src/plugins/audio/mod.rs`, `src/plugins/combat/mod.rs`, `src/plugins/ui/mod.rs`, `src/plugins/save/mod.rs`, `src/plugins/town/mod.rs`, `src/data/dungeon.rs`, `src/data/spells.rs`, `src/data/enemies.rs`.
