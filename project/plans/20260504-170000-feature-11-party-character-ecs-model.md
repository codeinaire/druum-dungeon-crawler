# Plan: Party & Character ECS Model — Feature #11

**Date:** 2026-05-04
**Status:** Draft
**Research:** ../research/20260504-160000-feature-11-party-character-ecs-model.md
**Depends on:** 20260501-220000-feature-3-asset-pipeline-loading-flow.md, 20260501-230000-feature-4-dungeon-grid-data-model.md, 20260502-000000-feature-5-input-system-leafwing.md

## Goal

Add the character data layer that every later feature reads from: 12 components (`CharacterName`, `Race`, `Class`, `BaseStats`, `DerivedStats`, `Experience`, `PartyRow`, `PartySlot`, `Equipment`, `StatusEffects`, `ActiveEffect`, `StatusEffectType`) plus a `PartyMember` marker, the `PartyMemberBundle`, the pure `derive_stats(base, equip_stats, status, level) -> DerivedStats` function, the `PartySize: Resource` hard-cap, and a `#[cfg(feature = "dev")]`-gated `spawn_default_debug_party` system. Author `assets/classes/core.classes.ron` with 3 classes (Fighter / Mage / Priest, deterministic per-level growth). Stub `ItemAsset` + `ItemStatBlock` types in `src/data/items.rs` so `Handle<ItemAsset>` resolves for `Equipment` slots. Net delivery: **Δ deps = 0** — `Cargo.toml`, `Cargo.lock`, and `src/main.rs` are all byte-unchanged.

## Approach

