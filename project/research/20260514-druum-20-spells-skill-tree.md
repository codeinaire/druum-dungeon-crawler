# Druum #20 — Spells & Skill Trees — Research

**Researched:** 2026-05-14
**Domain:** Druum (Bevy 0.18.1 Rust DRPG) — combat ability layer + per-class progression unlock graph
**Confidence:** HIGH on existing-tree integration points; MEDIUM on skill-tree shape and balance numbers; LOW on a few scope ambiguities surfaced as open questions
**Researcher tool note:** I could not directly invoke `gh issue view 20` from this environment. Issue #20 scope below is derived from the roadmap section `project/roadmaps/20260429-01-bevy-dungeon-crawler-roadmap.md:1079-1127` (the same source the prior 19 features used as their north star). The planner should run `gh issue view 20` themselves and reconcile any deltas against this doc's Open Questions section before writing the plan.

---

## Summary

Druum is unusually well-prepared for #20: nearly every integration seam the feature needs is already authored and waiting. `CombatActionKind::CastSpell { spell_id }` is shipped at `src/plugins/combat/actions.rs:28`, the spell submenu (`MenuFrame::SpellMenu`) lives at `src/plugins/combat/turn_manager.rs:101`, the `Silence` gate (`is_silenced` → forced pop-to-Main) is wired at `src/plugins/combat/ui_combat.rs:457-466`, MP fields (`max_mp` / `current_mp`) live on `DerivedStats` at `src/plugins/party/character.rs:135-140`, MP refill on Inn rest is at `src/plugins/town/inn.rs:153`, level-up MP reset is at `src/plugins/party/progression.rs:460`, and even the asset loader (`RonAssetPlugin::<SpellTable>::new(&["spells.ron"])`) is registered at `src/plugins/loading/mod.rs:135` pointing at the stub `assets/spells/core.spells.ron`. The `SpellTable` type at `src/data/spells.rs` is an empty-body placeholder explicitly noted as "Feature #20 fills in real spell definitions." Every party member already has `Experience { level, current_xp, xp_to_next_level }` accumulating from combat victories (#19).

What is **missing** and must be built: (1) the `SpellAsset` schema (id, mp_cost, target type, level, school, `SpellEffect`), (2) per-character `KnownSpells` component and a `SkillTree` data shape, (3) `CastSpell` resolver in `execute_combat_actions` to replace the "not yet implemented" stub, (4) a spell submenu UI replacing the stub at `ui_combat.rs:457-473`, (5) skill-point allocation hook on level-up, and (6) optionally a Guild "view tree / spend point" screen. The existing primitives (`ApplyStatusEvent`, `damage_calc`, `EquipmentChangedEvent` dual-use for re-derive, `TargetSelection` with re-target fallback, `apply_status_handler` as sole `StatusEffects` mutator) cover every effect category #20 lists in the roadmap.