The research recommends **Option A (Components-as-data with `Equipment = Option<Handle<ItemAsset>>`)** and the user resolved all 8 Category C decisions to the "all A" defaults. Each character is one entity carrying ten components plus the `PartyMember` marker; `derive_stats` is a pure function over `&BaseStats`, `&[ItemStatBlock]`, `&StatusEffects`, and `level: u32` — no `Mut<T>`, no entity lookups, no resource reads. Equipment slots hold `Option<Handle<ItemAsset>>` (not `Option<Entity>`) so save/load (#23) gets a serializable asset-path representation for free, sidestepping the `MapEntities` retrofit Option B would owe.

The architectural decision baked in: **components-as-data with serde from day one** (research §Pattern 3 / §Pitfall 5). Every component declared in this feature derives `Component, Reflect, Serialize, Deserialize, Default, Debug, Clone, PartialEq` (and `Eq + Hash` where the inner type allows). This is non-negotiable scope — Feature #23 (save/load) is the trust-boundary consumer of these derives, and bolting them on later means re-touching every save file.

The single load-bearing structural decision (the user did NOT explicitly resolve, but follows from research recommendations): **`src/data/classes.rs` imports `Class` and `BaseStats` from `src/plugins/party/character.rs`** — a one-way reverse dep from `data/` to `plugins/` (no cycle). Research's Pattern 3 puts the component value-types in `character.rs`; `ClassDef` needs a `Class` discriminator and a `BaseStats` shape; the alternative (move `Class` and `BaseStats` to `data/`) was considered and rejected as more disruptive (the rest of the components stay in `character.rs`, splitting only two of them looks arbitrary). Documented inline in both files so a future contributor knows the inversion is intentional.

The build follows the established #4 / #9 / #10 atomic-commit discipline: data-types and stubs first, then the plugin, then the system, then tests, then the integration test. Each commit compiles; `cargo test` passes at every checkpoint. **The cleanest-ship signal for #11 is Δ deps = 0 + `src/main.rs` byte-unchanged** (PartyPlugin is already registered as an empty stub at line 29 of `main.rs`'s plugin tuple) — same shape as Features #7, #8, #9.

## Critical

- **Δ deps = 0 is mandatory.** No `Cargo.toml` modifications. No new crates. `Cargo.lock` must be byte-unchanged. Final `git diff Cargo.toml` and `git diff Cargo.lock` MUST show zero changes. If implementer feels they need a new dep, STOP and surface as a question.

- **`src/main.rs` is byte-unchanged.** `PartyPlugin` is already in the `add_plugins(...)` tuple at line 29. Final `git diff src/main.rs` MUST show zero changes. If a registration needs to change, the change goes inside `PartyPlugin::build` in `src/plugins/party/mod.rs`, not in `main.rs`.

- **All 12 component types derive `Serialize + Deserialize` from the start.** Per research §Pitfall 5. This is non-negotiable scope, NOT a "nice to have." The full derive set per component is: `Component, Reflect, Serialize, Deserialize, Default, Debug, Clone, PartialEq` (plus `Copy + Eq + Hash` where the inner type allows). `Resource`-typed values (`PartySize`) get the same derive set minus `Component`/`Reflect` plus `Resource`.

- **`derive_stats` is PURE.** No `Mut<T>`, no `Query<...>`, no `Res<...>`, no entity lookups, no asset access, no randomness. Inputs: `&BaseStats, &[ItemStatBlock], &StatusEffects, level: u32`. Output: `DerivedStats` by value. The caller flattens `Equipment` → `&[ItemStatBlock]` via `Assets<ItemAsset>` lookup (no caller exists in #11 — that's #14/#15 work). DO NOT put `&Equipment + &Assets<ItemAsset>` in the signature.

- **`Equipment = Option<Handle<ItemAsset>>` per slot — NOT `Option<Entity>`.** Per Decision 3. If implementer feels they need `Entity` references for equipment, STOP and surface — adopting `Entity` adds a `MapEntities` impl + a save-round-trip integration test that #11 does not budget for.

- **Race = Human only at runtime; declare 5 enum variants.** Per Decision 2. Variants `Human, Elf, Dwarf, Gnome, Hobbit` are all declared in the `Race` enum. Only `Race::Human` is used by `spawn_default_debug_party`. The four unused variants will trip `clippy::enum_variant_names` only if their names share a prefix; declare them as a comment explaining the v1-vs-future-feature split.

- **Class = 3 classes (Fighter/Mage/Priest) at runtime; declare all 8 enum variants.** Per Decision 1. Variants `Fighter, Mage, Priest, Thief, Bishop, Samurai, Lord, Ninja` are all declared in the `Class` enum. Only `Fighter`, `Mage`, and `Priest` have `ClassDef` entries in `core.classes.ron`. The five unused variants will not trigger lint warnings as long as `derive_stats` and `core.classes.ron` parsing don't `match` exhaustively against `Class` without a wildcard arm. Document the split in the enum doc-comment.

- **Debug-party gate is `#[cfg(feature = "dev")]`, NOT `cfg(debug_assertions)`.** Per Decision 5a. Roadmap line 629 says `cfg(debug_assertions)` — IGNORE the roadmap on this point. Project precedent: `cycle_game_state_on_f9` at `src/plugins/state/mod.rs:62` uses `#[cfg(feature = "dev")]`. Match that exactly, both for the system function definition AND the `app.add_systems(...)` call site (per the symmetric-gating pattern in `project_druum_minimap.md`).

- **Debug-party trigger is `OnEnter(GameState::Dungeon)`, NOT `OnEnter(GameState::Loading)`.** Per Decision 5b. Roadmap line 596 says `OnEnter(Loading)` — IGNORE the roadmap on this point. At `OnEnter(Loading)`, `Assets<ClassTable>` is NOT yet loaded (research §Pitfall 3). At `OnEnter(Dungeon)`, assets are guaranteed loaded by `bevy_asset_loader`'s `continue_to_state`. The system MUST also include an idempotence guard: `if !party_query.is_empty() { return; }` — F9 cycler may re-trigger `OnEnter(Dungeon)`, which would otherwise spawn duplicates.

- **`PartySize::default() == 4` and is a HARD CAP.** Per Decision 6a + 6b. `spawn_default_debug_party` MUST refuse to spawn the (n+1)th character: `let count = party_size.0.min(4);` then `for slot in 0..count`. Future Features (#19 character creation) may mutate the resource; in v1 it is a `pub struct PartySize(pub usize)` with `Default::default() = Self(4)`.

- **`Dead` is a variant of `StatusEffectType`, NOT a separate marker component.** Per Decision 7. The 5 v1 variants are `Poison, Sleep, Paralysis, Stone, Dead`. `derive_stats` zeros `max_hp` and `max_mp` when `Dead` is present in `StatusEffects`. No separate `Dead` component; Combat (#15) checks `status.has(StatusEffectType::Dead)`.

- **No magnitude-modifying status effects in v1.** Per Decision 7. The v1 `StatusEffectType` variants are all gates or tick-on-turn (Poison/Sleep/Paralysis/Stone/Dead) — none modify a stat with a `magnitude` value. The `ActiveEffect.magnitude: f32` field is declared (it's part of the schema for #15 buffs like AttackUp), but `derive_stats` only reads it for branches that don't exist in v1. The "status order independence" test from research line 583 (which uses `AttackUp`) is **deferred to #15**; #11 ships a comment in `derive_stats` documenting that v1 statuses are order-independent trivially because none of them modify stats.

- **`classes.ron` schema is deterministic per-level growth.** Per Decision 8. NO `rand` crate (Δ deps = 0). Field shape: `id, display_name, starting_stats, growth_per_level, hp_per_level, mp_per_level, xp_to_level_2, xp_curve_factor`. `starting_equipment` is omitted (no items exist yet — Feature #12).

- **`bevy::utils::HashMap` is REMOVED in 0.18.** Use `std::collections::HashMap` if needed. (Per `ClassTable::get(class) -> Option<&ClassDef>` using `Vec::iter().find()`, no HashMap is needed in #11. Flag for #14 if a per-class lookup hot path emerges.)

- **`AssetEvent<T>` is a `Message` in 0.18, not an `Event`.** Per research §Pitfall 7. Not used by #11 (no hot-reload of stats), but flag for #12/#19 implementers when they wire balance hot-reload.

- **Single production file `src/plugins/party/character.rs`.** Per Decision 4. Do NOT pre-create `inventory.rs` or `progression.rs` despite roadmap line 609. Project precedent (#9, #10) is to NOT pre-split. If `character.rs` exceeds 800 LOC, extract a `tests.rs` sibling FIRST (per #9 precedent), then revisit `inventory.rs` / `progression.rs` only when their first system arrives in #12 / #14.

- **NEW files inherit the frozen-file convention.** `src/plugins/party/character.rs`, `src/data/classes.rs` (extension of existing stub), and `src/data/items.rs` (extension of existing stub) should be authored carefully on Step 1. After the implementation lands, treat them with the same care as `src/data/dungeon.rs`. Subsequent features needing schema extensions require a fresh research/planning round, not in-passing edits.

- **`assets/classes/core.classes.ron` path is fixed by Feature #3's loader.** The roadmap says `assets/data/classes.ron`; the actual loader at `src/plugins/loading/mod.rs:37,100` says `assets/classes/core.classes.ron`. Use the existing path. `LoadingPlugin` is FROZEN post-#3 — no loader edits.

- **`src/data/classes.rs` imports from `src/plugins/party/character.rs` — one-way reverse dep, no cycle.** This is a layering inversion (data depends on plugins, where the project convention is the opposite). Research recommends it; the alternative (move `Class` and `BaseStats` to `data/`) was rejected as more disruptive (would split character types arbitrarily across two locations). Document inline in both files so a future contributor knows the inversion is intentional.

- **All 7 verification commands MUST pass with ZERO warnings:** `cargo check`, `cargo check --features dev`, `cargo clippy --all-targets -- -D warnings`, `cargo clippy --all-targets --features dev -- -D warnings`, `cargo fmt --check`, `cargo test`, `cargo test --features dev`. Final `git diff --stat` MUST show ONLY: `src/plugins/party/mod.rs` (replaced), `src/plugins/party/character.rs` (new), `src/data/classes.rs` (replaced), `src/data/items.rs` (extended), `src/data/mod.rs` (re-exports added), `assets/classes/core.classes.ron` (replaced), `tests/class_table_loads.rs` (new). NO other files touched.

- **Atomic commits.** Each step in the plan corresponds to one commit, except where two steps logically combine (noted inline). Each commit MUST compile (`cargo check && cargo check --features dev`), pass clippy with -D warnings, and pass `cargo test` and `cargo test --features dev`. If a step breaks the build, fix before committing.

## Steps

> **Pre-pipeline action (run BEFORE branching):**
>
> ```bash
> git fetch origin
> git log main..origin/main --oneline   # expect to find PR #10's merge commit
> git checkout main
> git pull origin main                   # ensure local main is at PR #10 (sha 5f55069 per pipeline state)
> but branch new ja-feature-11-party-character-ecs-model
> ```
>
> Verify `but status` shows the new branch as the applied stack head before proceeding.

### Step 0: Baseline measurement

- [ ] `grep -rc '#\[test\]' src/ | grep -v ':0$' | sort` — record per-file lib test count. Sum and record as `BASELINE_LIB`.
- [ ] `cargo test 2>&1 | tail -20` — record actual `cargo test` total (lib + integration). Expect `BASELINE_LIB` lib tests + 3 integration tests.
- [ ] `cargo test --features dev 2>&1 | tail -20` — record. Expect `BASELINE_LIB + 1` lib tests (the dev-only F9 test) + 3 integration tests.
- [ ] Record both in this plan's **Implementation Discoveries** section. Final Verification asserts the new totals against these baselines.
- [ ] `git diff --stat HEAD~5..HEAD` — confirm last 5 commits are PR #10 merge + housekeeping; no in-flight changes leak into this branch.
- [ ] **Verification:** `cargo check` and `cargo check --features dev` both pass with zero warnings on the un-touched branch.

**Commit message:** N/A (no code changes; baseline only).

### Step 1: Add `ItemAsset` and `ItemStatBlock` stubs to `src/data/items.rs`

**Files touched:** `src/data/items.rs`.

- [ ] Read current contents (12 lines, contains `ItemDb` stub).
- [ ] Add `ItemAsset` struct alongside `ItemDb`, with derive set `Asset, Reflect, Serialize, Deserialize, Default, Debug, Clone`. Body: `pub stats: ItemStatBlock` (the per-item stat contribution flattened by callers of `derive_stats`).
- [ ] Add `ItemStatBlock` struct with derive set `Reflect, Serialize, Deserialize, Default, Debug, Clone, Copy, PartialEq, Eq`. Fields (all `u32`, all `#[serde(default)]`): `attack`, `defense`, `magic_attack`, `magic_defense`, `accuracy`, `evasion`, `hp_bonus`, `mp_bonus`. Doc-comment notes "v1 schema; #12 fleshes out per-item state, #14 may add more fields".
- [ ] Add `#[cfg(test)] mod tests` with one Layer 1 round-trip test: `item_stat_block_round_trips_through_ron` — build a non-default `ItemStatBlock`, serialize via `ron::ser::to_string_pretty`, deserialize, assert equality. Pattern from `src/data/dungeon.rs:438-455`.
- [ ] Update file-level doc-comment to mention `ItemAsset` and `ItemStatBlock` are the v1 stubs Feature #11 needs for `Handle<ItemAsset>` to resolve in `Equipment` slots.
- [ ] **Verification:** `cargo check && cargo check --features dev && cargo clippy --all-targets -- -D warnings && cargo clippy --all-targets --features dev -- -D warnings && cargo fmt --check && cargo test items::tests::item_stat_block_round_trips_through_ron`.

**Commit message:**
```
feat(data): stub ItemAsset and ItemStatBlock for Feature #11 (#11)

Equipment slots in PartyMemberBundle hold Option<Handle<ItemAsset>>;
this commit adds the empty ItemAsset asset type and the ItemStatBlock
value type that derive_stats reads. ItemAsset.stats: ItemStatBlock is
the only field for v1 — Feature #12 adds per-item enchantment, durability,
and custom-name fields.

Both types derive Serialize + Deserialize per #11's serde-from-day-one
constraint (research §Pitfall 5).

Adds one Layer 1 RON round-trip test.
```

### Step 2: Replace `src/data/classes.rs` stub with real `ClassTable` schema

**Files touched:** `src/data/classes.rs`.

- [ ] Read current contents (11 lines, empty `ClassTable` stub).
- [ ] Replace with full `ClassTable` definition per research line 875-903. Imports: `bevy::prelude::*`, `serde::{Deserialize, Serialize}`, `crate::plugins::party::character::{BaseStats, Class}`. Note the reverse-dep import inline in a comment ("Reverse dep: data/ imports from plugins/. See Critical-section note in #11 plan.").
- [ ] `ClassTable` struct: derives `Asset, Reflect, Serialize, Deserialize, Default, Debug, Clone, PartialEq`. Body: `pub classes: Vec<ClassDef>`.
- [ ] Add inherent method `pub fn get(&self, class: Class) -> Option<&ClassDef>` using `self.classes.iter().find(|c| c.id == class)`. NO `HashMap` — the 8-class roster is small and `Vec::iter().find()` is O(n=8), trivial. Doc-comment notes the linear-search choice and refers to `bevy::utils::HashMap` removal in 0.18 (research §Pitfall 4).
- [ ] `ClassDef` struct: derives `Reflect, Serialize, Deserialize, Default, Debug, Clone, PartialEq`. Fields: `pub id: Class`, `pub display_name: String`, `pub starting_stats: BaseStats`, `pub growth_per_level: BaseStats`, `pub hp_per_level: u32`, `pub mp_per_level: u32`, `pub xp_to_level_2: u64`, `pub xp_curve_factor: f32`.
- [ ] Update file-level doc-comment: "FROZEN-from-day-one. Feature #11 fleshed out the schema; subsequent features should not edit in passing — schema changes require their own research + plan round."
- [ ] Add `#[cfg(test)] mod tests` with two Layer 1 tests:
  - `class_table_round_trips_through_ron` — build a `ClassTable` with one `ClassDef` (Fighter), round-trip via ron 0.12, assert equality.
  - `class_table_get_returns_authored_class` — build a `ClassTable` with `Fighter` + `Mage`, assert `get(Class::Fighter).is_some()`, `get(Class::Priest).is_none()` (declared variant, no `ClassDef`).
- [ ] **Note for Step 3:** This file does NOT compile yet — `Class` and `BaseStats` are not yet defined in `character.rs`. Stage the change but do not commit until Step 3 lands. Steps 2 and 3 are committed together as a single atomic commit (the schema and the value types are mutually dependent).

**Commit message:** Combined with Step 3 (see Step 3 below).

### Step 3: Author `src/plugins/party/character.rs` (the 12 components + Bundle + `derive_stats`)

**Files touched:** `src/plugins/party/character.rs` (NEW).

- [ ] Create the file. Imports: `bevy::prelude::*`, `serde::{Deserialize, Serialize}`, `crate::data::items::{ItemAsset, ItemStatBlock}`. NO HashMap import (status effects use `Vec`).
- [ ] File header doc-comment: cite research source (`research/20260504-160000-feature-11-party-character-ecs-model.md`) and master research §Pattern 3. Note "single-file by Decision 4 (matches #9/#10 precedent); do NOT pre-split into inventory.rs / progression.rs".

**3a. Identity components (CharacterName, Race, Class, PartyMember marker)**

- [ ] `CharacterName(pub String)` — derives `Component, Reflect, Serialize, Deserialize, Default, Debug, Clone, PartialEq, Eq, Hash`.
- [ ] `Race` enum — derives `Component, Reflect, Serialize, Deserialize, Default, Debug, Clone, Copy, PartialEq, Eq, Hash`. Variants: `#[default] Human, Elf, Dwarf, Gnome, Hobbit`. Doc-comment: "Per Decision 2: 5 variants declared, only Human used in v1 by spawn_default_debug_party. Locks discriminant order for save-format stability (research §Pitfall 5)."
- [ ] `Class` enum — same derive set as `Race`. Variants: `#[default] Fighter, Mage, Priest, Thief, Bishop, Samurai, Lord, Ninja`. Doc-comment: "Per Decision 1: 8 variants declared, only Fighter/Mage/Priest have ClassDef entries in core.classes.ron in v1. derive_stats and ClassTable::get must use wildcard arms or Option returns to handle the unauthored five — never exhaustive `match` against Class without a wildcard."
- [ ] `PartyMember` zero-sized marker — derives `Component, Reflect, Serialize, Deserialize, Default, Debug, Clone, Copy, PartialEq, Eq, Hash`. Doc-comment: "Distinguishes party characters from NPCs (#18) and enemies (#15) which may share CharacterName, BaseStats, etc. Same pattern as `PlayerParty` in `src/plugins/dungeon/mod.rs`, `DungeonGeometry`, `Torch`."

**3b. Stats components (BaseStats, DerivedStats, Experience)**

- [ ] `BaseStats` struct — derives `Component, Reflect, Serialize, Deserialize, Default, Debug, Clone, Copy, PartialEq, Eq, Hash`. Fields (all `u16`): `strength, intelligence, piety, vitality, agility, luck`. Doc-comment with each field's role per research lines 162-169. Add `pub const ZERO: Self` constant for tests.
- [ ] `DerivedStats` struct — same derive set as `BaseStats` minus `Hash` (the float-derived fields would force Hash plumbing; current shape is all u32 so technically Hash works, BUT the values are computed from f32 in `derive_stats` for status modifiers and could grow to f32 in #15 — leave Hash off for forward stability). Fields per research lines 180-191: `max_hp, current_hp, max_mp, current_mp, attack, defense, magic_attack, magic_defense, speed, accuracy, evasion` — all `u32`.
- [ ] `Experience` struct — derives `Component, Reflect, Serialize, Deserialize, Default, Debug, Clone, Copy, PartialEq, Eq, Hash`. Fields: `pub level: u32`, `pub current_xp: u64`, `pub xp_to_next_level: u64`. Doc-comment notes the cache-vs-recompute decision (cached per OQ2; pure `xp_for_level` helper deferred to #14).

**3c. Position components (PartyRow, PartySlot)**

- [ ] `PartyRow` enum — derives `Component, Reflect, Serialize, Deserialize, Default, Debug, Clone, Copy, PartialEq, Eq, Hash`. Variants: `#[default] Front, Back`. Doc-comment notes Wizardry/Etrian convention (front row = melee + ranged target, back row = caster + reduced melee damage taken).
- [ ] `PartySlot(pub usize)` — same derive set. Doc-comment: "0..PartySize.0. Slot can change (formation reorder, #19); marker (PartyMember) is invariant."

**3d. Equipment component**

- [ ] `Equipment` struct — derives `Component, Reflect, Serialize, Deserialize, Default, Debug, Clone, PartialEq, Eq`. (No Hash — `Handle<T>` doesn't impl Hash by default.) Fields (all `Option<Handle<ItemAsset>>`): `weapon, armor, shield, helm, gloves, boots, accessory_1, accessory_2`. Doc-comment per Decision 3: "Handle<ItemAsset>, NOT Entity — keeps derive_stats pure and skips MapEntities for save/load (#23). Per-instance state (enchantment, durability, custom name) lands in #12 as a separate `ItemInstance` entity model."

**3e. Status effect components (StatusEffects, ActiveEffect, StatusEffectType)**

- [ ] `StatusEffectType` enum — derives `Reflect, Serialize, Deserialize, Default, Debug, Clone, Copy, PartialEq, Eq, Hash`. Variants: `#[default] Poison, Sleep, Paralysis, Stone, Dead`. Doc-comment per Decision 7: "v1 negative set. Buffs (AttackUp, DefenseUp, etc.) deferred to #15. `Dead` is a variant here (NOT a separate marker component) — derive_stats branches on it; combat (#15) checks `status.has(StatusEffectType::Dead)`."
- [ ] `ActiveEffect` struct — derives `Reflect, Serialize, Deserialize, Default, Debug, Clone, Copy, PartialEq`. (No Eq/Hash — has `f32` field.) Fields: `pub effect_type: StatusEffectType`, `pub remaining_turns: Option<u32>`, `pub magnitude: f32`. Doc-comment notes `Stone` and `Dead` are non-tickable (`remaining_turns: None` is canonical for them); `Poison` ticks per turn in #15. The `magnitude` field is part of the schema for #15 buffs (e.g., AttackUp 0.5 = +50%); v1 status types do not use it.
- [ ] `StatusEffects` struct — derives `Component, Reflect, Serialize, Deserialize, Default, Debug, Clone, PartialEq`. Body: `pub effects: Vec<ActiveEffect>`. Add inherent method `pub fn has(&self, kind: StatusEffectType) -> bool` using `self.effects.iter().any(|e| e.effect_type == kind)`.

**3f. PartyMemberBundle**

- [ ] `PartyMemberBundle` struct — derives `Bundle, Default`. Fields per research line 308-321: `marker: PartyMember, name: CharacterName, race: Race, class: Class, base_stats: BaseStats, derived_stats: DerivedStats, experience: Experience, party_row: PartyRow, party_slot: PartySlot, equipment: Equipment, status_effects: StatusEffects`. (No `Reflect`/`Serialize`/`Deserialize` on Bundle — it's a spawn helper, not stored.)

**3g. `derive_stats` pure function**

- [ ] Signature: `pub fn derive_stats(base: &BaseStats, equip_stats: &[ItemStatBlock], status: &StatusEffects, level: u32) -> DerivedStats`. Doc-comment: "PURE — no Mut<T>, no entity lookups, no resource reads, no randomness. Caller is responsible for flattening Equipment + Assets<ItemAsset> into &[ItemStatBlock]; this keeps derive_stats testable without asset access."
- [ ] Body per research lines 213-284, with v1 simplifications:
  - HP/MP from base VIT/PIE/INT scaled by level (use `saturating_mul` and `saturating_add` per research §Security architectural risks: malicious classes.ron could overflow).
  - Attack/defense from base STR + equipment additive stacking.
  - Status effect post-pass: only the v1 types (Poison/Sleep/Paralysis/Stone/Dead). Document inline that v1 has no magnitude-modifying status effects so the `magnitude` field is unread; #15 will add the buff branches.
  - `Dead` zeros `max_hp` and `max_mp`.
  - Returns `current_hp = max_hp` and `current_mp = max_mp` as defaults; the caller chooses whether to clamp to old values (per OQ1 — caller-clamp pattern).
- [ ] Use `saturating_*` arithmetic for all addition/multiplication paths involving `BaseStats` or `equip_stats` fields. Per research §Security: bounds the trust boundary on `classes.ron` and `items.ron` malicious data.

**3h. Layer 1 unit tests (inline in `#[cfg(test)] mod tests`)**

- [ ] `base_stats_round_trips_through_ron` — build non-default `BaseStats`, serialize via `ron::ser::to_string_pretty`, deserialize, assert equality. (Pattern from `src/data/dungeon.rs:438-455`.)
- [ ] `derive_stats_returns_zero_for_zero_inputs` — `derive_stats(&BaseStats::ZERO, &[], &StatusEffects::default(), 1)`; assert `attack == 0`, `max_hp` matches the level baseline (per `derive_stats` body), `max_mp` matches the level baseline.
- [ ] `derive_stats_equipment_stacks_additively` — two `ItemStatBlock` (sword: attack 10, armor: defense 5); assert resulting attack == 10, defense == 5.
- [ ] `derive_stats_dead_zeros_pools` — `StatusEffects { effects: vec![ActiveEffect { effect_type: StatusEffectType::Dead, ..default() }] }`; assert `max_hp == 0`, `max_mp == 0`.
- [ ] `derive_stats_poison_does_not_modify_stats_at_derive_time` — Poison effect present; assert `max_hp` and `attack` match the no-status baseline (Poison ticks happen in #15; derive_stats is a no-op for Poison).
- [ ] `derive_stats_saturating_arithmetic` — `BaseStats { strength: u16::MAX, .. }` + `ItemStatBlock { attack: u32::MAX, .. }`; assert no panic; `attack == u32::MAX` (saturated).
- [ ] `party_size_default_is_four` — `assert_eq!(PartySize::default().0, 4)`.
- [ ] `status_effects_has_returns_true_for_present_kind` — `StatusEffects` with one `Dead` effect; `has(StatusEffectType::Dead)` is true; `has(StatusEffectType::Poison)` is false.
- [ ] **NOTE:** Defer the `derive_stats_status_order_independent` test (research line 583) to #15 — v1 statuses are trivially order-independent because none modify stats. Document inline in the tests module.

**3i. `PartySize` resource (declared in `character.rs`)**

- [ ] `PartySize(pub usize)` struct — derives `Resource, Reflect, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash`. Add `impl Default { fn default() -> Self { Self(4) } }`. Doc-comment per Decision 6: "Hard cap at 4 in v1. spawn_default_debug_party refuses the (n+1)th spawn. #19 character creation may mutate this resource per game-start."

- [ ] **Verification:** `cargo check && cargo check --features dev && cargo clippy --all-targets -- -D warnings && cargo clippy --all-targets --features dev -- -D warnings && cargo fmt --check && cargo test plugins::party::character::tests`.
- [ ] Verify all 8 Layer 1 tests pass.

**Combined commit message (Steps 2+3):**
```
feat(party): introduce 12 character components + ClassTable schema (#11)

Adds the character data layer that every later feature reads from:

- 12 components (CharacterName, Race, Class, BaseStats, DerivedStats,
  Experience, PartyRow, PartySlot, Equipment, StatusEffects, ActiveEffect,
  StatusEffectType) plus PartyMember marker.
- PartyMemberBundle (Bevy Bundle for spawning).
- derive_stats(&BaseStats, &[ItemStatBlock], &StatusEffects, level) ->
  DerivedStats — PURE function, no world access, saturating arithmetic.
- PartySize: Resource (hard cap, default 4).
- ClassTable + ClassDef schema in src/data/classes.rs (replaces #3 stub),
  with deterministic per-level growth (no rand crate, Δ deps = 0).

All 12 components derive Serialize + Deserialize from the start per
research §Pitfall 5; Feature #23 (save/load) is the trust-boundary
consumer of these derives.

Decisions baked in (all "A" defaults from the 8-question Category C
table):
- D1: 3 classes (Fighter/Mage/Priest), 8 enum variants declared.
- D2: 5 races declared, only Human used.
- D3: Equipment = Option<Handle<ItemAsset>> (NOT Entity).
- D4: single character.rs (NOT inventory.rs/progression.rs split).
- D6: PartySize = 4, hard cap.
- D7: 5 status types (Poison/Sleep/Paralysis/Stone/Dead); Dead is a
  variant, not a marker component.
- D8: deterministic per-level growth schema.

src/data/classes.rs imports from src/plugins/party/character.rs — a
one-way reverse dep (data/ depends on plugins/). Documented inline.

Adds 8 Layer 1 unit tests in character.rs::tests + 2 in classes.rs::tests.
```

### Step 4: Replace `src/plugins/party/mod.rs` stub with `PartyPlugin` body

**Files touched:** `src/plugins/party/mod.rs`.

- [ ] Read current contents (10 lines, empty stub).
- [ ] Replace with new `mod.rs`:
  - `pub mod character;` declaration.
  - Re-exports for the 12 component types + `PartyMember` marker + `PartyMemberBundle` + `PartySize` + `derive_stats`. Pattern: `pub use character::{ActiveEffect, BaseStats, CharacterName, Class, DerivedStats, Equipment, Experience, PartyMember, PartyMemberBundle, PartyRow, PartySlot, PartySize, Race, StatusEffectType, StatusEffects, derive_stats};`.
  - `PartyPlugin` struct with `Plugin` impl.
  - `PartyPlugin::build`:
    - Always registered: `app.init_resource::<PartySize>()`. Doc-comment: "PartySize defaults to 4 (hard cap)."
    - Always registered: `app.register_type::<...>()` calls for every Reflect-deriving type so they appear in editor / debug tooling. (Component types: `CharacterName`, `Race`, `Class`, `BaseStats`, `DerivedStats`, `Experience`, `PartyRow`, `PartySlot`, `Equipment`, `StatusEffects`, `PartyMember`. Value types: `ActiveEffect`, `StatusEffectType`. Resource: `PartySize`.)
    - `#[cfg(feature = "dev")]` block: `app.add_systems(OnEnter(GameState::Dungeon), spawn_default_debug_party);` — gated symmetrically.
- [ ] Define `spawn_default_debug_party` system per research lines 515-541, with the idempotence guard: signature `fn spawn_default_debug_party(mut commands: Commands, party_size: Res<PartySize>, existing: Query<(), With<PartyMember>>)`. Body checks `if !existing.is_empty() { return; }` first; logs `info!("Skipping debug party spawn: {} party members already exist", existing.iter().count())` if non-empty (note: use `iter().count()` not `len()`, query may not impl `len` here). Then proceeds with the for-loop, capping at `party_size.0.min(4)`. Function gated `#[cfg(feature = "dev")]`.
- [ ] Top of file imports: `use bevy::prelude::*;`, `use crate::plugins::state::GameState;` (NEW import — confirm `state::GameState` is `pub`), and the character submodule re-exports.
- [ ] Module-level doc-comment: "Party management plugin — character data, bundle, party-size resource, dev-only debug-party spawn. Inventory and progression systems land in #12 / #14."
- [ ] **Verification:**
  - `cargo check` — confirms default-feature build compiles.
  - `cargo check --features dev` — confirms dev-feature build compiles (registers `spawn_default_debug_party`).
  - `cargo clippy --all-targets -- -D warnings` — confirms clippy clean default.
  - `cargo clippy --all-targets --features dev -- -D warnings` — confirms clippy clean dev.
  - `cargo fmt --check` — confirms formatting.
  - `cargo test` — confirms all tests pass default.
  - `cargo test --features dev` — confirms all tests pass dev.

**Commit message:**
```
feat(party): wire PartyPlugin to register PartySize + dev-only spawn (#11)

PartyPlugin (already in main.rs:32 as an empty stub) now:
- init_resource::<PartySize>() — default 4, hard cap.
- register_type::<...>() for all Reflect-deriving party types.
- Under #[cfg(feature = "dev")]: registers spawn_default_debug_party
  on OnEnter(GameState::Dungeon).

spawn_default_debug_party spawns 4 hardcoded characters
(Aldric/Mira/Father Gren/Borin = Fighter/Mage/Priest/Fighter, Human,
front/front/back/back rows). Includes idempotence guard
(`if !existing.is_empty() { return; }`) so F9 cycler re-entry to
Dungeon does not duplicate.

Per Decision 5: gate is feature = "dev" (NOT debug_assertions),
trigger is OnEnter(Dungeon) (NOT OnEnter(Loading)) so assets are
guaranteed loaded.

src/main.rs is byte-unchanged — PartyPlugin is already in the
add_plugins tuple.
```

### Step 5: Update `src/data/mod.rs` re-exports

**Files touched:** `src/data/mod.rs`.

- [ ] Read current contents (24 lines).
- [ ] Add to the existing `pub use classes::ClassTable;` line: `pub use classes::{ClassDef, ClassTable};`.
- [ ] Replace the existing `pub use items::ItemDb;` line with: `pub use items::{ItemAsset, ItemDb, ItemStatBlock};`.
- [ ] Update the inline doc-comment for `items` from "items — `ItemDb` (Features #11/#12)" to "items — `ItemDb`, `ItemAsset`, `ItemStatBlock` (Features #11/#12)".
- [ ] Update the inline doc-comment for `classes` from "classes — `ClassTable` (Feature #19)" to "classes — `ClassTable`, `ClassDef` (Feature #11)".
- [ ] **Verification:** `cargo check && cargo check --features dev && cargo clippy --all-targets -- -D warnings && cargo clippy --all-targets --features dev -- -D warnings && cargo fmt --check && cargo test`.

**Commit message:**
```
chore(data): re-export ClassDef, ItemAsset, ItemStatBlock for #11 (#11)

Feature #11's character data layer reads `ClassDef` (from `ClassTable`)
and `ItemStatBlock` (from `ItemAsset`) at the crate root via
`crate::data::*` paths. Add re-exports so callers don't reach into
submodule paths.
```

### Step 6: Author `assets/classes/core.classes.ron` with 3-class data

**Files touched:** `assets/classes/core.classes.ron`.

- [ ] Read current contents (3 lines, `()` stub).
- [ ] Replace entire file with the 3-class authored RON per research lines 909-942. Format: outer `(classes: [(...), (...), (...)])`. Each entry has `id, display_name, starting_stats, growth_per_level, hp_per_level, mp_per_level, xp_to_level_2, xp_curve_factor`.
- [ ] Authored values (matches research):
  - **Fighter:** STR 14 / INT 8 / PIE 8 / VIT 14 / AGI 10 / LUK 9; growth STR+2/VIT+2/AGI+1; hp/level 8; mp/level 0; xp_to_2 100; curve 1.5.
  - **Mage:** STR 7 / INT 14 / PIE 7 / VIT 8 / AGI 10 / LUK 10; growth INT+2/VIT+1/AGI+1/LUK+1; hp/level 4; mp/level 6; xp_to_2 100; curve 1.5.
  - **Priest:** STR 9 / INT 8 / PIE 14 / VIT 11 / AGI 9 / LUK 9; growth STR+1/PIE+2/VIT+1/LUK+1; hp/level 6; mp/level 5; xp_to_2 100; curve 1.5.
- [ ] Add a header comment at the top of the file: `// Feature #11 v1 class roster: Fighter / Mage / Priest only. Five classes (Thief, Bishop, Samurai, Lord, Ninja) declared in the Class enum but NOT authored here — Decision 1.`
- [ ] **Verification:**
  - `cargo test` — confirms `class_table_round_trips_through_ron` and `class_table_get_returns_authored_class` from Step 2 still pass (they don't read this file directly; `cargo test --test class_table_loads` in Step 7 will).
  - `cargo check && cargo check --features dev && cargo clippy --all-targets -- -D warnings && cargo clippy --all-targets --features dev -- -D warnings && cargo fmt --check`.

**Commit message:**
```
feat(assets): author core.classes.ron with 3 classes (#11)

Replaces the Feature #3 () stub with deterministic per-level growth
data for Fighter / Mage / Priest per research §Decision 8.

Five additional Class enum variants (Thief / Bishop / Samurai / Lord /
Ninja) are declared in src/plugins/party/character.rs but NOT authored
here — Decision 1 ships 3 classes for v1 to avoid combinatorial balance
work (research §Pitfall 6). They will be added in a future content
pass.

Field shape: id, display_name, starting_stats, growth_per_level,
hp_per_level, mp_per_level, xp_to_level_2, xp_curve_factor. No
starting_equipment field — items don't exist yet (Feature #12).
```

### Step 7: Add `tests/class_table_loads.rs` integration test

**Files touched:** `tests/class_table_loads.rs` (NEW).

- [ ] Create the file. Mirror `tests/dungeon_floor_loads.rs:1-80` exactly, substituting:
  - `DungeonFloor` → `ClassTable`.
  - `floor: Handle<DungeonFloor>` → `class_table: Handle<ClassTable>`.
  - Asset path `dungeons/floor_01.dungeon.ron` → `classes/core.classes.ron`.
  - Loader registration `RonAssetPlugin::<DungeonFloor>::new(&["dungeon.ron"])` → `RonAssetPlugin::<ClassTable>::new(&["classes.ron"])`.
- [ ] In `assert_class_table_shape`: load via `class_tables.get(&assets.class_table)`, then assert:
  - `table.classes.len() == 3` (Fighter + Mage + Priest).
  - `table.get(Class::Fighter).is_some()` with `display_name == "Fighter"` and `starting_stats.strength == 14`.
  - `table.get(Class::Mage).is_some()` with `starting_stats.intelligence == 14`.
  - `table.get(Class::Priest).is_some()` with `starting_stats.piety == 14`.
  - `table.get(Class::Thief).is_none()` (declared variant, no entry).
- [ ] Imports: `bevy::app::AppExit`, `bevy::asset::AssetPlugin`, `bevy::prelude::*`, `bevy::state::app::StatesPlugin`, `bevy_asset_loader::prelude::*`, `bevy_common_assets::ron::RonAssetPlugin`, `druum::data::ClassTable`, `druum::plugins::party::Class`. Use `MessageWriter<AppExit>` (NOT `EventWriter`) per Bevy 0.18 family rename.
- [ ] Use `exit.write(AppExit::Success)` (NOT `send`) — Bevy 0.18 Message API.
- [ ] **Verification:**
  - `cargo test --test class_table_loads` — confirms the integration test passes.
  - `cargo test` — full test suite still passes (lib + 4 integration tests now: dungeon_floor_loads, dungeon_geometry, audio_smoke if present, class_table_loads).
  - `cargo test --features dev` — confirms dev-feature suite passes.

**Commit message:**
```
test(classes): add Layer 3 integration test for ClassTable loader (#11)

Mirrors tests/dungeon_floor_loads.rs (the Feature #4 precedent for
verifying RonAssetPlugin's ron 0.11 path matches the unit-level
ron 0.12 round-trip). Loads assets/classes/core.classes.ron through
RonAssetPlugin and asserts:
- table.classes.len() == 3 (Fighter/Mage/Priest)
- get(Class::Fighter | Mage | Priest) returns Some with the authored
  starting_stats from core.classes.ron.
- get(Class::Thief) returns None (declared enum variant, not authored).

Catches loader-vs-stdlib divergence per the established pattern
(memory project_druum_dungeon_grid: "tests/ directory is the home for
App-level integration tests").
```

### Step 8: Documentation pass + Implementation Discoveries

**Files touched:** `project/plans/20260504-170000-feature-11-party-character-ecs-model.md` (this file).

- [ ] Update the plan's `## Implementation Discoveries` section with anything that surprised the implementer:
  - Was the BASELINE_LIB count what we expected (likely 75-77 lib tests pre-#11, +9-10 expected after, so 84-87 lib tests)?
  - Did the reverse-dep `data/classes.rs -> plugins/party/character.rs` cause any compilation issues?
  - Did clippy flag the unused enum variants (`Thief, Bishop, ...` and `Elf, Dwarf, ...`)? If so, what `#[allow(...)]` was added.
  - Did the saturating arithmetic in `derive_stats` cause any clippy warnings (e.g., `clippy::needless_else_block` if the saturating chain is verbose)?
  - Were there any 0.18 API surprises (e.g., `Resource` derive macro changes, `Message` vs `Event` traps)?
  - Did `cargo test --test class_table_loads` need adjustment for AppExit vs Message changes?
- [ ] Update `## Status` field from `Draft` to `Complete`.
- [ ] **Verification:** `git diff --stat` shows ONLY: `src/plugins/party/mod.rs` (replaced), `src/plugins/party/character.rs` (new), `src/data/classes.rs` (replaced), `src/data/items.rs` (extended), `src/data/mod.rs` (re-exports added), `assets/classes/core.classes.ron` (replaced), `tests/class_table_loads.rs` (new), `project/plans/20260504-170000-feature-11-party-character-ecs-model.md` (Implementation Discoveries populated). `Cargo.toml`, `Cargo.lock`, and `src/main.rs` are byte-unchanged.

**Commit message:**
```
docs: populate Feature #11 Implementation Discoveries (#11)

Records implementation surprises, baseline test counts, clippy lint
allowances, and any 0.18 API specifics encountered during the implementer
pass for future planners and reviewers.
```

### Step 9: Manual smoke test (REQUIRED before declaring done)

- [ ] `cargo run --features dev` — game launches, hits TitleScreen.
- [ ] Press F9 to advance: TitleScreen → Town → Dungeon. On `OnEnter(Dungeon)`, `spawn_default_debug_party` runs.
- [ ] In a separate terminal, attach to the running process via no observable means — INSTEAD, verify via the in-game logs: scroll back through stdout for `Spawned 4 debug party members` (or whatever the `info!()` says in `spawn_default_debug_party`'s success path).
- [ ] Press F9 again to cycle out of Dungeon (→ Combat) and again back into Dungeon (... → Loading → ...). Verify the log for `Skipping debug party spawn: 4 party members already exist` (the idempotence guard message).
- [ ] In default mode (without `--features dev`): `cargo run`. Cycle to Dungeon (no F9 available without dev feature; instead, the natural game flow eventually puts the player in Dungeon). Verify NO debug-party spawn log appears (the `#[cfg(feature = "dev")]` gate is excluding the system entirely).
- [ ] Record findings (especially the idempotence-guard behavior) in `## Implementation Discoveries`.

**Verification:** Manual smoke confirms the spawn system runs once per `OnEnter(Dungeon)` under `--features dev`, the idempotence guard prevents duplicates, and the system is absent from default builds.

## Security

**Known vulnerabilities:** No known CVEs as of 2026-05-04 for any library used in #11 (`bevy 0.18.1`, `serde 1.x`, `ron 0.12`, `bevy_common_assets 0.16.0`, `bevy_asset_loader 0.26.0`). Re-verified via researcher memory; same status as in master research §Security.

**Architectural risks:**

- **Malicious save files (deferred to #23):** The `Serialize`/`Deserialize` derives shipped here are Feature #23's trust-boundary surface. DO NOT add custom `Serialize` impls — let serde derive handle it so the attack surface is bounded by serde's parser (which itself has extensive fuzzing). Note for #23 planner: when implementing save/load, pass `ron::Options::limit_depth(...)` and bound `Vec<ActiveEffect>` length on load (a crafted save with `effects: Vec` of length 1B causes OOM during deserialize).

- **Malicious `classes.ron`:** A `ClassDef` with `hp_per_level: u32::MAX` would overflow in `derive_stats` if added directly. **Mandatory mitigation:** `derive_stats` MUST use `saturating_add` and `saturating_mul` on every arithmetic path involving `BaseStats` or `equip_stats` fields (Step 3g body). Tested by `derive_stats_saturating_arithmetic` (Step 3h test).

- **Negative-magnitude status effects:** `ActiveEffect.magnitude: f32` can be negative. `magnitude: -100.0` on a (future) AttackUp effect would underflow on `as u32` cast. **v1 mitigation:** the v1 status types (Poison/Sleep/Paralysis/Stone/Dead) do not branch on `magnitude` in `derive_stats`. **Future mitigation (#15 buffs):** when adding magnitude-modifying status types, clamp `magnitude` to `[-1.0, 10.0]` in `derive_stats` or in an `ActiveEffect` constructor.

- **Status-effect amplification loop:** If a future feature schedules `derive_stats` recompute on stat change, `DerivedStats` write triggers stat-change, triggering recompute, infinite loop. **v1 mitigation:** `derive_stats` is a pure function with no scheduling. Recompute is **event-driven (one-shot)** in the callers — #14 (level-up event), #15 (combat turn end, equip-change event). DO NOT use change-detection on `DerivedStats` to trigger recompute; that's the loop trap.

**Trust boundaries:**

- **`classes.ron` and `items.ron` from disk:** assumed-developer-authored. RON parser caps recursion via stack; for #11's small schema, no explicit cap needed. If modding is supported in the future, schema-validate before loading (defer to mod-support feature when planned).
- **Save files (#23 territory):** out of scope for #11. The serde derives shipped here are #23's contract — see "Malicious save files" above.
- **No network input.** Single-player game.

## Open Questions

All five technical Open Questions from the research doc have been resolved during planning:

1. **Should `derive_stats` clamp `current_hp` to `new_max_hp`, or leave it to the caller?** — RESOLVED: caller-clamps. `derive_stats` returns `current_hp = max_hp` as a sane default; callers in #14 / #15 overwrite per their semantics (level-up = reset to max; equip-change = `min(old_current, new_max)`). Documented in `derive_stats` doc-comment.

2. **Should `Experience::xp_to_next_level` be cached or recomputed?** — RESOLVED: cached (matches research recommendation and master research line 561). A pure `xp_for_level(level: u32, curve: f32) -> u64` helper deferred to #14 (progression).

3. **Should `BaseStats` and `DerivedStats` derive `Reflect`?** — RESOLVED: yes. All 12 components derive `Reflect`. Free at runtime, unlocks `bevy_egui_inspector`-style tooling for #19 / #25.

4. **Is `ItemStatBlock` defined in #11 or #12?** — RESOLVED: in #11, in `src/data/items.rs` alongside the `ItemAsset` stub. `derive_stats` needs the type for tests; #12 fleshes out the rest of `ItemAsset` (enchantment, durability, custom name fields).

5. **Should `PartyMember` be a marker `()` component or carry the slot index?** — RESOLVED: separate `PartyMember` marker + `PartySlot(usize)`. Marker is invariant per character; slot can change (formation reorder in #19). Conflating them couples two separately-mutating concerns.

All 8 Category C decisions resolved by user as "all A" (defaults) — see plan dispatch context. None re-surfaced.

## Implementation Discoveries

[Starts empty — populate during Step 0 with baseline test counts and during Steps 1-9 with unexpected findings, wrong assumptions, API quirks, edge cases, and fixes applied.]

**BASELINE_LIB:** _____ (recorded in Step 0)

**Pre-#11 `cargo test` total:** _____ lib + 3 integration

**Pre-#11 `cargo test --features dev` total:** _____ lib + 3 integration

## Verification

After all 9 steps complete, run the full verification gate:

- [ ] **Build (default):** `cargo check` — Automatic — zero warnings.
- [ ] **Build (dev):** `cargo check --features dev` — Automatic — zero warnings.
- [ ] **Lint (default):** `cargo clippy --all-targets -- -D warnings` — Automatic — zero warnings.
- [ ] **Lint (dev):** `cargo clippy --all-targets --features dev -- -D warnings` — Automatic — zero warnings.
- [ ] **Format:** `cargo fmt --check` — Automatic — exit 0.
- [ ] **Test (default):** `cargo test` — Automatic — `BASELINE_LIB + 9` lib tests pass (8 in `character.rs::tests` + 2 in `classes.rs::tests` + 1 in `items.rs::tests` = ~11 new lib tests; allow ±1 if a test gets factored). 4 integration tests pass (existing 3 + new `class_table_loads`).
- [ ] **Test (dev):** `cargo test --features dev` — Automatic — `BASELINE_LIB + 9` lib tests pass + 1 dev-only test (the existing `f9_advances_game_state` from Feature #2). 4 integration tests pass.
- [ ] **`derive_stats` zero baseline:** `cargo test plugins::party::character::tests::derive_stats_returns_zero_for_zero_inputs` — Automatic — passes.
- [ ] **`derive_stats` equipment additivity:** `cargo test plugins::party::character::tests::derive_stats_equipment_stacks_additively` — Automatic — passes.
- [ ] **`derive_stats` Dead zeros pools:** `cargo test plugins::party::character::tests::derive_stats_dead_zeros_pools` — Automatic — passes.
- [ ] **`derive_stats` Poison no-op at derive time:** `cargo test plugins::party::character::tests::derive_stats_poison_does_not_modify_stats_at_derive_time` — Automatic — passes.
- [ ] **`derive_stats` saturating arithmetic:** `cargo test plugins::party::character::tests::derive_stats_saturating_arithmetic` — Automatic — no panic; `attack == u32::MAX`.
- [ ] **`PartySize::default()` is 4:** `cargo test plugins::party::character::tests::party_size_default_is_four` — Automatic — passes.
- [ ] **`StatusEffects::has` returns true for present kind:** `cargo test plugins::party::character::tests::status_effects_has_returns_true_for_present_kind` — Automatic — passes.
- [ ] **`BaseStats` RON round-trip:** `cargo test plugins::party::character::tests::base_stats_round_trips_through_ron` — Automatic — passes.
- [ ] **`ClassTable` RON round-trip:** `cargo test data::classes::tests::class_table_round_trips_through_ron` — Automatic — passes.
- [ ] **`ClassTable::get` returns authored class:** `cargo test data::classes::tests::class_table_get_returns_authored_class` — Automatic — passes.
- [ ] **`ItemStatBlock` RON round-trip:** `cargo test data::items::tests::item_stat_block_round_trips_through_ron` — Automatic — passes.
- [ ] **`ClassTable` integration via `RonAssetPlugin`:** `cargo test --test class_table_loads` — Automatic — single test `class_table_loads_through_ron_asset_plugin` passes; asserts 3 classes loaded with the authored shapes.
- [ ] **`Cargo.toml` byte-unchanged:** `git diff Cargo.toml` — Automatic — empty output.
- [ ] **`Cargo.lock` byte-unchanged:** `git diff Cargo.lock` — Automatic — empty output.
- [ ] **`src/main.rs` byte-unchanged:** `git diff src/main.rs` — Automatic — empty output.
- [ ] **Files touched (final `git diff --stat`):** Manual — confirm ONLY these files changed: `src/plugins/party/mod.rs` (replaced), `src/plugins/party/character.rs` (new), `src/data/classes.rs` (replaced), `src/data/items.rs` (extended), `src/data/mod.rs` (re-exports added), `assets/classes/core.classes.ron` (replaced), `tests/class_table_loads.rs` (new), and `project/plans/20260504-170000-feature-11-party-character-ecs-model.md` (Implementation Discoveries). NO other files touched.
- [ ] **Manual smoke (Step 9):** Manual — `cargo run --features dev`, F9 to Dungeon, verify spawn log appears once. F9 cycle out and back, verify idempotence-guard log appears (no second spawn). `cargo run` (default), verify spawn log does NOT appear.
- [ ] **GitButler ship workflow:** Manual — `but status` shows the branch with the correct commits; `but push -u origin ja-feature-11-party-character-ecs-model`; `gh pr create --base main --title "feat: Party & Character ECS Model (Feature #11)" --body "<plan link + summary>"`. CI (if present) green.