The **structurally novel piece** is the skill tree. The roadmap describes a per-class graph of nodes, each unlocking a spell or passive ability. The lightest design that delivers "tree feel" without becoming a separate subsystem is a **flat per-class node list with `Vec<NodeId>` prerequisites**, where each node grants either a `learn_spell: SpellId` or a passive (a `BaseStats` delta, a `StatusEffectType` resistance flag, or an authored `SpellEffect`). Skill points (1 per level-up, gated by `level < level_cap()`) are accumulated on `Experience` (additive extension; matches the #19 `ClassDef` additive pattern). Combat queries known-spells, not the tree directly — the tree is a creation/Town concept; combat only ever sees the resolved `KnownSpells` list. This keeps the tree data structure entirely out of the combat hot path.

**Primary recommendation:** Land #20 in three phases. Phase 1 = `SpellAsset` schema + RON authoring + `CastSpell` resolver (replaces the existing stub, makes the existing menu functional). Phase 2 = per-character `KnownSpells` + spell submenu UI (with MP-gating, Silence-gating already wired). Phase 3 = `SkillTree` data shape + `SkillPoints` on level-up + Guild "view tree" panel. Phases 1 and 2 are independently shippable and unlock a playable combat loop; Phase 3 layers progression UX on top. Δ Cargo.toml = 0 (no new crates required). All new RON files must use the double-dot extension convention.

---

## Standard Stack

### Core (already present in tree — verify don't replace)

| Library | Version | Purpose | License | Maintained? | Why Standard |
| --- | --- | --- | --- | --- | --- |
| bevy | =0.18.1 | ECS / scheduler / asset pipeline / UI host | MIT/Apache-2.0 | ✅ | Pinned project-wide; see `Cargo.toml:10` |
| bevy_common_assets | =0.16.0 (ron feat) | `RonAssetPlugin<T>` typed RON loader | MIT/Apache-2.0 | ✅ | The pattern every Druum asset uses |
| bevy_asset_loader | =0.26.0 | `AssetCollection` blocking-load + state continuation | MIT/Apache-2.0 | ✅ | Adds spell handle to `DungeonAssets` |
| bevy_egui | =0.39.1 | Combat & Town overlay UI | MIT/Apache-2.0 | ✅ | Spell submenu painter pattern already used by `paint_combat_screen` |
| leafwing-input-manager | =0.20.0 | Action enums for Combat / Menu | ISC | ✅ | Spell menu nav uses existing `CombatAction` enum |
| serde | 1 (derive) | RON serde derives on `SpellAsset` | MIT/Apache-2.0 | ✅ | Used by every `data/*.rs` module |
| ron | 0.12 | RON parsing under unit tests | MIT/Apache-2.0 | ✅ | Same path as ItemDb/ClassTable |
| rand | 0.9 (small_rng) | Re-use `CombatRng` for variance/crits | MIT/Apache-2.0 | ✅ | Already wrapped; spells just borrow `&mut *rng.0` |

### Supporting — none new

This feature **adds zero crates.** Verified by walking through every code path in `roadmap §20` and confirming every primitive is already present:

- Damage formula → `damage_calc` (already pure, takes `weapon: Option<&ItemAsset>`; extend or branch by `SpellEffect`)
- Status apply → `ApplyStatusEvent` + `apply_status_handler` (sole `StatusEffects` mutator; `potency` clamped to `[0.0, 10.0]` at trust boundary)
- Heal → identical pattern to consumable potion at `turn_manager.rs:567-598` (read `derived.max_hp/4`, write `derived.current_hp.saturating_add(...).min(max_hp)`, call `check_dead_and_apply`)
- Revive → exception path identical to `temple.rs:285-330` (effects.retain != Dead → `current_hp = 1` → fire `EquipmentChangedEvent { slot: EquipSlot::None }`)
- Buff → `ApplyStatusEvent { effect: AttackUp/DefenseUp/SpeedUp/Regen, potency: f32, duration: Some(N) }` — the dual-use `EquipmentChangedEvent` writer at `status_effects.rs:222-233` already triggers `recompute_derived_stats_on_equipment_change`

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
| --- | --- | --- |
| Single `SpellEffect` enum | Trait-object `Box<dyn SpellEffect>` | Enum keeps it `Reflect` + serde-derivable; trait-object would break RON round-trip. Reject. |
| `KnownSpells: Vec<String>` (SpellId) | `KnownSpells: Vec<Handle<SpellAsset>>` | Handles don't `Serialize` in Bevy 0.18 (same gotcha as `Equipment`, see `character.rs:204-211`). Strings are saveable today; #23 doesn't need a custom serde wrapper. Recommend strings. |
| Per-class skill tree files (`assets/skills/fighter.skills.ron`) | `ClassDef.skill_tree: SkillTree` (inlined) | Inlining keeps one file per class authoring location; matches `advancement_requirements` already in `ClassDef`. Recommend inlining over a parallel file family. |
| New `SpellId(u32)` newtype | `pub type SpellId = String` | Strings match `ItemAsset.id`/`EnemySpec.id` precedent; `CombatActionKind::CastSpell { spell_id: String }` ALREADY uses `String` at `actions.rs:28`. Recommend keeping `String`. |
| Adding `current_mp` reset to Inn (already done) | n/a | Already wired at `inn.rs:153`. ✅ |
| Adding MP-cost decrement system | Decrement inline in `CastSpell` arm of `execute_combat_actions` | Same pattern as `UseItem` removing inventory entity at `turn_manager.rs:574-589`. Reject standalone system. |

**Installation:** No `cargo add` required.

---

## Architecture Options

The roadmap describes #20 as **two coupled features**: (A) spell registry + resolver (a data table the combat loop reads), and (B) per-class skill trees (unlock graph that gates which spells a character knows). The recommendation crystallises around which architecture each half adopts.

### A. Spell registry + resolver — three layouts

| Option | Description | Pros | Cons | Best When |
| --- | --- | --- | --- | --- |
| **A1. Enum-of-effects** (recommended) | `SpellAsset { id, name, mp_cost, target, school, level, effect: SpellEffect }` with `SpellEffect = Damage { power, kind } \| Heal { amount } \| ApplyStatus { effect, potency, duration } \| Buff { ... } \| Revive { hp }`. Resolver in `execute_combat_actions` matches on `effect`. | Reflect-derivable; round-trips RON; one enum is the spec; aligns with existing `CombatActionKind` pattern. | Effect variants and the resolver must be edited together — adding a new effect category needs Rust changes, not just RON. | Most cases. The 15-25 starter spells in the roadmap fit ~5 effect variants. |
| **A2. Effect-as-component-list** | `SpellAsset.effects: Vec<SpellEffectAtom>` — multiple `SpellEffectAtom` per spell (e.g., damage + apply Poison). | Maximally composable; one spell can do damage AND status. | Doubles RON authoring complexity for marginal v1 benefit; harder to balance. | Phase 2+, once balance matters more than authoring speed. |
| **A3. Special-case Rust per spell** | `SpellAsset { id, name, mp_cost }` only — resolver has `match spell.id.as_str() { "fireball" => ..., "heal" => ... }`. | Maximum flexibility per spell. | Anti-pattern: explicitly called out in the roadmap (line 1089 "data-driven spells"). Hard-codes spell behaviour. Reject. | Never. |

**Recommended for half A: A1 (enum-of-effects).** The roadmap explicitly notes "data-driven spells — adding a spell is a RON entry plus optional special-case Rust" — A1 fits exactly. The escape hatch for special-case Rust is keeping `SpellEffect` extensible (add a variant when a one-off spell needs it).

### B. Skill tree — four layouts

| Option | Description | Pros | Cons | Best When |
| --- | --- | --- | --- | --- |
| **B1. Flat node list with prerequisite IDs** (recommended) | `SkillTree.nodes: Vec<SkillNode>` where `SkillNode { id, display_name, cost, prerequisites: Vec<NodeId>, grant: NodeGrant }`. `NodeGrant = LearnSpell(SpellId) \| StatBoost(BaseStats) \| Resist(StatusEffectType)`. Available-when: all prerequisites unlocked. | Linear data; RON-authorable; tree shape emerges from prerequisite IDs (a DAG); no coordinate system; lazy. | "Tree" rendering is the author's responsibility (row + column hints). | Lightweight progression. The roadmap's wording ("a per-class graph of nodes") matches this exactly. |
| **B2. Coordinate-grid talent matrix** (e.g. Diablo 2) | `SkillTree.nodes: Vec<SkillNode { id, row, col, grant, max_rank }>` with grid-based progression. | Visually authorable, "talent calculator" feel. | Adds row/col bookkeeping + multi-rank logic that doesn't pay off for ~10 nodes per class. | A massive (50+ node) tree per class. Defer to Phase 2+. |
| **B3. Pure level-gated learning** | No tree at all; `SpellAsset.required_level: u32`, `KnownSpells = all spells where class+level match`. Skill points unused. | Simplest. Zero player choice. | Roadmap explicitly says "spending skill points" — this contradicts scope. | Reject for #20 scope (matches the roadmap's "below the floor" alternative). |
| **B4. Trait-based (passive perks only)** | `KnownPerks: Vec<PerkId>` — no learnable spells, only stat / behaviour modifiers. | Tiny scope. | The roadmap couples spells + tree; #20 ships both. Reject as standalone. | A future "Talents" feature; not #20. |

**Recommended for half B: B1 (flat node list with prerequisite IDs).** This delivers the "tree feel" the roadmap describes with the smallest data structure. The DAG validation (no cycles, prerequisites reference existing nodes) is a 30-line pure-fn check identical to `can_create_class` at `progression.rs:307-354`.

**Combined recommendation:** A1 + B1.

### Counterarguments

Why someone might NOT choose the recommended option:

- **"Effect enum will balloon to 20 variants."** — **Response:** if it does, that's the right time to introduce A2 (effect atoms). The migration is mechanical and the variants you authored in A1 become atomic compositions in A2. Start small.
- **"Skill trees without rendering coordinates are unreadable."** — **Response:** Phase 3 painter can sort nodes by `level` (depth in DAG from any "root" node, computed once) and render rows by level. This emerges from prerequisite IDs without needing authoring of row/col. The Etrian Odyssey reference matches this pattern.
- **"`KnownSpells: Vec<String>` is unbounded — crafted save data could OOM."** — **Response:** clamp via `KNOWN_SPELLS_MAX = 64` in the serde trust boundary (same defensive pattern as `clamp_recruit_pool` at `data/town.rs:120`). The clamp is a single-line truncate; the structure stays simple.
- **"Why not branch the existing `damage_calc` for spell damage?"** — **Response:** spells have no weapon, different scaling stat (`magic_attack` vs `attack`), and don't trigger the front/back row block. Forking the function (or branching internally on action kind, which it already does — `if !matches!(action, CombatActionKind::Attack) { return ... }` at `damage.rs:70-77`) is cleaner than overloading the weapon parameter. Recommend a separate `spell_damage_calc` pure fn.
- **"Skill trees freeze design choices early."** — **Response:** trees are 100% RON-authored. Re-tuning a tree is editing one file; no Rust changes. The risk is the data shape, which B1 keeps minimal.

---

## Architecture Patterns

### Recommended Project Structure

New files (5 new modules + 4 new RON assets):

```
src/
├── data/
│   ├── spells.rs            # REPLACES stub: SpellAsset, SpellEffect, SpellDb, SpellSchool, SpellTarget
│   └── skills.rs            # NEW: SkillTree, SkillNode, NodeGrant, NodeId — DAG validation
├── plugins/
│   ├── combat/
│   │   └── spell_cast.rs    # NEW: CastSpell resolver helpers; reads SpellDb + writes ApplyStatusEvent
│   └── party/
│       └── skills.rs        # NEW: KnownSpells component, SkillPoints u32 on Experience extension,
│                            #      learn_spell pure fn, allocate_skill_point pure fn
assets/
├── spells/
│   └── core.spells.ron      # REPLACES stub: 15-25 spells across 3-4 schools
└── skills/
    ├── fighter.skills.ron   # NEW: per-class skill tree
    ├── mage.skills.ron      # NEW
    └── priest.skills.ron    # NEW
```

**Trade-offs of this structure:**

- `skills.rs` lives under `plugins/party/` (not `plugins/combat/`) because skill trees are a progression concern, not a combat concern. Combat consumes resolved `KnownSpells` and never touches the tree.
- `spell_cast.rs` is the symmetric helper for `damage.rs` — both are pure-fn modules called by `execute_combat_actions`. This mirrors the precedent of keeping resolver logic out of the action-dispatch system.
- One RON file per class avoids `clamp_skill_tree` over a huge `Vec`; each class is independently sized.

### Pattern 1: Spell asset shape (extends Reflect+serde precedent)

**What:** Each spell is a deserialised `SpellAsset` in `SpellDb.spells: Vec<SpellAsset>`. Schema follows `ItemAsset`/`ClassDef` exactly.

**When to use:** Always. Combat reads `Res<Assets<SpellDb>>` + walks the vec; same access pattern as `ItemDb`.

**Example (illustrative; not production-ready):**

```rust
// Source: pattern mirror of src/data/items.rs:84-118 and src/data/classes.rs:65-106
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use crate::plugins::party::character::StatusEffectType;

pub type SpellId = String; // matches ItemAsset.id, EnemySpec.id, RecruitDef.id

#[derive(Asset, Reflect, Serialize, Deserialize, Default, Debug, Clone, PartialEq)]
pub struct SpellDb {
    pub spells: Vec<SpellAsset>,
}

impl SpellDb {
    pub fn get(&self, id: &str) -> Option<&SpellAsset> {
        self.spells.iter().find(|s| s.id == id)
    }
}

#[derive(Reflect, Serialize, Deserialize, Default, Debug, Clone, PartialEq)]
pub struct SpellAsset {
    pub id: SpellId,
    pub display_name: String,
    pub mp_cost: u32,
    pub level: u32,            // school-level (1-7 Wizardry convention), NOT character level
    pub school: SpellSchool,
    pub target: SpellTarget,
    pub effect: SpellEffect,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub icon_path: String,
}

#[derive(Reflect, Serialize, Deserialize, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpellSchool {
    #[default]
    Mage,    // Mage class
    Priest,  // Priest class
    // Future: Bishop, Samurai, Lord, Ninja schools.
}

#[derive(Reflect, Serialize, Deserialize, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpellTarget {
    #[default]
    SingleEnemy,
    AllEnemies,
    SingleAlly,
    AllAllies,
    Self_,
    // No tiles / grid range in v1 (combat is not grid-based; the dungeon is).
}

#[derive(Reflect, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum SpellEffect {
    /// Magic damage; scales off attacker's magic_attack and defender's magic_defense.
    Damage { power: u32 },
    /// Healing; uses caster.magic_attack/4 or a flat amount.
    Heal { amount: u32 },
    /// Apply a StatusEffect via ApplyStatusEvent (the SOLE mutator path).
    ApplyStatus { effect: StatusEffectType, potency: f32, duration: Option<u32> },
    /// Buffs use ApplyStatus internally — declared as a separate variant for clarity in RON.
    Buff { effect: StatusEffectType, potency: f32, duration: u32 },
    /// Revive a Dead party member; exception path (effects.retain → current_hp = 1).
    Revive { hp: u32 },
    /// Reserved for spells that need bespoke Rust (e.g. dispel, MP drain).
    Special { variant: String },
}

impl Default for SpellEffect {
    fn default() -> Self { Self::Damage { power: 0 } }
}
```

**Anti-pattern variant:** `SpellEffect::DamageWithCustomFormula(f32, f32, f32)` — three magic floats; future you will not remember which is which. Use named-field structs even for single-field variants when the field meaning is non-obvious.

### Pattern 2: `KnownSpells` component (mirrors `StatusEffects`)

**What:** Per-character component holding the spell IDs the character has learned.

**When to use:** Always on `PartyMember`. Enemies do not learn spells; enemy spell behaviour (if any) is hard-authored on `EnemyAi`.

**Example:**

```rust
// Source: pattern mirror of src/plugins/party/character.rs:300-310 (StatusEffects)
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Component, Reflect, Serialize, Deserialize, Default, Debug, Clone, PartialEq, Eq)]
pub struct KnownSpells {
    pub spells: Vec<SpellId>,
}

impl KnownSpells {
    pub fn knows(&self, id: &str) -> bool {
        self.spells.iter().any(|s| s == id)
    }
    pub fn learn(&mut self, id: SpellId) {
        if !self.knows(&id) {
            self.spells.push(id);
        }
    }
}
```

Add `pub known_spells: KnownSpells` to `PartyMemberBundle` (additive — same pattern as the #11 → #19 expansion of the bundle).

### Pattern 3: Skill tree shape — flat list with prerequisite IDs

**What:** A DAG of nodes, where each node references its prerequisite NodeIds.

**When to use:** Always for per-class skill data. Loaded once via `RonAssetPlugin<SkillTree>`.

**Example:**

```rust
// Source: pattern mirror of src/data/encounters.rs:EnemyGroup (ID-by-string graph)
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use crate::data::spells::SpellId;
use crate::plugins::party::character::{BaseStats, StatusEffectType};

pub type NodeId = String;

#[derive(Asset, Reflect, Serialize, Deserialize, Default, Debug, Clone, PartialEq)]
pub struct SkillTree {
    pub class_id: String,                  // matches Class display_name
    pub nodes: Vec<SkillNode>,
}

#[derive(Reflect, Serialize, Deserialize, Default, Debug, Clone, PartialEq)]
pub struct SkillNode {
    pub id: NodeId,                        // unique within this tree
    pub display_name: String,
    pub cost: u32,                         // skill points to unlock (typically 1)
    #[serde(default)]
    pub prerequisites: Vec<NodeId>,        // empty = root node
    pub grant: NodeGrant,
    #[serde(default)]
    pub description: String,
}

#[derive(Reflect, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum NodeGrant {
    /// Add a spell to KnownSpells when unlocked.
    LearnSpell(SpellId),
    /// Permanent stat bonus — applied via the existing EquipmentChangedEvent dual-use trigger.
    StatBoost(BaseStats),
    /// Reduces probability or magnitude of an inflicted status (resolver work in Phase 3+).
    Resist(StatusEffectType),
}

impl Default for NodeGrant {
    fn default() -> Self { Self::LearnSpell(String::new()) }
}
```

`UnlockedNodes` component holds the resolved set (the per-character "what tree state is this character in").

```rust
#[derive(Component, Reflect, Serialize, Deserialize, Default, Debug, Clone, PartialEq, Eq)]
pub struct UnlockedNodes {
    pub nodes: Vec<NodeId>,
}
```

### Pattern 4: `SkillPoints` extension on `Experience` (additive — matches #19 precedent)

**What:** Track unspent skill points alongside XP.

```rust
// File: src/plugins/party/character.rs — extend Experience additively.
#[derive(
    Component, Reflect, Serialize, Deserialize, Default, Debug, Clone, Copy, PartialEq, Eq, Hash,
)]
pub struct Experience {
    pub level: u32,
    pub current_xp: u64,
    pub xp_to_next_level: u64,
    // NEW for #20 — defaults to 0 so existing save data still loads (defense-in-depth).
    #[serde(default)]
    pub unspent_skill_points: u32,
    #[serde(default)]
    pub total_skill_points_earned: u32, // monotonic; for UI display
}
```

Award 1 skill point per level-up in `apply_level_up_threshold_system` at `progression.rs:438-471` — same loop that increments level. Insert after `exp.level = exp.level.saturating_add(1)`:

```rust
exp.unspent_skill_points = exp.unspent_skill_points.saturating_add(1);
exp.total_skill_points_earned = exp.total_skill_points_earned.saturating_add(1);
```

### Pattern 5: `CastSpell` resolver — replaces existing stub

**What:** Replace the "not yet implemented" stub at `turn_manager.rs:531-541` with the real resolver.

**Example dispatch:**

```rust
// File: src/plugins/combat/turn_manager.rs — replace lines 531-541
CombatActionKind::CastSpell { spell_id } => {
    let actor_name = name_of(action.actor);

    // 1. Look up the spell.
    let spell = match spell_db.get(spell_id) {
        Some(s) => s,
        None => {
            combat_log.push(format!("{}'s spell fizzles.", actor_name), turn);
            continue;
        }
    };

    // 2. Pre-flight: Silenced? (already gated in UI but defense-in-depth here)
    let actor_status = chars.get(action.actor).map(|(_, s, _, _, _)| s).ok().cloned();
    if actor_status.as_ref().is_some_and(crate::plugins::combat::status_effects::is_silenced) {
        combat_log.push(format!("{} is silenced.", actor_name), turn);
        continue;
    }

    // 3. MP check.
    let current_mp = derived_mut.get(action.actor).map(|d| d.current_mp).unwrap_or(0);
    if current_mp < spell.mp_cost {
        combat_log.push(format!("{} lacks MP for {}.", actor_name, spell.display_name), turn);
        continue;
    }

    // 4. Deduct MP.
    if let Ok(mut actor_derived) = derived_mut.get_mut(action.actor) {
        actor_derived.current_mp = actor_derived.current_mp.saturating_sub(spell.mp_cost);
    }

    // 5. Resolve targets via existing resolve_target_with_fallback.
    // ... (same pattern as Attack arm above)

    // 6. Dispatch on SpellEffect.
    match &spell.effect {
        SpellEffect::Damage { power } => { /* spell_damage_calc + apply via derived_mut */ }
        SpellEffect::Heal { amount } => { /* current_hp.saturating_add(amount).min(max_hp) + check_dead_and_apply */ }
        SpellEffect::ApplyStatus { effect, potency, duration } => {
            apply_status.write(ApplyStatusEvent { target, effect: *effect, potency: *potency, duration: *duration });
        }
        SpellEffect::Buff { effect, potency, duration } => {
            apply_status.write(ApplyStatusEvent { target, effect: *effect, potency: *potency, duration: Some(*duration) });
        }
        SpellEffect::Revive { hp } => { /* exception path: effects.retain != Dead; current_hp = hp; fire EquipmentChangedEvent */ }
        SpellEffect::Special { variant } => { /* match on variant string */ }
    }

    combat_log.push(format!("{} casts {}.", actor_name, spell.display_name), turn);
}
```

The system signature gains one parameter: `spell_db: Res<Assets<SpellDb>>` plus the spell-handle resource (a `SpellHandle: Resource(Handle<SpellDb>)` populated in `LoadingPlugin` analogous to `TownAssets.class_table`).

### Pattern 6: Spell submenu UI — replaces existing stub

**What:** Replace the "not yet implemented" stub at `ui_combat.rs:457-473` with a working two-pane menu (spell list + target select).

```rust
// File: src/plugins/combat/ui_combat.rs — replace lines 457-473
MenuFrame::SpellMenu => {
    // Silence gate (already wired — keep)
    if is_silenced(status) {
        combat_log.push("You are silenced; cannot cast.".into(), input_state.current_turn);
        input_state.menu_stack = vec![MenuFrame::Main];
        return;
    }

    let known = match known_spells.get(actor_entity) {
        Ok(k) => k,
        Err(_) => { input_state.menu_stack = vec![MenuFrame::Main]; return; }
    };

    // Filter to spells the character has MP for AND knows.
    let castable: Vec<&SpellAsset> = known.spells.iter()
        .filter_map(|id| spell_db.get(id))
        .filter(|s| s.mp_cost <= derived.current_mp)
        .collect();

    // Up/Down: move spell_cursor; Confirm: push TargetSelect with the spell's target type;
    // Cancel: pop. (Mirror the Attack arm at line 360-372 exactly.)
}
```

A new `MenuFrame::SpellMenu` variant with embedded `selected_spell_id` is **NOT** required — `PlayerInputState.spell_cursor: usize` and a lookup into the filtered list is sufficient.

### Anti-Patterns to Avoid

- **Computing the tree's unlock state on every frame.** Cache `UnlockedNodes: Vec<NodeId>` per character. Tree-relative queries (is X reachable?) run only on point-spend.
- **Mutating `StatusEffects.effects` from the spell resolver.** This violates the "sole mutator" invariant at `status_effects.rs:163-235`. ALL spell-applied status MUST go through `ApplyStatusEvent`.
- **Spell `damage_calc` overloading.** Keep `spell_damage_calc` as a separate pure fn that takes `magic_attack`/`magic_defense` (analogous to `attack`/`defense`) — overloading `damage_calc` with a "is this a spell?" branch entangles two formulae.
- **Skipping the MP check during AI spell-cast.** Enemy AI (if you ever want enemies to cast) must check MP just like the player. The existing AI doesn't cast spells; if Phase 3 adds it, mirror the player flow.
- **Authoring `KnownSpells` directly in `core.recruit_pool.ron`.** The recruit's starting spells should come from a class-level "starting nodes" list in `<class>.skills.ron`, not duplicated per recruit. Single source of truth.
- **Skill tree without DAG validation.** A cyclic `prerequisites` list will silently lock all nodes. Add `validate_no_cycles(&SkillTree) -> Result<(), CycleError>` and call it during asset load (test via unit test, fail-fast on load).

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
| --- | --- | --- | --- |
| Spell damage formula | New variance/crit code | Borrow shape from `damage_calc` at `damage.rs:62-152` | Already saturating, already deterministic with seeded RNG, already crit-aware |
| Status apply | Direct `StatusEffects.effects.push` | `ApplyStatusEvent` + handler at `status_effects.rs:177-235` | Sole-mutator invariant; trust-boundary clamp; D5α re-derive |
| Heal apply | New heal system | Mirror the consumable path at `turn_manager.rs:561-599` | Saturating math + `min(max_hp)` cap + `check_dead_and_apply` pairing |
| Re-derive stats after buff/perk | New re-derive plugin | Write `EquipmentChangedEvent { slot: EquipSlot::None }` | Dual-use trigger; reused by Inn rest, Temple revive, status apply (see `inventory.rs:200-213`) |
| Tree DAG traversal | Recursive node walk | `Vec<NodeId>` prerequisites + closed-form `is_node_available` (linear scan) | <50 nodes per class — O(N²) check is fine; matches `ClassTable::get` rationale at `classes.rs:33-44` |
| MP deduction | New MP system | Inline in `CastSpell` arm of `execute_combat_actions` | Mirror the `UseItem` consumable removal at `turn_manager.rs:574-589` |
| Asset loader | New `SpellLoaderPlugin` | Add `RonAssetPlugin::<SpellDb>::new(&["spells.ron"])` to `LoadingPlugin` | Loader stub already at `loading/mod.rs:135`; just upgrade the type |
| Revive path | New revive handler | Mirror Temple's exception path at `temple.rs:285-330` | Documented order: effects.retain → current_hp = 1 → fire EquipmentChangedEvent |
| Skill-point allocation animation | Frame-by-frame interpolation | Plain numeric ledger on `Experience` | UI polish is #25; data layer is just `unspent_skill_points: u32` |
| Spell icon rendering | Per-spell `Handle<Image>` resolver | Use `SpellAsset.icon_path: String` and resolve in painter (deferred to #25) | Matches `ItemAsset.icon_path` precedent at `items.rs:104-107` |

---

## Common Pitfalls

### Pitfall 1: RON double-dot extension

**What goes wrong:** Naming a file `assets/spells/core_spells.ron` (single-dot) or `assets/skills/fighter_skills.ron` produces a runtime "Could not find an asset loader matching" panic on `cargo run`. Unit tests pass (they use `ron::from_str` directly).

**Why it happens:** Bevy parses the asset extension as everything after the FIRST dot. `RonAssetPlugin::<SpellDb>::new(&["spells.ron"])` matches `*.spells.ron`, not `*spells*.ron`.

**How to avoid:** New files MUST be `core.spells.ron`, `fighter.skills.ron`, `mage.skills.ron`, `priest.skills.ron`. The current stub at `assets/spells/core.spells.ron` is correctly named. The new skill-tree files must follow the same convention. Cross-checked against `project_druum_ron_asset_naming` memory.

### Pitfall 2: `Experience` discriminant order

**What goes wrong:** Inserting `unspent_skill_points` between `current_xp` and `xp_to_next_level` breaks save-format stability.

**Why it happens:** `Experience` derives `Reflect, Serialize, Deserialize` and is part of `PartyMemberBundle`. Field-position changes shift serialized output for ANY serde format that uses positional encoding (RON is name-keyed so usually safe, but #23 save format is not yet pinned).

**How to avoid:** ALWAYS APPEND new fields. `unspent_skill_points` and `total_skill_points_earned` go at the END of `Experience`. Both `#[serde(default)]` so pre-#20 save data still loads.

### Pitfall 3: `CastSpell` arm forgetting MP deduction

**What goes wrong:** Player casts the same spell every turn for free.

**Why it happens:** The `Defend` arm at `turn_manager.rs:520-530` is a write-once event; `UseItem` removes from inventory; `Attack` doesn't touch MP. The `CastSpell` arm has no precedent to crib from — easy to forget the `derived.current_mp -= spell.mp_cost` line.

**How to avoid:** Write the MP deduction test FIRST. Pattern: spawn party member with `current_mp = 10`, queue `CastSpell { spell_id: "lightning" }` (cost 5), assert `current_mp == 5` after Update. Mirror `defend_writes_defense_up_via_apply_status_event` at `turn_manager.rs:1283-1310`.

### Pitfall 4: Spell resolver mutating `StatusEffects` directly

**What goes wrong:** Stack rules are violated; multiple `Poison` instances appear on one character.

**Why it happens:** Tempting shortcut: `let mut status = chars.get_mut(target); status.effects.push(...)` skips `ApplyStatusEvent`. The handler at `status_effects.rs:177-235` enforces stack merge (`take_higher` magnitude + refresh duration). Bypassing it produces duplicate effects.

**How to avoid:** Spell resolver writes ONLY `ApplyStatusEvent`. There is a grep-guard comment in the codebase at `inventory.rs` that you can mirror; add one to spell_cast.rs. **Specifically:** the Sole-Mutator-of-StatusEffects-Is-`apply_status_handler` invariant is documented at `status_effects.rs:160-175`.

### Pitfall 5: Skill tree cycles

**What goes wrong:** All nodes lock. Game appears to have no skill tree.

**Why it happens:** A typo in a `prerequisites` field creates a cycle (A requires B, B requires A). The naïve `is_available` check loops forever or returns false for every node.

**How to avoid:** `validate_no_cycles(&SkillTree) -> Result<(), CycleError>` runs once on asset load. Topological sort via Kahn's algorithm (~20 LOC). Fail-fast: log error, mark tree as empty.

### Pitfall 6: Effect potency NaN through RON

**What goes wrong:** `magnitude: f32::NAN` clamps to 0 silently (per `status_effects.rs:185-189`), so a buff has no effect.

**Why it happens:** A typo in `spells.ron` like `potency: NaN` parses successfully via ron 0.12.

**How to avoid:** Add the same defensive `is_finite()` clamp pattern to `SpellAsset.mp_cost`/`SpellEffect`'s float fields when authored, OR rely on `apply_status_handler`'s existing clamp (preferred — single trust boundary). Add a unit test `spell_with_nan_potency_clamps_to_zero` mirroring `apply_status_handler_clamps_nan_to_zero` at `status_effects.rs:629-647`.

### Pitfall 7: Forgetting `RonAssetPlugin<T>` registration when switching to non-empty `SpellDb`

**What goes wrong:** First `cargo run` after replacing `SpellTable {}` with `SpellDb { spells: [...] }` fails with "wrong asset type."

**Why it happens:** The loader at `loading/mod.rs:135` currently registers `RonAssetPlugin::<SpellTable>`. Renaming the type without updating the registration silently breaks loading.

**How to avoid:** Rename `SpellTable` → `SpellDb` in BOTH `data/spells.rs` AND `loading/mod.rs:135` AND `data/mod.rs` (the `pub use spells::SpellTable` line). Alternatively, keep the name `SpellTable` for backwards-compat. Recommend `SpellDb` to match `ItemDb`/`EnemyDb` precedent.

### Pitfall 8: Allowing skill points to be spent below required level

**What goes wrong:** Player allocates 99 points to one node that should require level 99 first.

**Why it happens:** Nothing in the proposed `SkillNode` schema gates by level — only by `prerequisites: Vec<NodeId>`.

**How to avoid:** Add `#[serde(default)] pub min_level: u32` to `SkillNode`. Pure fn `can_unlock_node(&node, &experience, &unlocked) -> Result<(), SkillError>` checks both `level >= node.min_level` AND `all prerequisites are in unlocked`. Mirrors `can_create_class` at `progression.rs:307-354`.

### Pitfall 9: Trust-boundary on `mp_cost`

**What goes wrong:** A crafted RON specifies `mp_cost: u32::MAX`. Caster can never cast; effectively bricked.

**Why it happens:** No upstream clamp.

**How to avoid:** Clamp at consumer side: `spell.mp_cost.min(MAX_SPELL_MP_COST)` (suggested constant = 999, matching Wizardry mp pool size). Apply at the MP check in the resolver. Defense-in-depth: also clamp `power` and `amount` on `SpellEffect` variants.

### Pitfall 10: Re-deriving stats after `StatBoost` perk unlock

**What goes wrong:** Player spends a point on a +2 STR node and stat doesn't change until next combat / next equipment swap.

**Why it happens:** `derive_stats` reads `BaseStats` once at spawn / level-up / equipment-change. Adding a perk doesn't trigger anything.

**How to avoid:** When a `StatBoost` node is unlocked, apply the delta directly to `BaseStats` via `saturating_add` (matches `allocate_bonus_pool` at `progression.rs:254-294`), then write `EquipmentChangedEvent { character, slot: EquipSlot::None }` to trigger re-derive via the dual-use pathway at `inventory.rs:444-510`.

---

## Security

### Known Vulnerabilities

No new dependencies → no new CVE surface for #20. All extant dependency versions are pinned and already audited by prior features (research/plans #11 through #19).

| Library | CVE / Advisory | Severity | Status | Action |
| --- | --- | --- | --- | --- |
| bevy 0.18.1 | None found | — | — | Re-use existing pin |
| ron 0.12 | None found in `0.12.x` | — | — | Round-trip tested per #11 |
| rand 0.9 | RUSTSEC-2024-0429 (rand 0.6 only) | LOW | Patched | Druum is on 0.9 — unaffected |
| serde 1 / serde_derive | None found | — | — | Same as existing |
| bevy_egui 0.39.1 | None found | — | — | UI overlay only |

No known CVEs or advisories applicable to the recommended approach as of 2026-05-14.

### Architectural Security Risks

| Risk | Affected Architecture Options | How It Manifests | Secure Pattern | Anti-Pattern to Avoid |
| --- | --- | --- | --- | --- |
| Untrusted RON spell potency | A1, A2 | Crafted save sets `potency: 999.9` → instant party wipe via NaN-coerced buff | Clamp at trust boundary: `potency.clamp(0.0, 10.0)` (already enforced at `status_effects.rs:185-189` — spells just need to fire the event, not bypass the handler) | Spell resolver writes `effects.push(ActiveEffect { magnitude: ev.potency, ... })` directly, skipping the clamp |
| Untrusted skill-point allocation | B1 | Crafted save sets `unspent_skill_points: u32::MAX`; player unlocks the entire tree instantly | Clamp `unspent_skill_points` at deserialize via `#[serde(deserialize_with = ...)]` OR validate against `total_skill_points_earned <= level_cap()` at the spend-point handler | Trust the field unconditionally |
| Cyclic prerequisites DoS | B1 | A crafted `<class>.skills.ron` has a cycle → infinite loop on first `is_node_available` call | `validate_no_cycles` on asset load; fail-fast | Topo-sort lazily during paint |
| Asset path traversal in `icon_path` | A1 | RON specifies `icon_path: "../../../etc/passwd"` | Same mitigation as `ItemAsset.icon_path`: rendered by future #25 UI which resolves via `Handle<Image>` (the asset pipeline rejects out-of-`assets/` paths) | Direct filesystem read in painter |
| `KnownSpells` vector unbounded | A1, B1 | Crafted save with `spells: Vec<SpellId>` of length 10M | Clamp on deserialize: `KNOWN_SPELLS_MAX = 64`; truncate on load; matches `clamp_recruit_pool` at `data/town.rs:120-127` | `Vec` length unchecked |
| `SkillNode.cost = u32::MAX` | B1 | Player can never unlock the node | Clamp `cost.clamp(0, MAX_SKILL_NODE_COST)` (suggested = 99) in `can_unlock_node` | Treat as authoritative |

### Trust Boundaries

The recommended architecture has three new trust boundaries:

- **RON-loaded `SpellAsset`:** untrusted. Clamp `mp_cost`, `level`, `effect.{power,amount,duration,potency}` at consumer side. The existing `ApplyStatusEvent` handler already clamps potency `[0.0, 10.0]` and is fail-safe on non-finite values. **What happens if skipped:** crafted spell `Damage { power: 999_999 }` → one-shots boss without variance/defense reduction.
- **RON-loaded `SkillTree`:** untrusted. `validate_no_cycles` at load; clamp `nodes` length to `MAX_SKILL_TREE_NODES`; clamp `cost` and `min_level` per node. **What happens if skipped:** infinite paint loop or instant-unlock everything.
- **Save-loaded `KnownSpells` / `UnlockedNodes` / `Experience.unspent_skill_points`:** untrusted from #23 save data. Cross-validate `unlocked ⊆ nodes_in_class_tree`; cap `unspent_skill_points` at `total_skill_points_earned`. **What happens if skipped:** save-edit grants spells the player's class shouldn't have.

---

## Performance

| Metric | Value / Range | Source | Notes |
| --- | --- | --- | --- |
| `SpellDb::get` lookup | O(N) over ~15-25 spells | Pattern mirror of `ItemDb::get` at `items.rs:36-43` | Linear scan; HashMap unnecessary for v1 |
| `is_node_available` per node | O(N²) worst case over ~10-20 nodes per class | Recommended pure-fn shape | Run only on point-spend; not per-frame |
| Spell resolver hot path | Single `match` + 1-3 mutations per cast | Pattern mirror of `Attack` arm at `turn_manager.rs:438-519` | One cast per turn per character; ~4 casts/turn at most |
| `validate_no_cycles` | O(N + E) once per asset load | Topo-sort | <1ms for ~10 nodes |
| `apply_level_up_threshold_system` SkillPoint award | +1 `saturating_add` per level-up | `progression.rs:438-471` | Trivial; level-up is rare |
| RON asset size | ~5-10KB per `core.spells.ron` (20 spells) | Pattern from `core.items.ron` shape | Loaded once at boot |

No benchmark dependencies — all costs are dominated by the existing combat loop. Spells DO NOT add per-frame work outside the cast-resolution step.

---

## Code Examples

Verified pattern templates from the existing codebase that #20 should mirror:

### 1. Replacing the CastSpell stub (entry point)

```rust
// Source: src/plugins/combat/turn_manager.rs:531-541 (existing stub)
CombatActionKind::CastSpell { spell_id } => {
    // Decision 32: stub.
    combat_log.push(
        format!(
            "{} casts {}: not yet implemented.",
            name_of(action.actor),
            spell_id
        ),
        turn,
    );
}
```

Replace with the Pattern 5 resolver from above. The system signature must gain:

```rust
spell_db: Res<Assets<SpellDb>>,
spell_handle: Res<SpellDbHandle>,       // new Resource(Handle<SpellDb>) for one-shot lookup
known_spells: Query<&KnownSpells, With<PartyMember>>,
```

### 2. SpellMenu — filter known + MP-affordable, then push TargetSelect

```rust
// Source: src/plugins/combat/ui_combat.rs:457-473 (existing stub)
MenuFrame::SpellMenu => {
    // Decision 34: Silence gates spell access.
    if is_silenced(status) {
        combat_log.push("You are silenced; cannot cast.".into(), input_state.current_turn);
        input_state.menu_stack = vec![MenuFrame::Main];
        return;
    }
    // Stub for v1; #20 fills in spell menu.
    combat_log.push("Spell menu: not yet implemented.".into(), input_state.current_turn);
    input_state.menu_stack = vec![MenuFrame::Main];
}
```

Replace with paint+handler pair following the cursor pattern at `ui_combat.rs:359-411` (Main-cursor) — `PlayerInputState.spell_cursor: usize` + arrow-driven nav + Confirm → push `MenuFrame::TargetSelect { kind: CombatActionKind::CastSpell { spell_id: castable[cursor].id.clone() } }`.

### 3. Buff via existing `ApplyStatusEvent`

```rust
// Source: src/plugins/combat/turn_manager.rs:520-530 (existing Defend arm — same pattern)
apply_status.write(ApplyStatusEvent {
    target: action.actor,
    effect: StatusEffectType::AttackUp,
    potency: 0.3,                  // +30% from a buff spell
    duration: Some(3),             // 3 rounds
});
combat_log.push(format!("{} casts Bless.", name_of(action.actor)), turn);
```

### 4. Heal via consumable-pattern mirror

```rust
// Source: src/plugins/combat/turn_manager.rs:567-598 (existing consumable arm — same pattern)
let heal_amount = spell.heal_amount; // or scaled by caster.magic_attack
if let Ok(mut target_derived) = derived_mut.get_mut(target) {
    target_derived.current_hp = target_derived.current_hp.saturating_add(heal_amount).min(target_derived.max_hp);
    check_dead_and_apply(target, &target_derived, &mut apply_status);
}
combat_log.push(format!("{} heals {} for {}.", caster_name, target_name, heal_amount), turn);
```

### 5. Revive via exception path

```rust
// Source: src/plugins/town/temple.rs:285-330 (existing Revive system — exception path)
// Order MATTERS: retain → set HP → fire event
if let Ok((mut derived, mut status)) = chars.get_mut(target) {
    status.effects.retain(|e| e.effect_type != StatusEffectType::Dead);
    derived.current_hp = revive_hp; // typically 1 (Wizardry convention)
    equipment_changed.write(EquipmentChangedEvent { character: target, slot: EquipSlot::None });
}
```

### 6. Skill point award on level-up

```rust
// File: src/plugins/party/progression.rs — insert after line 453
exp.level = exp.level.saturating_add(1);
// NEW for #20:
exp.unspent_skill_points = exp.unspent_skill_points.saturating_add(1);
exp.total_skill_points_earned = exp.total_skill_points_earned.saturating_add(1);
```

### 7. Spell list RON shape

```ron
// File: assets/spells/core.spells.ron
(
    spells: [
        (
            id: "halito",
            display_name: "Halito",
            mp_cost: 2,
            level: 1,
            school: Mage,
            target: SingleEnemy,
            effect: Damage(power: 8),
            description: "A small flame attack.",
        ),
        (
            id: "dios",
            display_name: "Dios",
            mp_cost: 2,
            level: 1,
            school: Priest,
            target: SingleAlly,
            effect: Heal(amount: 8),
            description: "Heals one ally.",
        ),
        (
            id: "katino",
            display_name: "Katino",
            mp_cost: 3,
            level: 1,
            school: Mage,
            target: AllEnemies,
            effect: ApplyStatus(effect: Sleep, potency: 1.0, duration: Some(3)),
            description: "Lulls a group of enemies to sleep.",
        ),
        (
            id: "matu",
            display_name: "Matu",
            mp_cost: 5,
            level: 2,
            school: Priest,
            target: AllAllies,
            effect: Buff(effect: AttackUp, potency: 0.3, duration: 3),
            description: "Bestows blessing on all allies.",
        ),
        (
            id: "di",
            display_name: "Di",
            mp_cost: 30,
            level: 5,
            school: Priest,
            target: SingleAlly,
            effect: Revive(hp: 1),
            description: "Resurrects a fallen ally to 1 HP.",
        ),
        // Continue with ~15-20 more (mage damage tiers, priest cures, etc.)
    ],
)
```

### 8. Skill tree RON shape

```ron
// File: assets/skills/mage.skills.ron
(
    class_id: "Mage",
    nodes: [
        (
            id: "mage_lvl1_combat",
            display_name: "Combat Magic 1",
            cost: 1,
            prerequisites: [],
            grant: LearnSpell("halito"),
            description: "Learn Halito (Damage 8 / Single).",
        ),
        (
            id: "mage_lvl1_control",
            display_name: "Crowd Control 1",
            cost: 1,
            prerequisites: [],
            grant: LearnSpell("katino"),
            description: "Learn Katino (Sleep / All Enemies).",
        ),
        (
            id: "mage_lvl2_combat",
            display_name: "Combat Magic 2",
            cost: 2,
            prerequisites: ["mage_lvl1_combat"],
            grant: LearnSpell("mahalito"),
            description: "Learn Mahalito (Damage 20 / Group).",
        ),
        (
            id: "mage_perk_int",
            display_name: "Arcane Mind",
            cost: 1,
            prerequisites: [],
            grant: StatBoost(intelligence: 2),
            description: "+2 Intelligence.",
        ),
        // ... ~6-10 more nodes per class
    ],
)
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
| --- | --- | --- | --- |
| Wizardry-style 7×7 spell matrix (memorise N slots per level) | Modern flat KnownSpells + MP pool | 1990s onward (Final Fantasy / Bard's Tale 3+) | Less bookkeeping; aligns with #20 design |
| Hard-coded class skill paths | Player-allocated skill points / talent trees | 1996 (Diablo 1+) | Adds replayability + class identity (the cornerstone of #20) |
| One spell per turn per character | Spell + free action mix (rare in Wizardry-likes) | Modern WRPGs (Pillars, Pathfinder) | Druum stays one-action-per-turn — matches `QueuedAction` shape |
| Spell components tracked separately (V, S, M from D&D) | Single integer MP cost | Modern CRPG mainstream | Druum already uses MP — no V/S/M overhead |

**Deprecated/outdated approaches to avoid:**

- **In-memory spell-learning at the per-character level (no tree).** Loses class identity, gets boring fast. The roadmap explicitly mandates trees.
- **One file per spell (`spells/halito.ron`, etc.).** Druum's convention is a single `core.<type>.ron` table; matches `core.items.ron`/`core.enemies.ron`/`core.classes.ron`. Maintain the convention.
- **Bevy `Component` per spell (`#[derive(Component)] struct Halito;`).** Spell IS data, not behaviour-on-an-entity. The component would never be attached to anything. Reject.

---

## Validation Architecture

### Test Framework

| Property | Value |
| --- | --- |
| Framework | `cargo test` (built-in) — both unit (`#[test]` inside modules) and app-level integration (`tests/*.rs`) |
| Config file | `Cargo.toml` (existing dev-deps `rand_chacha = 0.9`) |
| Quick run command | `cargo test --lib spells` (filters to spell-related tests, runs ~under 30s) |
| Full suite command | `cargo test` and `cargo test --features dev` (both must pass) |
| Clippy gate | `cargo clippy --all-targets -- -D warnings` (also `--features dev`) |

### Requirements → Test Map

The roadmap (line 1113-1124) lists the broad todo. Below maps each to a test category and command.

| Requirement | Behavior | Test Type | Automated Command | File Exists? |
| --- | --- | --- | --- | --- |
| Define `SpellAsset` schema | Round-trips through RON without field loss | Unit | `cargo test spell_asset_round_trips` | ❌ needs creating in `src/data/spells.rs` |
| Define `SpellDb` | Loads via `RonAssetPlugin` end-to-end | App-level | `cargo test --test spell_db_loads` | ❌ needs creating at `tests/spell_db_loads.rs` (mirror `tests/item_db_loads.rs`) |
| `SpellEffect::Damage` resolves | Damage applied to target's `current_hp`; deterministic with seeded RNG | Unit (pure fn) | `cargo test spell_damage_calc` | ❌ needs creating in `src/plugins/combat/spell_cast.rs` |
| `SpellEffect::Heal` caps at max_hp | Cap enforced + `check_dead_and_apply` paired | App-level | `cargo test heal_caps_at_max_hp` | ❌ needs creating |
| `SpellEffect::ApplyStatus` writes event | Verifies event written; handler picks up next frame | App-level | `cargo test spell_apply_status_writes_event` | ❌ needs creating |
| `SpellEffect::Buff` triggers re-derive | After spell, `derived.attack` includes buff | App-level | `cargo test buff_spell_modifies_attack_via_recompute` | ❌ needs creating |
| `SpellEffect::Revive` exception path | After Revive, `current_hp == 1` and `Dead` removed | App-level | `cargo test revive_spell_restores_at_1_hp` | ❌ needs creating |
| MP gate | Spell with cost > current_mp rejected with log | App-level | `cargo test insufficient_mp_blocks_cast` | ❌ needs creating |
| Silence gate | Silenced caster can't open spell menu (already wired) | App-level | (extend existing `silence_blocks_spell_menu` at `ui_combat.rs:587-648`) | ✅ existing test — extend to assert no `Update` MP-deduction |
| `KnownSpells.knows()` lookup | True for learned, false otherwise | Unit | `cargo test known_spells_lookup` | ❌ needs creating |
| Skill-point award on level-up | `unspent_skill_points += 1` per level-up | Unit (extend `xp_threshold_triggers_level_up`) | `cargo test skill_point_awarded_on_level_up` | 🟡 partial — `progression.rs:796-886` exists but doesn't yet test skill points |
| `SkillTree` validates no cycles | Cyclic tree returns `Err(CycleError)` | Unit | `cargo test skill_tree_rejects_cycles` | ❌ needs creating in `src/data/skills.rs` |
| `can_unlock_node` enforces prereqs | Returns `Err` when prereq not in unlocked set | Unit | `cargo test can_unlock_requires_prereqs` | ❌ needs creating in `src/plugins/party/skills.rs` |
| `LearnSpell` grant updates `KnownSpells` | After unlock, spell appears in `KnownSpells` | App-level | `cargo test unlock_node_learns_spell` | ❌ needs creating |
| `StatBoost` grant triggers re-derive | After unlock, `BaseStats` updated + `EquipmentChangedEvent` fires | App-level | `cargo test stat_boost_node_modifies_derived` | ❌ needs creating |
| Per-class tree loads | Fighter/Mage/Priest each load without panic | App-level | `cargo test --test skill_tree_loads` | ❌ needs creating at `tests/skill_tree_loads.rs` |
| Spell sim debug command (Manual smoke) | "spell sim runs N battles with random spell choices" (roadmap line 1124) | Manual (`cargo run --features dev`) | F-key dev hotkey TBD | ❌ optional, can defer to #21+ |
| End-to-end RON load smoke | `cargo run --features dev` parses `core.spells.ron` + class trees | Manual | `cargo run --features dev` | Manual smoke — double-dot check |
| End-to-end cast smoke | F9 → Dungeon → Encounter → CastSpell on spell submenu → see damage in log | Manual | `cargo run --features dev` | Manual smoke |

### Gaps (files to create before implementation)

- [ ] `src/data/spells.rs` — REWRITE: full `SpellAsset`/`SpellDb`/`SpellEffect`/`SpellSchool`/`SpellTarget` (currently 11-line stub)
- [ ] `src/data/skills.rs` — NEW: `SkillTree`/`SkillNode`/`NodeGrant`/`NodeId` + `validate_no_cycles`
- [ ] `src/plugins/party/skills.rs` — NEW: `KnownSpells`/`UnlockedNodes` components + `learn_spell`/`allocate_skill_point` pure fns
- [ ] `src/plugins/combat/spell_cast.rs` — NEW: `spell_damage_calc` pure fn + resolver helper
- [ ] `assets/spells/core.spells.ron` — REWRITE: 15-20 spell entries (currently a 4-line stub)
- [ ] `assets/skills/fighter.skills.ron` — NEW: ~6-10 nodes
- [ ] `assets/skills/mage.skills.ron` — NEW: ~10-15 nodes
- [ ] `assets/skills/priest.skills.ron` — NEW: ~10-15 nodes
- [ ] `tests/spell_db_loads.rs` — NEW: mirror `tests/item_db_loads.rs`
- [ ] `tests/skill_tree_loads.rs` — NEW: mirror `tests/class_table_loads.rs`
- [ ] Extend `src/data/mod.rs`: replace `pub use spells::SpellTable;` → `pub use spells::{SpellDb, SpellAsset, SpellEffect, SpellSchool, SpellTarget};` + add `pub mod skills; pub use skills::{SkillTree, SkillNode, NodeGrant, NodeId};`
- [ ] Extend `src/plugins/loading/mod.rs`: update RonAssetPlugin registration; add per-class `Handle<SkillTree>` fields to `DungeonAssets` (or a new collection)
- [ ] Extend `src/plugins/party/character.rs`: append `unspent_skill_points` + `total_skill_points_earned` to `Experience`
- [ ] Extend `src/plugins/party/mod.rs`: add `pub mod skills;` and re-export
- [ ] Extend `src/plugins/party/progression.rs`: insert skill-point award in `apply_level_up_threshold_system`

---

## Open Questions

The roadmap is light on specifics — these decisions deserve user input before the planner finalises scope. Each is flagged with confidence: HIGH (planner can default-resolve), MEDIUM (worth surfacing), LOW (must ask).

1. **Skill tree shape: per-class trees or shared?** (LOW)
   - What we know: roadmap line 1100 says "`assets/skills/<class>.skills.ron`" (per-class).
   - What's unclear: whether all three classes (Fighter/Mage/Priest) ship trees day-one, or whether Fighter (which has no spells per `core.classes.ron:31` `mp_per_level: 0`) gets passives-only.
   - Recommendation: per-class trees for ALL three classes. Fighter tree has `StatBoost` and `Resist` grants only, no `LearnSpell`. Authoring the Fighter tree (even with 3-4 nodes) preserves the user-promised tree feel for non-casters.

2. **Race-specific spells / racial restrictions?** (LOW)
   - What we know: roadmap doesn't mention race-gated spells. `RaceData` schema at `src/data/races.rs` has no `allowed_spells` or `spell_modifier` fields.
   - What's unclear: should Elves get bonus to mage spell damage? Dwarves resist Sleep?
   - Recommendation: defer to #25 polish. Day-one #20: race has zero effect on spells. Document as forward-compat (add `RaceData.spell_modifier: Option<f32>` later if needed; would be `#[serde(default)]`).

3. **MP regeneration: on level-up, on rest, both?** (HIGH)
   - What we know: `current_mp = max_mp` on level-up (`progression.rs:460`); `current_mp = max_mp` on Inn rest (`inn.rs:153`). No per-combat-turn or per-step regen.
   - What's unclear: does #20 add per-turn MP regen during combat?
   - Recommendation: NO per-turn regen day-one. Matches Wizardry/Etrian convention. If desired, ship a `Regen`-symmetric MP regen status as a Phase 2 polish (low scope cost).

4. **Spells have ranges / AoE in a grid game?** (HIGH)
   - What we know: combat is NOT grid-based — combat happens "abstractly" between two rows of party + group of enemies. The dungeon is grid-based but encounters are room-based.
   - What's unclear: should `SpellTarget::Group { idx }` exist for hitting one enemy "group" out of multiple groups (Wizardry-style 4 groups of monsters)?
   - Recommendation: `SpellTarget` covers `SingleEnemy / AllEnemies / SingleAlly / AllAllies / Self_` day-one. Group targeting is a polish #25 item. The encounter at `src/plugins/combat/encounter.rs` already supports multiple enemy groups internally; UI targeting would need extension which is out of scope for #20.

5. **Front/back row affect spell damage?** (MEDIUM)
   - What we know: physical attacks at `damage.rs:96-112` block Front→Back. Spells skip this entirely (proposal: `spell_damage_calc` does not check row).
   - What's unclear: should some `SpellEffect::Damage` variants respect row (a "lightning bolt" arcs but a "ground spike" doesn't)?
   - Recommendation: ALL spells ignore rows day-one. Spell power is the differentiator from physical attacks. If needed, add `#[serde(default)] pub melee_like: bool` to `SpellEffect::Damage` in a future polish.

6. **Skill point amount per level-up: 1 or scalable?** (MEDIUM)
   - What we know: roadmap line 1119 says "skill-point allocation (1-2 per level up, configurable)".
   - What's unclear: 1 per level (simpler) or 2 (more allocations / faster tree filling)?
   - Recommendation: 1 per level-up day-one. Configurable via a `SKILL_POINTS_PER_LEVEL: u32` const that the level-up system reads. Future polish can swap to per-class via `ClassDef.skill_points_per_level: u32` (additive `#[serde(default)]` extension).

7. **Reset/respec ability?** (LOW)
   - What we know: roadmap doesn't mention respec.
   - What's unclear: can players reset their skill tree at the Temple (paid) or never?
   - Recommendation: NO respec day-one. Add as Temple service in a future polish if playtesting reveals respec-locked frustration.

8. **Does Bishop class (declared in `Class` enum but unauthored) get a hybrid tree?** (LOW)
   - What we know: `Class::Bishop` declared at `character.rs:68` but has no `ClassDef` entry (`Class { Bishop, Samurai, Lord, Ninja, Thief }` are declared-but-unauthored).
   - What's unclear: scope of #20 includes Bishop / other unauthored classes?
   - Recommendation: NO. #20 ships trees only for the 3 v1 classes (Fighter/Mage/Priest). Mirrors the #19 decision pattern.

9. **What happens if `KnownSpells` references a spell that doesn't exist in `SpellDb`?** (HIGH)
   - What we know: `SpellDb::get` returns `Option<&SpellAsset>`. The proposed `castable.filter_map(|id| spell_db.get(id))` swallows missing references.
   - What's unclear: should this `warn!` or silently filter?
   - Recommendation: `warn!` once on the first miss per character (track in a `Resource WarnedMissingSpells: HashSet<SpellId>`), then silently filter. Same defense-in-depth as `recompute_derived_stats_on_equipment_change` at `inventory.rs:475-487` warning on missing item handles.

10. **Spell-icon paths: ship day-one or defer?** (HIGH)
    - What we know: `SpellAsset.icon_path: String` matches `ItemAsset.icon_path` precedent.
    - What's unclear: are icons part of #20 scope or #25 polish?
    - Recommendation: defer icons to #25. Day-one painter shows spell names + MP cost only. Field is authored as `""` in RON.

11. **Should the spell sim debug command (roadmap line 1124) ship with #20?** (LOW)
    - What we know: roadmap explicitly mentions "Add a 'spell sim' debug command that runs N battles with random spell choices to surface broken combinations."
    - What's unclear: is this in-scope or punt to a future polish/standalone tool?
    - Recommendation: defer to a follow-up feature. Day-one #20 ships gameplay; balance tooling is its own work-unit.

---

## Sources

### Primary (HIGH confidence)

- [Druum codebase](https://github.com/codeinaire/druum) — Read directly from `/Users/nousunio/Repos/Learnings/claude-code/druum/src/**` and `/Users/nousunio/Repos/Learnings/claude-code/druum/assets/**` as of 2026-05-14
  - `src/data/spells.rs` — current stub (11 LOC)
  - `src/plugins/combat/actions.rs:21-33` — `CombatActionKind::CastSpell { spell_id }` pre-shipped
  - `src/plugins/combat/turn_manager.rs:531-541` — existing stub resolver
  - `src/plugins/combat/turn_manager.rs:99-104` — `MenuFrame::SpellMenu` pre-shipped
  - `src/plugins/combat/ui_combat.rs:457-473` — existing stub UI + silence gate
  - `src/plugins/combat/status_effects.rs:79-86,177-235` — `ApplyStatusEvent` + sole-mutator handler
  - `src/plugins/combat/damage.rs:62-152` — `damage_calc` pure-fn precedent
  - `src/plugins/party/character.rs:132-165` — `DerivedStats.current_mp/max_mp` + `Experience`
  - `src/plugins/party/progression.rs:1-499` — `apply_level_up_threshold_system` + `level_up` pure fn
  - `src/plugins/party/inventory.rs:444-510` — `recompute_derived_stats_on_equipment_change` (dual-use)
  - `src/plugins/town/temple.rs:285-330` — Revive exception-path precedent (effects.retain → set HP → fire event)
  - `src/plugins/town/inn.rs:153` — MP refill on rest precedent
  - `src/plugins/loading/mod.rs:46,135` — `SpellTable` loader + `core.spells.ron` path
  - `src/data/items.rs:84-118` — `ItemAsset` shape precedent
  - `src/data/classes.rs:65-106` — `ClassDef` additive-extension pattern (matches #19)
  - `assets/spells/core.spells.ron` — current 4-line stub
  - `assets/classes/core.classes.ron` — `mp_per_level` per class (Fighter:0, Mage:6, Priest:5)
  - `Cargo.toml` — all crates pinned (no Δ needed)
- [Druum roadmap §20 Spells & Skill Trees](file:///Users/nousunio/Repos/Learnings/claude-code/druum/project/roadmaps/20260429-01-bevy-dungeon-crawler-roadmap.md) lines 1079-1127 — primary scope source absent direct `gh issue view 20`
- [Druum #19 plan (precedent for ClassDef additive extension)](file:///Users/nousunio/Repos/Learnings/claude-code/druum/project/plans/20260513-120000-feature-19-character-creation.md) — adopted patterns: `#[serde(default)]` on new fields, `Class` enum filter via `is_some()`, `Option<&Class>` wildcard arms, MAX_* trust-boundary constants, double-dot extension contract, `?Sized` bound on RNG pure fns

### Secondary (MEDIUM confidence)

- [Druum #15 turn-based combat research](file:///Users/nousunio/Repos/Learnings/claude-code/druum/project/research/20260508-093000-feature-15-turn-based-combat-core.md) — Published 2026-05-08, Accessed 2026-05-14. Source of `CombatActionKind::CastSpell` shape; `MenuFrame::SpellMenu` design; `is_silenced` predicate gate decision.
- [Druum #14 status effects research](file:///Users/nousunio/Repos/Learnings/claude-code/druum/project/research/20260507-115500-feature-14-status-effects-system.md) — Published 2026-05-07, Accessed 2026-05-14. Source of: `ApplyStatusEvent` as sole-mutator pattern; `magnitude.clamp(0.0, 10.0)` trust boundary; D5α `EquipmentChangedEvent` dual-use re-derive.
- [Druum #18b Temple plan (exception path for status removal)](file:///Users/nousunio/Repos/Learnings/claude-code/druum/project/plans/20260512-173000-feature-18b-town-temple-guild.md) — Accessed 2026-05-14. Source of Revive ordering invariant (effects.retain BEFORE current_hp write BEFORE event fire).

### Tertiary (LOW confidence)

- Wizardry-1 (1981) school/level/cost conventions for spell naming (e.g., Halito / Dios / Katino / Mahalito / Di / Matu). Sourced from genre familiarity; the Druum project explicitly cites Wizardry/Etrian convention multiple times (`damage.rs:1-12`, `progression.rs:114-122`). Accessed 2026-05-14.
- Diablo / Path of Exile / WoW talent-tree shapes for B2 vs B1 comparison. General industry knowledge; no specific URL.

---

## Metadata

**Confidence breakdown:**

- Combat integration (CastSpell resolver, MP, MenuFrame::SpellMenu wiring): HIGH — every seam already in tree
- Status & buff/heal resolution: HIGH — exact patterns documented in #14/#15 plans
- Asset schema (SpellAsset + SpellDb): HIGH — mirrors ItemDb byte-for-byte
- Skill tree DAG shape: MEDIUM — recommended pattern (B1) is conservative; could be replaced with B2 grid talent matrix if user wants visual-coordinate authoring
- Skill point award math: MEDIUM — 1-per-level is the planner default; user may want 2-per-level
- Bevy ecosystem version compat: HIGH — Δ Cargo.toml = 0; all extant pins verified working
- Pitfalls: MEDIUM-HIGH — well-tested patterns from #11-#19; RON double-dot is the highest-frequency miss
- Open questions: this section is by design where the residual ambiguity lives

**Research date:** 2026-05-14

**Recommendation summary for the planner (carry into the plan as Critical):**

1. **Δ Cargo.toml = 0.** No new crates.
2. **Replace the existing `SpellTable {}` stub with `SpellDb { spells: Vec<SpellAsset> }`** — both the type at `src/data/spells.rs` AND the registration at `src/plugins/loading/mod.rs:135`. Re-exports in `src/data/mod.rs:30` must be updated.
3. **Append, do not insert:** new fields on `Experience` (skill points) MUST go at the END for save-format stability.
4. **Resolver order:** spell pipeline mirrors existing arms — MP check → resolve targets via `resolve_target_with_fallback` → dispatch on `SpellEffect` → write `ApplyStatusEvent`s and / or apply HP via `derived_mut` paired with `check_dead_and_apply`. The `Revive` exception path mirrors Temple revive at `temple.rs:285-330`.
5. **RON double-dot extensions are mandatory.** All new files: `core.spells.ron` (already correctly named), `fighter.skills.ron`, `mage.skills.ron`, `priest.skills.ron`. `RonAssetPlugin::<SkillTree>::new(&["skills.ron"])` is the registration extension.
6. **`KnownSpells: Vec<SpellId>`** (NOT `Vec<Handle<SpellAsset>>`) — `Handle<T>` lacks serde in Bevy 0.18, same gotcha as `Equipment` at `character.rs:204-211`.
7. **Trust boundaries:** clamp `mp_cost.min(MAX_SPELL_MP_COST=999)`, `effect.power.min(MAX_SPELL_DAMAGE=999)`, `effect.amount.min(MAX_SPELL_HEAL=999)`, `effect.duration.min(MAX_SPELL_DURATION=99)`, `nodes.len() <= MAX_SKILL_TREE_NODES=64`, `unspent_skill_points <= total_skill_points_earned`, `KnownSpells.spells.len() <= KNOWN_SPELLS_MAX=64`.
8. **DAG validation on asset load.** `validate_no_cycles(&SkillTree) -> Result<(), CycleError>` runs once; fail-fast with `error!` log.
9. **Phase split:** Phase 1 = `SpellDb` + `CastSpell` resolver. Phase 2 = `KnownSpells` + functional `MenuFrame::SpellMenu`. Phase 3 = `SkillTree` data + level-up skill points + Guild "view tree" UI. Phases 1+2 unblock playable spell-casting; Phase 3 is the per-class progression layer.
10. **LOC estimate:** ~900-1300 LOC + ~15-20 tests + 4 new RON files + 1 RON-asset-loads integration test (matches roadmap +700-1200 range).

Suggested MAX_* constants to add (one place — recommend `src/data/spells.rs` for spell-side, `src/data/skills.rs` for tree-side, mirroring `data/town.rs:24-46`):

```rust
// src/data/spells.rs
pub const MAX_SPELL_MP_COST: u32 = 999;
pub const MAX_SPELL_DAMAGE: u32 = 999;
pub const MAX_SPELL_HEAL: u32 = 999;
pub const MAX_SPELL_DURATION: u32 = 99;
pub const KNOWN_SPELLS_MAX: usize = 64;

// src/data/skills.rs
pub const MAX_SKILL_TREE_NODES: usize = 64;
pub const MAX_SKILL_NODE_COST: u32 = 99;
pub const SKILL_POINTS_PER_LEVEL: u32 = 1;
```
