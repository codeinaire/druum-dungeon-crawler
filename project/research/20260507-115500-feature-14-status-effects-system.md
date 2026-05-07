# Feature #14 — Status Effects System — Research

**Researched:** 2026-05-07
**Domain:** Druum / Bevy 0.18.1 / DRPG status-effect resolution layer
**Confidence:** HIGH on every "what already exists" claim (each cited at file:line, all from the merged tree). HIGH on the canonical Bevy 0.18 message/system patterns reused below (every shape is mirrored from a working precedent — `MovedEvent` / `EquipmentChangedEvent` / `recompute_derived_stats_on_equipment_change` / `apply_poison_trap`). HIGH on the recommended event signature, module placement, tick-trigger shape, and stacking semantics. MEDIUM on the per-tick poison-damage policy (D7) — the genre permits two reasonable defaults; both are plausible, and the user should ratify. MEDIUM on the dungeon-tick frequency (D9) — Wizardry/Etrian-canonical is "every step" but the project may prefer "every Nth step" for less brutal feel.

---

## Executive summary

Feature #14 lands on top of an unusually well-prepared Feature #11 foundation. **All four data types — `StatusEffectType`, `ActiveEffect`, `StatusEffects`, and `StatusEffects::has(kind)` — are already defined, registered, and battle-tested with serde+Reflect derives.** `derive_stats` already iterates `StatusEffects`, already branches on `StatusEffectType::Dead` to zero HP/MP, and explicitly notes (`character.rs:340-342`) that #14 is the natural place to add buff branches. The roadmap's spec line 780 — *"same effect refreshes duration; potency takes the higher value"* — survives stress-testing and should be adopted verbatim, with two minor refinements (a) for permanent-cure effects (`Stone`/`Dead`) re-application is a no-op, (b) for buffs the `magnitude` field is what the recommendation calls "potency". The roadmap's spec line 785 — *"treat the `Vec<ActiveEffect>` as opaque for serialization"* — is **already true**: `StatusEffects` derives `Serialize` + `Deserialize` + `Reflect` at `character.rs:264`. The save plugin (`src/plugins/save/mod.rs`) is empty and lands in #23 — **#14 has zero save-plugin work**.

The single canonical `ApplyStatusEvent` (Bevy 0.18 family rename: this is a `Message`, NOT an `Event`) is the central pivot. Recommended type signature is `ApplyStatusEvent { target: Entity, effect: StatusEffectType, potency: f32, duration: Option<u32> }` — exactly the roadmap's spec, with `Option<u32>` for the permanent-cure case (`Stone`, `Dead`) which is the same shape `ActiveEffect.remaining_turns` already uses (`character.rs:258`). `apply_status_handler` is the single system that mutates `StatusEffects.effects` — every other source (poison trap, future enemy spell, future item, future ailment-curing potion) writes the message instead of pushing directly. **This refactors `apply_poison_trap` (currently `features.rs:412-445`, naive push) into a one-line message write — the "D12 deferred" comment at `features.rs:412` is the precise hook.**

Module placement: **`src/plugins/combat/status_effects.rs` registered via a new `StatusEffectsPlugin` in `combat/mod.rs`** — this matches the roadmap line 760 and respects the genre concern that status effects fundamentally belong to the combat domain even though their data is shared. The recommendation is layered: `combat/status_effects.rs` owns the `ApplyStatusEvent` message, the canonical `apply_status_handler` system, the `tick_status_durations` ticker, the per-effect resolvers (`apply_poison_damage`, `apply_regen`), and the gate predicates (`is_paralyzed_or_asleep`, `is_silenced`, `is_confused`) as `pub fn` helpers that #15 will call from `turn_manager.rs`. The dungeon-step poison/regen tick fires via a thin `tick_on_dungeon_step` system in `combat/status_effects.rs` (NOT in `dungeon/`) that reads `MovedEvent` and emits a single `StatusTickEvent` message which the per-effect resolvers all subscribe to. This keeps the dungeon plugin clean and makes the #15 wire-up one new emitter line in `turn_manager.rs`.

Total scope: **+5 enum variants** (`AttackUp`, `DefenseUp`, `SpeedUp`, `Regen`, `Silence`) — defer `Blind` and `Confused` to #15 where they have systems that read them, defer `Paralysis`/`Sleep` block-action stubs as `pub fn` helpers with unit tests that #15 wires into `turn_manager`. **+1 message** (`ApplyStatusEvent`), **+1 internal message** (`StatusTickEvent`), **+1 plugin file** (`combat/status_effects.rs`), **+1 plugin** (`StatusEffectsPlugin` registered in `combat/mod.rs`), **+1 line edit to `recompute_derived_stats_on_equipment_change`** (it already takes `&StatusEffects`; #14 adds buff branches inside `derive_stats` so the existing recompute path picks them up — but a NEW small subscriber fires `EquipmentChangedEvent` for the affected character when buffs change so `DerivedStats` re-derives — see D5), **+1 refactor** to `apply_poison_trap` (replace the naive push with `ApplyStatusEvent` write), **+0 dependencies**, **+8-10 tests**, **+~440 LOC** (squarely in the roadmap's +350-500 envelope). Cargo.toml is byte-unchanged.

**Primary recommendation:** Implement #14 as a single new file `src/plugins/combat/status_effects.rs` plus a new `StatusEffectsPlugin` in `combat/mod.rs`. Add 5 enum variants (`AttackUp`, `DefenseUp`, `SpeedUp`, `Regen`, `Silence`) to `StatusEffectType`, register them in `party/mod.rs:47`, extend `derive_stats` to apply buff `magnitude` branches before the equipment additive stack closes. Add the canonical `ApplyStatusEvent` message and `apply_status_handler` system. Add the `tick_status_durations` system (decrement, remove on 0). Add `apply_poison_damage` and `apply_regen` resolvers reading a new `StatusTickEvent` message that **two** emitters publish: a #14-owned `tick_on_dungeon_step` system reading `MovedEvent`, and a future #15-owned emitter (one line in `turn_manager.rs`'s round-end). Stub `is_paralyzed`, `is_asleep`, `is_silenced`, `is_confused` as `pub fn` predicates with unit tests; #15 imports them. Refactor `apply_poison_trap` to `ApplyStatusEvent` write. **Zero new dependencies. Zero save-plugin work.**

---

## Live ground truth (the planner must mirror these)

These are the load-bearing facts from the merged code that contradict or refine the roadmap. Read these before designing anything.

### A. `StatusEffectType` v1 has 5 variants, NOT 12 — #14 must add the others

**File:** `/Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/party/character.rs:235-243`

```rust
#[derive(Reflect, Serialize, Deserialize, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StatusEffectType {
    #[default]
    Poison,
    Sleep,
    Paralysis,
    Stone,
    Dead,
}
```

The roadmap's broad-todo §14 line 778 names additional variants: **silence (block spells), blind (reduce accuracy), confused (random target), and the buff trio `AttackUp`, `DefenseUp`, `SpeedUp`, `Regen`**. The doc-comment at `character.rs:230-231` is explicit: *"Buffs (`AttackUp`, `DefenseUp`, etc.) are deferred to #15"* — but the **roadmap §14 line 779 says #14 owns them**: *"Implement the buff variants: `AttackUp`, `DefenseUp`, `SpeedUp`, `Regen` mutate `derive_stats` output."* These two statements **contradict**. The roadmap is the more recent authority and the more specific scope statement; #14 owns the buffs.

Per Decision 7 of #11 (`character.rs:228-230`), discriminant order is locked. **All new variants must be added at the END** to preserve the existing 0-4 indices for `Poison/Sleep/Paralysis/Stone/Dead`. Save-format stability: see Pitfall 5 of #11.

`magnitude: f32` on `ActiveEffect` (`character.rs:260`) is already part of the schema — the doc-comment notes it is *"Unused by v1 status types; reserved for #15 magnitude-modifying buffs"* but **#14 puts it to use** (the roadmap is the authority). For buffs, `magnitude` is the multiplier (e.g., `AttackUp 0.5` = +50%); for `Regen`, it's the per-tick HP heal as a fraction of `max_hp`; for `Poison`, **the recommendation in D7 below is for poison to also use `magnitude`** so the same field encodes potency uniformly.

### B. `ActiveEffect` shape is sufficient as-is — extend nothing

**File:** `character.rs:253-261`

```rust
#[derive(Reflect, Serialize, Deserialize, Default, Debug, Clone, Copy, PartialEq)]
pub struct ActiveEffect {
    pub effect_type: StatusEffectType,
    /// `None` for permanent/non-tickable effects (Stone, Dead).
    /// `Some(n)` for temporary effects (Poison, Sleep, Paralysis).
    pub remaining_turns: Option<u32>,
    /// Unused by v1 status types; reserved for #15 magnitude-modifying buffs.
    pub magnitude: f32,
}
```

The user's brief asked: *"is the current `{ effect_type, remaining_turns: Option<u32>, magnitude: f32 }` shape sufficient for #14's needs, or does #14 need extension (e.g., a `source` field for "apply only if not already at higher potency", or a `potency` field separate from `magnitude`)?"* **The shape is sufficient. Do NOT extend.** Reasoning:

1. **`source: Option<Entity>` is YAGNI.** No #14 system needs to know who applied a poison effect. The use case ("attribution for kill credit") only matters once #15 ships kill-attribution and XP-on-kill — and even then, the `MovedEvent` / trap source / spell caster is captured by the *caller* (the system writing `ApplyStatusEvent`), not by the persisted `ActiveEffect`. Adding a field forces save-format migration (Pitfall 5) for zero benefit in v1.
2. **`potency: f32` separate from `magnitude: f32` is YAGNI.** The roadmap's §14 line 780 stacking rule — *"potency takes the higher value"* — talks about the same field. **Treat `magnitude` as the canonical potency field.** Renaming would be a thrash; the doc comment can be updated to *"Magnitude / potency, depending on effect type."*
3. **`Hash`/`Eq` are still off because of `f32`.** Don't add them; the existing `PartialEq` is sufficient for `assert_eq!` round-trip tests.

**Sufficient as-is. Do not refactor.**

### C. `StatusEffects { effects: Vec<ActiveEffect> }` and `.has(kind)` are present — NO field changes

**File:** `character.rs:264-274`

```rust
#[derive(Component, Reflect, Serialize, Deserialize, Default, Debug, Clone, PartialEq)]
pub struct StatusEffects {
    pub effects: Vec<ActiveEffect>,
}

impl StatusEffects {
    /// Returns `true` if `kind` is currently active on this character.
    pub fn has(&self, kind: StatusEffectType) -> bool {
        self.effects.iter().any(|e| e.effect_type == kind)
    }
}
```

`PartyMemberBundle` already includes `status_effects: StatusEffects` at `character.rs:296`. Every party member spawned by #11 has it. **#14 adds nothing to this component or this bundle.** Two new helper methods are recommended on `impl StatusEffects` for #14 (see D2 below): `find(kind) -> Option<&ActiveEffect>` and `find_mut(kind) -> Option<&mut ActiveEffect>` to support the "refresh duration / take higher magnitude" merge. These are pure functions, additive, no schema impact.

### D. Type registrations already cover the v1 variants

**File:** `/Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/party/mod.rs:35-48`

```rust
app.register_type::<CharacterName>()
    .register_type::<Race>()
    .register_type::<Class>()
    .register_type::<BaseStats>()
    .register_type::<DerivedStats>()
    .register_type::<Experience>()
    .register_type::<PartyRow>()
    .register_type::<PartySlot>()
    .register_type::<Equipment>()
    .register_type::<StatusEffects>()    // <-- already registered
    .register_type::<PartyMember>()
    .register_type::<ActiveEffect>()      // <-- already registered
    .register_type::<StatusEffectType>()  // <-- already registered
    .register_type::<PartySize>();
```

`#[derive(Reflect)]` on the enum (line `character.rs:235`) and the structs (lines 253, 264) means **adding new variants to `StatusEffectType` requires zero changes here** — the existing `register_type::<StatusEffectType>()` automatically covers all variants of that enum once they're declared. Save-format stability still requires variants to be added at the end.

### E. `derive_stats` already iterates status effects — extending is a localized edit

**File:** `character.rs:343-426`

```rust
pub fn derive_stats(
    base: &BaseStats,
    equip_stats: &[ItemStatBlock],
    status: &StatusEffects,
    level: u32,
) -> DerivedStats { ... }
```

Two existing post-pass branches at `character.rs:400-411`:

```rust
// ── Status effect post-pass (v1 gates only) ────────
// V1 status types are order-independent (none modify via magnitude).
// #15 will add magnitude-modifying buff branches here.   <-- THE HOOK
if status.has(StatusEffectType::Dead) {
    max_hp = 0;
    max_mp = 0;
}
// Poison, Sleep, Paralysis, Stone: no stat modification at derive time.
```

**The hook is named for #15 in the comment, but the roadmap §14 line 779 owns the work.** The doc comment at `character.rs:340-342` reads:

> *"V1 status types (`Poison`, `Sleep`, `Paralysis`, `Stone`, `Dead`) are trivially order-independent because none of them modify a stat via the `magnitude` field — they are pure gates. #15 will add magnitude-modifying buff branches; at that point, order dependence must be re-evaluated and the deferred `derive_stats_status_order_independent` test should be written."*

**#14 (NOT #15) is the feature that adds the buff branches** and writes the order-independence test. The order-independence concern is real: `attack += attack * 0.5` then `attack += attack * 0.5` is NOT the same as a single `attack += attack * 1.0` — surface as Decision D6.

### F. `recompute_derived_stats_on_equipment_change` already passes `&StatusEffects` to `derive_stats` — but only fires on equipment change

**File:** `/Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/party/inventory.rs:421-481`

```rust
pub fn recompute_derived_stats_on_equipment_change(
    mut events: MessageReader<EquipmentChangedEvent>,
    items: Res<Assets<ItemAsset>>,
    mut characters: Query<
        (&BaseStats, &Equipment, &StatusEffects, &Experience, &mut DerivedStats),
        With<PartyMember>,
    >,
) {
    for ev in events.read() {
        let Ok((base, equip, status, xp, mut derived)) = characters.get_mut(ev.character) else { continue };
        // ... flatten equipment ...
        let new = derive_stats(base, &equip_stats, status, xp.level);
        // caller-clamp: preserve current_hp/mp under new max
        let old_current_hp = derived.current_hp;
        let old_current_mp = derived.current_mp;
        *derived = new;
        derived.current_hp = old_current_hp.min(derived.max_hp);
        derived.current_mp = old_current_mp.min(derived.max_mp);
    }
}
```

**This is a critical finding for #14.** The system reads `&StatusEffects`, calls `derive_stats(... status ...)`, and clamps. The buff-magnitude branches added in E above will work here automatically — *if* this system fires when buffs change. Today it only fires on `EquipmentChangedEvent`. **#14 must trigger a re-derive when status effects change.** Two architectural options surface as Decision D5:

- **D5α [RECOMMENDED]:** `apply_status_handler` (the central #14 system that mutates `StatusEffects`) **also writes `EquipmentChangedEvent` for the affected character** when the changed effect is one of the `derive_stats`-affecting variants (`AttackUp`/`DefenseUp`/`SpeedUp`/`Dead`). This is a one-line addition in `apply_status_handler` and reuses 100% of the existing recompute pipeline including the caller-clamp. Cost: `EquipmentChangedEvent` becomes "stats-changed event" semantically — slightly impure naming, mitigated by a doc-comment update.
- **D5β:** New `StatusEffectsChangedEvent` message + extend `recompute_derived_stats_on_equipment_change` to subscribe to BOTH events (or split it into a new shared system both call). Cleaner naming. Adds ~20 LOC, +1 message registration, +1 system or +1 reader on the existing system.

**Recommended: D5α.** Cleanest ship. The naming impurity is acceptable for v1; #23/#25 can rename later. This avoids fragmenting the recompute path.

### G. `apply_poison_trap` is the canonical refactor target — D12-deferred naive push

**File:** `/Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/dungeon/features.rs:412-445`

```rust
/// Apply poison trap on entry. Naive push (D12) — stacking deferred to #14.
fn apply_poison_trap(
    mut moved: MessageReader<MovedEvent>,
    floors: Res<Assets<DungeonFloor>>,
    dungeon_assets: Option<Res<DungeonAssets>>,
    active_floor: Res<ActiveFloorNumber>,
    mut party: Query<&mut StatusEffects, With<PartyMember>>,
    mut sfx: MessageWriter<SfxRequest>,
) {
    const POISON_TURNS: u32 = 5;
    // ... walks the floor's CellFeatures.trap looking for TrapType::Poison ...
    for mut effects in &mut party {
        effects.effects.push(ActiveEffect {
            effect_type: StatusEffectType::Poison,
            remaining_turns: Some(POISON_TURNS),
            magnitude: 0.0,
        });
    }
    // ...
}
```

The comment **`Naive push (D12) — stacking deferred to #14`** is the explicit hook. **#14 must refactor this system.** Refactor shape:

```rust
fn apply_poison_trap(
    mut moved: MessageReader<MovedEvent>,
    floors: Res<Assets<DungeonFloor>>,
    dungeon_assets: Option<Res<DungeonAssets>>,
    active_floor: Res<ActiveFloorNumber>,
    party: Query<Entity, With<PartyMember>>,         // <-- now reads only Entity
    mut apply: MessageWriter<ApplyStatusEvent>,        // <-- writes the canonical message
    mut sfx: MessageWriter<SfxRequest>,
) {
    // ... walks the floor's CellFeatures.trap looking for TrapType::Poison ...
    for entity in &party {
        apply.write(ApplyStatusEvent {
            target: entity,
            effect: StatusEffectType::Poison,
            potency: 1.0,                              // designer-tunable per-trap (D8)
            duration: Some(5),
        });
    }
    // ...
}
```

The D12 stacking comment becomes irrelevant — `apply_status_handler` enforces the merge rule once. **Loss of `&mut StatusEffects` access** in `apply_poison_trap` is a pure win — it removes a query borrow, simplifies the system signature, and lets the canonical handler enforce the policy.

This is the **one frozen-file edit #14 makes outside `combat/`**: `src/plugins/dungeon/features.rs:412-445`. The signature change is contained; the existing test `poison_trap_applies_status` at `features.rs:938-978` will continue to pass because the end-state (poison present on party members) is unchanged — but the test's wiring will need to flush a frame between the trap fire and the assertion (the message has to round-trip through `apply_status_handler`). See Pitfall 1 in `## Common Pitfalls` below.

### H. `MovedEvent { from, to, facing }` is the established subscription shape — emit on commit-frame

**File:** `/Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/dungeon/mod.rs:192-197, 847-851`

```rust
#[derive(Message, Clone, Copy, Debug)]
pub struct MovedEvent {
    pub from: GridPosition,
    pub to: GridPosition,
    pub facing: Direction,
}
// ... in handle_dungeon_input ...
moved.write(MovedEvent { from: old_pos, to: *pos, facing: facing.0 });
```

**Already published BEFORE the visual tween — same-frame consumable.** The mod-doc at `dungeon/mod.rs:30-34` is explicit:

> *"Downstream consumers (#13 cell-trigger, #16 encounter) react to the new logical state on the commit frame, not after the tween completes."*

**#14's dungeon-step tick reads this same message.** Subscriber pattern (mirroring `apply_pit_trap` / `apply_poison_trap` / minimap):

```rust
fn tick_on_dungeon_step(
    mut moved: MessageReader<MovedEvent>,
    mut tick: MessageWriter<StatusTickEvent>,
    party: Query<Entity, With<PartyMember>>,
) {
    for _ev in moved.read() {                          // one tick per step (D9α)
        for entity in &party {
            tick.write(StatusTickEvent { target: entity });
        }
    }
}
```

System ordering: **`.run_if(in_state(GameState::Dungeon)).after(handle_dungeon_input)`** — same as the seven existing `MovedEvent` consumers in `features.rs:160-187`. `pub(crate) fn handle_dungeon_input` is exposed at `dungeon/mod.rs:768` for exactly this `.after(...)` use case (the doc comment at lines 759-765 names #13 explicitly; #14 mirrors).

### I. `Combat` plugin is a stub — `combat/mod.rs` has only state-entry log lines

**File:** `/Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/combat/mod.rs:1-19`

```rust
pub struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Combat), || info!("Entered GameState::Combat"))
           .add_systems(OnExit(GameState::Combat), || info!("Exited GameState::Combat"));
    }
}
```

The directory `src/plugins/combat/` exists with only `mod.rs`. **#14 adds `src/plugins/combat/status_effects.rs`** (the roadmap names this exact path at line 760) and a new `StatusEffectsPlugin` that `combat/mod.rs` registers via `app.add_plugins(StatusEffectsPlugin)`. **`CombatPlugin` itself stays a state-stub** until #15. The `StatusEffectsPlugin` could be registered directly in `main.rs` parallel to `CombatPlugin` (matching the `CellFeaturesPlugin` precedent at `main.rs:33`), OR registered as a sub-plugin of `CombatPlugin` via `app.add_plugins(StatusEffectsPlugin)` from inside `CombatPlugin::build`. **D3 below recommends sub-plugin** — keeps `main.rs` shorter and respects the future shape where #15's `TurnManagerPlugin` will sit alongside.

### J. The save plugin is empty — #14 has zero save-plugin work

**File:** `/Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/save/mod.rs:1-9`

```rust
pub struct SavePlugin;

impl Plugin for SavePlugin {
    fn build(&self, _app: &mut App) {}
}
```

**`SavePlugin` is empty until #23.** `StatusEffects` already derives `Serialize + Deserialize + Reflect` at `character.rs:264`. The roadmap §14 line 785 says *"Treat the `Vec<ActiveEffect>` as opaque for serialization"* — **this is automatically true** because `Vec<T: Serialize + Deserialize>` ships with serde derives. No #23 carve-out, no `MapEntities` work needed for status effects (the only concern would be `source: Option<Entity>` if it were added — see B above; it should not be).

### K. `combat/turn_manager.rs` does not exist yet — #14 must NOT create it

`#15` owns `combat/turn_manager.rs` per roadmap §15 line 808. The user's brief asks: *"`block_action_if_paralyzed/asleep/silenced/confused`: combat #15 doesn't exist yet. Do these systems exist as stubs in #14, or are they purely hooks that #15 will call?"*

**Recommended: pure `pub fn` predicate helpers in `status_effects.rs`, NOT systems.** Reasoning:
- A "system" registered in `Update` schedule needs a place to run. Without a `CombatPhase::PlayerInput` schedule (deferred to #15), there's no runnable target; registering them dead-on-arrival creates noise.
- A `pub fn is_paralyzed(status: &StatusEffects) -> bool` is a **one-liner unit-testable predicate** that #14 owns and #15 imports.
- The same shape is used by `EquipSlot::read` / `EquipSlot::write` at `inventory.rs:118-149` — pure helper functions, called by Bevy systems registered elsewhere. This is the established Druum pattern (research §Pattern 6).

**#14 ships:** `pub fn is_paralyzed(s: &StatusEffects) -> bool`, `pub fn is_asleep(s: &StatusEffects) -> bool`, `pub fn is_silenced(s: &StatusEffects) -> bool`. Defer `is_blind` (no consumer until #15 adds accuracy roll) and `is_confused` (no consumer until #15 adds target selection) to #15 to avoid speculative API surface (these enum variants ship in #14 to keep save-format stable — see Decision D1 — but the predicates that read them ship with their consumer).

### L. Test patterns are established — three layers

**File:** `/Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/dungeon/features.rs:710-934` (full app_tests module)

The features.rs test pattern is the layer-2 app-driven test: `MinimalPlugins + AssetPlugin + StatesPlugin + InputPlugin + StatePlugin + DungeonPlugin + CellFeaturesPlugin + PartyPlugin`, with `init_resource::<ActionState<DungeonAction>>` as the bypass for leafwing's tick (NO `ActionsPlugin`). The pattern is documented in agent-memory `feedback_bevy_input_test_layers.md`.

**For #14 tests:**
- **Layer 1 — pure logic on `StatusEffects` / `derive_stats` / merge helpers:** No `App` needed. Same shape as `derive_stats_dead_zeros_pools` at `character.rs:514-526`. Fast, deterministic, ~10 lines per test.
- **Layer 2 — `apply_status_handler` end-to-end:** Use the `make_test_app` helper from `features.rs:736-771`. Need `MinimalPlugins + StatesPlugin + InputPlugin + StatePlugin + DungeonPlugin + CellFeaturesPlugin + PartyPlugin + StatusEffectsPlugin`, plus `app.add_message::<ApplyStatusEvent>()` if not registered by the plugin (it will be). For `tick_status_durations` tests of duration-decrement: use `TimeUpdateStrategy::ManualDuration` from agent-memory `reference_bevy_018_time_update_strategy.md` IF the ticker reads `Time::delta` — but the recommendation in D9 is for ticks to be **count-based** (one per `MovedEvent`), so `Time` is irrelevant; tests just call `app.update()` after writing a `MovedEvent`.
- **Layer 3 — refactored `apply_poison_trap`:** existing test `poison_trap_applies_status` at `features.rs:938-978` must continue to pass. **Note (Pitfall 1 below):** the test currently writes a `MovedEvent` then calls `app.update()` once. After #14's refactor, the trap writes `ApplyStatusEvent` instead of pushing directly — so the test needs **two `app.update()` calls** to flush both the trap system's write and the handler's read. This is a one-line test change.

### M. `register_type` for new variants is automatic — no `mod.rs` register adds needed

**File:** `party/mod.rs:35-48`

`StatusEffects`, `ActiveEffect`, `StatusEffectType` are already individually registered. `#[derive(Reflect)]` on the enum at `character.rs:235` covers all variants. **Adding a new variant `AttackUp` to `StatusEffectType` requires zero edits to `party/mod.rs`'s registration block.** Buff variants (`AttackUp`, `DefenseUp`, `SpeedUp`, `Regen`) and debuffs (`Silence`, etc.) are visible in inspector tooling automatically.

The new `ApplyStatusEvent` and `StatusTickEvent` messages do NOT need `register_type` (messages aren't reflected components); they ARE registered via `app.add_message::<T>()` in `StatusEffectsPlugin::build` — same shape as `MovedEvent` at `dungeon/mod.rs:224` and `EquipmentChangedEvent` at `party/mod.rs:62`.

### N. `rusty_door_01` and the v1 testbed

`assets/dungeons/floor_01.dungeon.ron` already has a `Poison` trap (per the #13 implementation, around (4,4) per the comment in `features.rs:938`). The integration test `poison_trap_applies_status` at `features.rs:938-978` exercises it. **#14 has a ready-to-use floor for end-to-end smoke testing** — walk into the poison cell, take 5 ticks of damage over 5 dungeon steps, observe the effect drop off.

For testing buffs, `Regen`, and `Silence`, the recommended pattern is **direct `ApplyStatusEvent` write in tests** (no need to author asset content). Buffs do not currently have a trap or ailment-curing item to apply them; the consumer that applies AttackUp ships in #15 (mage spell) or #20 (potion). #14's role is to **make the buffs *consumable* by `derive_stats` once #15/#20 emit them.**

---

## Stale-roadmap summary

| Roadmap claim (line) | Reality |
|----------------------|---------|
| Line 747: "Build per-status systems: `tick_status_durations` ... `apply_poison_damage` ... `block_action_if_paralyzed`" | Partly outdated. The recommendation here splits these into **2 systems + N pure-fn helpers**: `tick_status_durations` (decrement only), `apply_poison_damage` (read tick events, mutate HP) — and `is_paralyzed/is_asleep/is_silenced` as `pub fn` helpers, NOT systems. #15 registers these as systems in `turn_manager.rs` once it has a schedule to run them in. |
| Line 747: "Add an `ApplyStatusEvent { target, effect, potency, duration }`" | Mostly correct. The recommendation is exactly this with one refinement: `duration: Option<u32>` (not `u32`) so callers can express "permanent until cured" for `Stone`/`Dead`. |
| Line 760: "`src/plugins/combat/status_effects.rs` (the central status registry)" | Correct. **File doesn't exist; #14 creates it.** Module placement confirmed — see D4 below. |
| Line 761: "Cross-cutting: `combat/turn_manager.rs` (skip turn if paralyzed/asleep), `dungeon/movement.rs` (poison damage over steps)" | Stale. `combat/turn_manager.rs` does not exist (lands in #15). `dungeon/movement.rs` does not exist either — dungeon movement is in `dungeon/mod.rs:744-879` (single-file pattern). The poison-damage tick belongs in `combat/status_effects.rs`, NOT in dungeon — the ticker subscribes to `MovedEvent` from the combat side. (D4) |
| Line 776: "central handler that adds/refreshes effects" | Correct. The recommendation names this `apply_status_handler`. |
| Line 777: "`tick_status_durations` system (decrement `remaining_turns`, remove on 0)" | Correct, with one nuance: `remaining_turns: None` (permanent) variants must be SKIPPED, not decremented. The retain-filter pattern at `character.rs:271` is the model. |
| Line 778: "blind (reduce accuracy), confused (random target)" | Defer. The roadmap names them but no #14 system reads them. **Recommendation: declare `Blind` and `Confused` enum variants in #14 for save-format stability, but ship the predicates in #15.** Saves a roundtrip on save-format breaking later. |
| Line 779: "Implement the buff variants: `AttackUp`, `DefenseUp`, `SpeedUp`, `Regen` mutate `derive_stats` output." | Correct. Note: `Regen` is **not** a `derive_stats` modifier — it ticks like Poison but heals. The roadmap wording groups them but the implementation splits: `AttackUp/DefenseUp/SpeedUp` modify `derive_stats`; `Regen` is a tick resolver mirroring `apply_poison_damage`. (Important distinction the planner must encode.) |
| Line 780: "same effect refreshes duration; potency takes the higher value" | **Survives stress-testing.** Adopt verbatim with two refinements: (a) for permanent-cure effects (`Stone`/`Dead`), re-application is a no-op, (b) for buffs, `magnitude` IS potency. (D2) |
| Line 781: "Unit tests: apply poison, tick 5 rounds, verify HP and removal." | Correct. The recommendation expands this to ~10 tests (see "Test count estimate" below). |
| Line 782: "Hook poison/regen ticks to *both* combat rounds (in combat) and dungeon steps (out of combat) — same event, different triggers." | Correct, but the architecture is decision-shaped. Recommendation in D9 below: a single `StatusTickEvent` message with two emitters (dungeon-step now, combat-round later). |
| Line 785: "Treat the `Vec<ActiveEffect>` as opaque for serialization" | **Already true.** `StatusEffects` derives `Serialize + Deserialize + Reflect` at `character.rs:264`. Zero #23 carve-out. |

---

## Standard Stack

### Core (already in deps — no Δ)

| Library | Version | Purpose | License | Maintained? | Why Standard |
|---------|---------|---------|---------|-------------|--------------|
| [bevy](https://crates.io/crates/bevy) | =0.18.1 | `Component`, `Message`, `Resource`, `Plugin`, `MessageReader`, `MessageWriter` | MIT/Apache-2.0 | Active | Engine — pinned. |
| [serde](https://crates.io/crates/serde) | 1 | `Serialize`/`Deserialize` for new enum variants (auto-derived). | MIT/Apache-2.0 | Active | Already in deps. |
| [bevy_reflect](https://crates.io/crates/bevy_reflect) | (transitive via bevy) | `Reflect` derive on new enum variants — auto. | MIT/Apache-2.0 | Active | Already wired. |

### Supporting (NOT used in #14)

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| [rand](https://crates.io/crates/rand) | (transitive) | NOT needed in #14 (Confused random-target ships in #15). | #15 |
| [bevy_egui](https://crates.io/crates/bevy_egui) | =0.39.1 | Status-effect icon overlay UI (cons-of-#14 line 758). | #25 |
| [leafwing-input-manager](https://crates.io/crates/leafwing-input-manager) | =0.20.0 | NOT needed — #14 systems don't read `ActionState`. | (unused in #14) |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `apply_status_handler` writes `EquipmentChangedEvent` to trigger re-derive (D5α) | New `StatusEffectsChangedEvent` message + new system reader (D5β) | D5α reuses 100% of existing recompute pipeline; D5β has cleaner naming. **Recommended: D5α** (cleanest ship). |
| Single `tick_status_durations` system reads a `StatusTickEvent` Message published by both #15 and dungeon-step (D9α) | Two separate systems, one per trigger (D9β); OR one system listening directly to `MovedEvent` for now, future #15 patches it (D9γ) | D9α is the most decoupled — #15 wires its own emitter without touching #14's tick system. **Recommended: D9α.** |
| `pub fn is_paralyzed(s: &StatusEffects) -> bool` helpers (K above) | Real Bevy systems registered into a not-yet-existing schedule | Helpers are unit-testable without an `App`, callable from #15 with zero scheduling gymnastics. **Recommended: helpers.** |
| `apply_status_handler` runs in `Update` schedule | Run in `PostUpdate` so all message-writers in `Update` are seen this frame | `Update` matches every other gameplay system in Druum; `MovedEvent` consumers all run in `Update` `.after(handle_dungeon_input)`. Cross-frame readability is fine for status apply. **Recommended: Update.** |
| Buff `magnitude` is a multiplier (`AttackUp 0.5` = +50%) | Buff `magnitude` is an absolute additive (`AttackUp 5` = +5 attack) | Multiplier composes with existing equipment-additive arithmetic (saturating-mul-then-add). Absolute additive collides with the per-stat unit (attack is a u32, magnitude is f32 — would need rounding). **Recommended: multiplier.** |
| `Regen` heals as a fraction of `max_hp` (e.g., `magnitude = 0.05` → 5% of max per tick) | `Regen` heals a flat amount per tick (`magnitude = 5.0` → +5 HP) | Fraction-of-max scales with character power; flat amount mirrors `Poison` (D7) for consistency. **Recommended: same shape as Poison** (whatever D7 lands on, Regen mirrors it). |

---

## Architecture Options

Three architectural decisions that shape #14. All three have a strong default; the ones marked **Category C** are genuine A/B/C user picks.

### D3 (Category B): Module placement & plugin shape

| Option | Description | Pros | Cons | Best When |
|--------|-------------|------|------|-----------|
| **α: New `combat/status_effects.rs` + new `StatusEffectsPlugin` registered as sub-plugin of `CombatPlugin`** [RECOMMENDED] | New file at `src/plugins/combat/status_effects.rs`. New struct `StatusEffectsPlugin: Plugin`. `combat/mod.rs::CombatPlugin::build` adds `app.add_plugins(StatusEffectsPlugin)`. Single line in `main.rs` is unchanged — `CombatPlugin` is already registered there. | Matches the roadmap line 760 path verbatim. Keeps `main.rs` short. Keeps related systems together — `combat/` will accumulate `turn_manager.rs`, `actions.rs`, `damage.rs` in #15. The `StatusEffectsPlugin` becomes a peer of those. The doc-comment at `character.rs:230-231` ("Buffs ... are deferred to #15") is wrong on the deferral but right on the *home*: status effects belong to combat. | The dungeon-step tick subscribes to `MovedEvent` which comes from `dungeon/mod.rs`. There's a slight cross-domain coupling that the planner must justify — mitigated by the fact that the *trigger* shape (`MovedEvent`) is already the dungeon's exported public surface (used by `features.rs`, `minimap.rs`, future #16 encounters). The combat plugin reading dungeon events is symmetric and fine. | Default. The roadmap names this path; the `CellFeaturesPlugin` precedent at `main.rs:33` uses the same sub-plugin pattern. |
| β: New `party/status_effects.rs` (data lives there already) | Move logic to where the data is. | Status data IS in `party/`. Keeps everything tied to `PartyMember`. | Combat domain is the natural reader for status effects (skip-turn-if-paralyzed). Putting status-effect resolution in `party/` puts cross-domain logic (combat-round ticks in #15) in the wrong plugin. **Worse fit.** | Only if #15 ends up not registering combat-side ticks at all (unlikely). |
| γ: Top-level `plugins/status/` directory | Zero ambiguity. | Most explicit. | Yet another top-level directory; `status` is more entwined with combat than the existing peers (`audio`, `dungeon`, `party`, `town`, etc.) suggest by their split. **Overdesign for one feature.** | Only if status effects ever grow to compete in scope with combat — very unlikely in Druum's scope. |

**Recommended: α.** Default. Confidence HIGH (roadmap-stated path).

### D4 (Category B): Tick trigger architecture

| Option | Description | Pros | Cons | Best When |
|--------|-------------|------|------|-----------|
| **α: Single `StatusTickEvent` message; two emitters (dungeon-step now, combat-round in #15)** [RECOMMENDED] | New `#[derive(Message)] struct StatusTickEvent { target: Entity }`. **#14 owns the message + the dungeon-step emitter** (`tick_on_dungeon_step` reads `MovedEvent`, writes one `StatusTickEvent` per party member). **#15 owns the combat-round emitter** (one line in `turn_manager.rs::round_end`: `for entity in alive_combatants { tick.write(StatusTickEvent { target: entity }) }`). The single `tick_status_durations` system reads `StatusTickEvent` and decrements; the per-effect resolvers (`apply_poison_damage`, `apply_regen`) also read it. | Clean separation: #14's dungeon emitter never touches combat, #15's combat emitter never touches dungeon. The `tick_status_durations` system is one place, one rule. **Same primitive (Bevy `Messages<T>`) we use everywhere else.** | Slightly more LOC than option γ (one extra message type). | Default. Mirrors the existing `MovedEvent` / `TeleportRequested` / `EncounterRequested` / `EquipmentChangedEvent` pattern verbatim. |
| β: Two separate systems, one per trigger | `tick_on_dungeon_step` (decrement + per-effect) and `tick_on_combat_round` (decrement + per-effect) — each is a self-contained ticker. | Triggers are independent; no message indirection. | Duplicates the decrement logic, the per-effect resolvers, and the merge rules across two systems. **Future per-effect resolver additions (e.g., `apply_burn_damage` in a hypothetical #20) require editing both.** Worse. | Never. |
| γ: One system listens directly to `MovedEvent` for now, #15 patches it later | `tick_status_durations` reads `MovedEvent`. When #15 lands, it adds a `RoundEnd` message and refactors. | One less message type today. | Couples status logic to dungeon movement at the API level. When #15 lands, refactor surface is bigger (the "patch later" is a bigger surgery than an "add later" of an emitter). | Never (premature simplification). |

**Recommended: α.** Cleanest decoupling. Confidence HIGH.

### D9 (Category C, USER PICK): Dungeon tick frequency

| Option | Description | Genre fit | Implementation |
|--------|-------------|-----------|----------------|
| α: Every dungeon step | One tick per `MovedEvent` (translation moves only — turns and bumps don't tick). | Wizardry-canonical. | One emitter, one `for ev in moved.read()` loop. |
| β: Every Nth step (e.g., every 3 steps) | Resource counter `StepsSinceLastTick: u32` — tick when `>= N`, reset. | Less brutal; modern-cozy fit. | One emitter, +1 resource, +5 LOC. |
| γ: Time-based | Tick every X seconds of in-game time (advances regardless of player movement). | Real-time idle DRPG fit. | Reads `Time::delta`; less control for the player. Not Wizardry. |

**Recommendation:** **Ask the user.** This is a genuine UX tuning call, not a technical decision. The technical implementation is ~5 LOC different across the three. If the user has no preference, default to **α (every step)** — it's Wizardry-canonical and matches the project's overall tone (per the existing dungeon design at `dungeon/mod.rs:1-34` referencing Wizardry).

---

## Architecture Patterns

### Recommended file layout

```
src/plugins/combat/
├── mod.rs                  # CombatPlugin (existing) — add `add_plugins(StatusEffectsPlugin)`
└── status_effects.rs       # NEW — entire #14 lives here

src/plugins/party/character.rs   # +5 enum variants, +N derive_stats branches
src/plugins/party/mod.rs         # NO changes (Reflect handles new variants automatically)
src/plugins/dungeon/features.rs  # apply_poison_trap refactor (signature change + 1 fn body)
```

### Pattern 1: Canonical message shape (Bevy 0.18 family rename)

**What:** All gameplay events in Druum are `#[derive(Message)]`, NOT `#[derive(Event)]`. Read with `MessageReader<T>`, write with `MessageWriter<T>`, register with `app.add_message::<T>()`.

**When to use:** ALL #14 cross-system communication.

**Example:**
```rust
// Source: dungeon/mod.rs:192-197 (canonical project precedent)
#[derive(Message, Clone, Copy, Debug)]
pub struct ApplyStatusEvent {
    pub target: Entity,
    pub effect: StatusEffectType,
    pub potency: f32,
    pub duration: Option<u32>,
}

#[derive(Message, Clone, Copy, Debug)]
pub struct StatusTickEvent {
    pub target: Entity,
}

// In StatusEffectsPlugin::build:
app.add_message::<ApplyStatusEvent>()
   .add_message::<StatusTickEvent>();
```

### Pattern 2: System ordering with `.after(handle_dungeon_input)`

**What:** Every `MovedEvent` consumer in Druum is registered with `.run_if(in_state(GameState::Dungeon)).after(handle_dungeon_input)`.

**When to use:** `tick_on_dungeon_step` (the new #14 system that reads `MovedEvent`).

**Example:**
```rust
// Source: features.rs:160-187 (the seven existing #13 consumers)
app.add_systems(
    Update,
    (
        tick_on_dungeon_step
            .run_if(in_state(GameState::Dungeon))
            .after(handle_dungeon_input),
        tick_status_durations,             // no state gate — runs everywhere
        apply_poison_damage,
        apply_regen,
        apply_status_handler,
    ),
);
```

`tick_status_durations`, `apply_poison_damage`, `apply_regen`, and `apply_status_handler` do NOT need a state gate — they're message-driven, and no message will fire outside `Dungeon` (today) or `Combat` (#15+). They naturally idle. Keeping them ungated lets the same systems serve both states without a `OR` predicate.

### Pattern 3: Pure-fn helpers (NOT systems) for #15 hooks

**What:** Predicate functions `pub fn is_paralyzed(s: &StatusEffects) -> bool` etc., callable from #15's `turn_manager.rs::collect_player_actions` and similar.

**When to use:** Anything in roadmap §14 line 778 that says "block X if Y" — these are predicates, not systems.

**Example:**
```rust
// Same shape as `StatusEffects::has` at character.rs:269-273.
pub fn is_paralyzed(status: &StatusEffects) -> bool {
    status.has(StatusEffectType::Paralysis)
}
pub fn is_asleep(status: &StatusEffects) -> bool {
    status.has(StatusEffectType::Sleep)
}
pub fn is_silenced(status: &StatusEffects) -> bool {
    status.has(StatusEffectType::Silence)
}
// `is_blind`, `is_confused` defer to #15.
```

### Pattern 4: Stacking merge

**What:** The canonical `apply_status_handler` is the ONLY system that mutates `StatusEffects.effects`. The merge rule (D2) applies here.

**Example:**
```rust
// Sketch — actual signature in `## Code Examples` below.
fn apply_status_handler(
    mut events: MessageReader<ApplyStatusEvent>,
    mut equip_changed: MessageWriter<EquipmentChangedEvent>,  // D5α: triggers re-derive
    mut characters: Query<&mut StatusEffects, With<PartyMember>>,
) {
    for ev in events.read() {
        let Ok(mut status) = characters.get_mut(ev.target) else { continue };

        // Permanent-cure effects: re-application is a no-op.
        if matches!(ev.effect, StatusEffectType::Stone | StatusEffectType::Dead)
           && status.has(ev.effect)
        {
            continue;
        }

        // Stacking rule D2: refresh duration; take higher magnitude.
        if let Some(existing) = status.effects.iter_mut().find(|e| e.effect_type == ev.effect) {
            existing.remaining_turns = ev.duration;          // refresh (always)
            existing.magnitude = existing.magnitude.max(ev.potency);  // take higher
        } else {
            status.effects.push(ActiveEffect {
                effect_type: ev.effect,
                remaining_turns: ev.duration,
                magnitude: ev.potency,
            });
        }

        // D5α: re-derive triggered if effect modifies derive_stats.
        if matches!(ev.effect,
            StatusEffectType::AttackUp | StatusEffectType::DefenseUp
            | StatusEffectType::SpeedUp | StatusEffectType::Dead) {
            equip_changed.write(EquipmentChangedEvent {
                character: ev.target,
                slot: EquipSlot::None,   // stat-changed sentinel
            });
        }
    }
}
```

### Anti-Patterns to Avoid

- **Pushing directly to `StatusEffects.effects` outside `apply_status_handler`.** This is the D12 mistake the existing `apply_poison_trap` makes. Refactor it. Future contributors who do this break the stacking rule.
- **Reading `Time::delta_secs()` in `tick_status_durations`.** The tick is COUNT-based (one decrement per `StatusTickEvent`), not time-based. Adding `Time` would couple the ticker to wall-clock and break test determinism.
- **Putting a `.run_if(in_state(...))` gate on `apply_status_handler`.** It must run in both Dungeon and Combat. Leaving it ungated and message-driven is correct.
- **Relying on `EquipmentChangedEvent` as the only stat-change trigger.** Once #14 lands, *status changes* are also stat-change triggers. The D5α recommendation makes the existing system a "stat-changed" system in spirit even though the name keeps "Equipment" for v1. The doc-comment edit is mandatory.
- **Adding `Hash` or `Eq` to `ActiveEffect`.** It contains `f32` — same reason `DerivedStats` doesn't have them (`character.rs:122-126`). Use `PartialEq` only.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Duration ticker | A custom `Time::delta`-based timer | Count-based decrement reading `StatusTickEvent` | Determinism in tests; matches roadmap "tick on combat rounds AND dungeon steps" semantics. |
| Save-format for status effects | A custom `MapEntities` impl | Already-derived `Serialize + Deserialize` on `StatusEffects` | `Vec<ActiveEffect>` is already serde-clean. Roadmap §14 line 785 confirms the design. |
| Multi-effect resolver dispatch | A hand-rolled `match StatusEffectType` in every system | Existing `MessageReader<StatusTickEvent>` pattern + per-effect filter | Each resolver (`apply_poison_damage`, `apply_regen`) is its own system with its own `if effects.iter().any(|e| e.effect_type == X)` filter. No central dispatch table needed; this scales to N effects without a central point of contention. |
| Random-target selection for `Confused` | Custom RNG | Defer to #15 + use Bevy's transitive `rand` | #14 declares the variant for save-format stability; #15 owns the random-pick logic when it has a target list to choose from. |

---

## Common Pitfalls

### Pitfall 1: Refactored `apply_poison_trap` test needs an extra `app.update()` cycle

**What goes wrong:** The existing test `poison_trap_applies_status` at `features.rs:938-978` writes `MovedEvent`, calls `app.update()` once, and asserts `StatusEffectType::Poison` is present. After #14's refactor, the trap writes `ApplyStatusEvent` instead — the message must be **read by `apply_status_handler` in the next system run**. With one `app.update()`, the message is queued but not consumed.

**Why it happens:** Bevy `Messages<T>` allows multiple readers across frames; within a single `Update` schedule run, system order matters. If `apply_poison_trap` writes `ApplyStatusEvent` in a system scheduled before `apply_status_handler` (no explicit ordering), the handler reads in the same frame **only if** they're in the same schedule and unordered systems happen to run in the right sequence — which Bevy does NOT guarantee. The reliable fix is `apply_poison_trap.before(apply_status_handler)` — or, if that creates ordering complexity, just call `app.update()` twice in the test.

**How to avoid:** Either add `.before(apply_status_handler)` to all `ApplyStatusEvent`-writers (cleanest — guarantees same-frame application), OR document in the test that it needs `app.update()` twice. **Recommended: explicit `.before(apply_status_handler)` on every writer.** Same shape as `handle_dungeon_input.before(animate_movement)` at `dungeon/mod.rs:241`.

### Pitfall 2: `derive_stats` order-dependence after buffs land

**What goes wrong:** The doc-comment at `character.rs:340-342` is explicit:
> *"V1 status types are trivially order-independent because none of them modify a stat via the `magnitude` field — they are pure gates. #15 will add magnitude-modifying buff branches; at that point, order dependence must be re-evaluated."*

Once `AttackUp 0.5` and `AttackUp 0.5` exist as separate effects, the merge rule (D2: take higher magnitude) collapses them — but that's at the `apply_status_handler` level. Inside `derive_stats`, **iterating the effects list is fine because the merge rule guarantees AT MOST ONE `AttackUp` is present** at any given time. So `derive_stats`'s order-independence is preserved by virtue of the merge.

**How to avoid:** Write the deferred `derive_stats_status_order_independent` test (`character.rs:611-615`) AS PART of #14. Apply two of the same buff in different orders, assert the same `DerivedStats` output. The test ALSO acts as a regression guard against a future contributor adding a duplicate-stack code path.

### Pitfall 3: `EquipmentChangedEvent` name drift after D5α

**What goes wrong:** Per D5α, `apply_status_handler` writes `EquipmentChangedEvent` to trigger the existing `recompute_derived_stats_on_equipment_change` system. The event name implies "equipment changed" but the cause is "status applied" — readers of the message in the future may be confused.

**How to avoid:** Update the doc-comment on `EquipmentChangedEvent` at `inventory.rs:194-202` to read *"Emitted by `equip_item`, `unequip_item`, AND `apply_status_handler` when something requires `derive_stats` to re-run."* Update the system name? Not in #14 — that's a #25 polish concern. The semantic-drift is acceptable for v1.

### Pitfall 4: Dead variant interaction with `magnitude` re-derive

**What goes wrong:** `derive_stats` zeroes HP/MP on `Dead` (`character.rs:403-407`). If buff branches are added BEFORE the `Dead` branch, the buff math runs first — wasted work, but correct. If buff branches are added AFTER `Dead`, the buff math could try `0 * 1.5 = 0` — still correct, but creates a foot-gun if the math changes. **Always put the `Dead` branch LAST** so it dominates.

**How to avoid:** Comment-block-tag the `Dead` branch as `// LAST: zero-out dominates all buffs above` and add a unit test `derive_stats_dead_dominates_buffs` that applies `AttackUp` then `Dead` and asserts attack is 0 (because Dead zeroes max_hp/max_mp; attack stays — clarify what zeros and what doesn't). **Note:** the current `Dead` branch only zeros `max_hp`/`max_mp`, NOT `attack`/`defense`/etc. — this is correct behavior in the genre (a dead character's offensive stats are irrelevant; #15 won't allow them to act). Don't change this.

### Pitfall 5: Save-format breakage from variant reordering

**What goes wrong:** Per #11 Decision 7 (`character.rs:228-230`), discriminant order is locked. Adding `AttackUp/DefenseUp/SpeedUp/Regen/Silence/Blind/Confused` BEFORE the existing `Poison/Sleep/Paralysis/Stone/Dead` shifts every save game's serialized index by N — breaking every existing save.

**How to avoid:** **Append-only.** New variants go at the END of the enum, AFTER `Dead`. Discriminant indices 5+. Document this in the doc-comment at `character.rs:225-234`. Add a comment marker `// HISTORICAL APPEND ORDER — DO NOT REORDER` above the enum to flag for future contributors.

### Pitfall 6: `magnitude: f32` Reflect serialization

**What goes wrong:** `f32::NAN` and `f32::INFINITY` round-trip through serde but break invariants. A buff with `magnitude = NAN` would propagate NaN through `derive_stats`'s `attack += attack * 1.5`-style math.

**How to avoid:** `apply_status_handler` clamps incoming `potency` to a finite range: `let potency = ev.potency.clamp(0.0, 10.0)` (10x is well past any sane buff). This is a one-line guard. The existing #11 saturating-arithmetic precedent (`character.rs:331-336`) is the model — finite-clamp at the trust boundary.

### Pitfall 7: Tick race with `Dead` re-application

**What goes wrong:** A character at 1 HP gets a poison tick that brings them to 0 HP. Should this **immediately apply `Dead`** or wait for #15 to detect and apply? If #14 auto-applies `Dead`, it couples to the genre rule. If #14 doesn't, `current_hp = 0` is a "dying-but-not-dead" state.

**How to avoid:** **Defer to #15.** `apply_poison_damage` mutates `current_hp` via `saturating_sub` (mirroring `apply_pit_trap` at `features.rs:393-395`). It does NOT apply `Dead`. #15's combat resolver sees the zero-HP state and writes `ApplyStatusEvent { effect: Dead, ... }`. Document this contract in the doc-comment. **#14 ships a stub function `pub fn check_dead_and_apply(...)` for #15 to call** — this is a thin convenience that #15 wires into `turn_manager.rs::after_damage`.

---

## Security

### Known Vulnerabilities

| Library | CVE / Advisory | Severity | Status | Action |
|---------|---------------|----------|--------|--------|
| bevy | none found | — | — | (current pinned 0.18.1 is the project standard) |
| serde / ron | none found | — | — | (already in use; no new attack surface) |

No new dependencies; no new attack surface beyond what #1–#13 already shipped.

### Architectural Security Risks

| Risk | Affected | How It Manifests | Secure Pattern | Anti-Pattern to Avoid |
|------|----------|------------------|----------------|-----------------------|
| Crafted save with `magnitude = NaN` propagates through `derive_stats` | The new buff branches | `attack += attack * NAN` produces `NAN`; downstream comparisons return false; UI renders garbage | `clamp(0.0, 10.0)` at the trust boundary in `apply_status_handler` (Pitfall 6) | Trusting `f32` from save files without clamp |
| Save with `effects.len() == u32::MAX` causes OOM | `StatusEffects` deserialization | `Vec<ActiveEffect>` length not bounded | #23 must add a max-length cap on deserialization (mirrors the inventory bound concern at `inventory.rs:183-185`). **Out of scope for #14.** | Mutating `effects.push(...)` without a length cap (the `apply_status_handler` merge rule prevents unbounded growth at runtime — only crafted saves can do it) |
| Crafted `ApplyStatusEvent` from a hostile source bypasses gate logic | All `ApplyStatusEvent`-writers | If a future contributor exposes a debug command that writes `ApplyStatusEvent { effect: Dead, ... }` for an enemy entity | The handler currently doesn't validate the target; #15 is the right place for the validate-target check (is target alive? in this combat?) | Trusting the message body for combat-state correctness — defer to #15 |

The first one (NaN clamp) is the **only architectural-security item #14 ships**. The other two are flagged for #23 / #15.

### Trust Boundaries

- **`ApplyStatusEvent` from anywhere in the codebase:** The handler is a trust point. **Validation required:** clamp `potency` to `[0.0, 10.0]`. **What happens if skipped:** NaN propagates through `derive_stats`; combat damage math becomes unstable.
- **`StatusEffects` deserialized from save (deferred to #23):** **Validation required:** bound `effects.len()`. **What happens if skipped:** crafted save → OOM at load. Out of scope for #14 but flag for #23 plan.

---

## Performance

| Metric | Value / Range | Source | Notes |
|--------|---------------|--------|-------|
| `apply_status_handler` per-frame cost | O(N events × M existing effects per character) — typically N≤4, M≤5 → ~20 iterations max | (synthesized) | Dwarfed by render cost; not a hot path. |
| `tick_status_durations` per-frame cost | O(P party × M effects) — P=4, M≤5 → 20 iterations | (synthesized) | Same as above. |
| `derive_stats` cost after buff branches | +1 iteration over effects per buff variant added (4 buffs → +4 branches) | (synthesized; existing `derive_stats` does already iterate) | Negligible — `derive_stats` is called only on equipment-change events. |
| Memory delta | +5-7 enum variants × ~24 bytes (rust enum with f32 payload) | (synthesized) | Trivial. |

No benchmarks needed; the volumes are too small to register on any profile. Performance is **not a #14 concern**.

---

## Code Examples

Verified patterns from existing project code.

### Example 1: Plugin shape (mirrors `CellFeaturesPlugin`)

```rust
// Source: features.rs:142-189 (CellFeaturesPlugin) — file mirror this.
// File: src/plugins/combat/status_effects.rs

use bevy::prelude::*;
use crate::plugins::dungeon::{handle_dungeon_input, MovedEvent};
use crate::plugins::party::{
    ActiveEffect, EquipmentChangedEvent, EquipSlot, PartyMember, StatusEffectType, StatusEffects,
};
use crate::plugins::state::GameState;

/// Owns all status-effect systems, the `ApplyStatusEvent` and `StatusTickEvent`
/// messages, and the per-effect resolver systems.
///
/// Registered as a sub-plugin of `CombatPlugin` (combat/mod.rs).
pub struct StatusEffectsPlugin;

impl Plugin for StatusEffectsPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<ApplyStatusEvent>()
            .add_message::<StatusTickEvent>()
            .add_systems(
                Update,
                (
                    // Dungeon-step tick — gated; #15 will register a parallel
                    // combat-round emitter without touching this one.
                    tick_on_dungeon_step
                        .run_if(in_state(GameState::Dungeon))
                        .after(handle_dungeon_input),
                    // Message-driven systems — no state gate (idle when no events).
                    apply_status_handler.before(tick_status_durations),
                    tick_status_durations,
                    apply_poison_damage,
                    apply_regen,
                ),
            );
    }
}
```

### Example 2: `ApplyStatusEvent` message + handler

```rust
// File: src/plugins/combat/status_effects.rs (continued)

/// Canonical "apply this effect" message. Every status source — traps, enemy
/// spells (#15), items (#20) — writes this. The handler enforces stacking.
///
/// `Message`, NOT `Event` — Bevy 0.18 family rename. Read with
/// `MessageReader<ApplyStatusEvent>`.
#[derive(Message, Clone, Copy, Debug)]
pub struct ApplyStatusEvent {
    pub target: Entity,
    pub effect: StatusEffectType,
    /// Multiplier for buffs (e.g. `0.5` = +50% attack); fixed at `0.0`-`1.0`
    /// for tick effects (Poison, Regen). Clamped to `[0.0, 10.0]` defensively
    /// (Pitfall 6).
    pub potency: f32,
    /// `Some(n)` for tickable effects; `None` for permanent (Stone, Dead).
    pub duration: Option<u32>,
}

/// The single mutator of `StatusEffects.effects`. Every other system writes
/// `ApplyStatusEvent` rather than pushing directly.
///
/// Stacking rule (roadmap §14 line 780, refined):
/// - Same effect already present: refresh duration, take higher magnitude.
/// - Permanent-cure effects (Stone, Dead) already present: no-op.
/// - Otherwise: push.
///
/// D5α: writes `EquipmentChangedEvent` to trigger `derive_stats` re-run when
/// the change affects derived output (`AttackUp`/`DefenseUp`/`SpeedUp`/`Dead`).
pub fn apply_status_handler(
    mut events: MessageReader<ApplyStatusEvent>,
    mut equip_changed: MessageWriter<EquipmentChangedEvent>,
    mut characters: Query<&mut StatusEffects, With<PartyMember>>,
) {
    for ev in events.read() {
        // Defensive clamp on f32 trust boundary (Pitfall 6).
        let potency = ev.potency.clamp(0.0, 10.0);

        let Ok(mut status) = characters.get_mut(ev.target) else { continue };

        // Permanent effects: re-application no-op.
        if matches!(ev.effect, StatusEffectType::Stone | StatusEffectType::Dead)
           && status.has(ev.effect)
        {
            continue;
        }

        // Stacking merge.
        if let Some(existing) = status.effects.iter_mut().find(|e| e.effect_type == ev.effect) {
            existing.remaining_turns = ev.duration;
            existing.magnitude = existing.magnitude.max(potency);
        } else {
            status.effects.push(ActiveEffect {
                effect_type: ev.effect,
                remaining_turns: ev.duration,
                magnitude: potency,
            });
        }

        // D5α: nudge `derive_stats` re-run on stat-affecting variants.
        if matches!(
            ev.effect,
            StatusEffectType::AttackUp
            | StatusEffectType::DefenseUp
            | StatusEffectType::SpeedUp
            | StatusEffectType::Dead
        ) {
            equip_changed.write(EquipmentChangedEvent {
                character: ev.target,
                slot: EquipSlot::None,  // sentinel — "not really an equipment change"
            });
        }
    }
}
```

### Example 3: `tick_status_durations` (count-based decrement)

```rust
// File: src/plugins/combat/status_effects.rs (continued)

/// Internal tick message. Two emitters:
/// - `tick_on_dungeon_step` (this file, fires on `MovedEvent`)
/// - #15's `turn_manager::round_end` (future — fires on combat-round-end)
#[derive(Message, Clone, Copy, Debug)]
pub struct StatusTickEvent {
    pub target: Entity,
}

/// Decrement `remaining_turns` and remove expired effects.
///
/// `None` (permanent) effects are skipped — they don't tick.
/// Emits `EquipmentChangedEvent` for the affected character if a removed
/// effect was a stat-modifier (D5α).
pub fn tick_status_durations(
    mut ticks: MessageReader<StatusTickEvent>,
    mut equip_changed: MessageWriter<EquipmentChangedEvent>,
    mut characters: Query<&mut StatusEffects, With<PartyMember>>,
) {
    for ev in ticks.read() {
        let Ok(mut status) = characters.get_mut(ev.target) else { continue };

        let pre_count = status.effects.len();
        let mut had_stat_modifier_removed = false;

        status.effects.retain_mut(|e| {
            match e.remaining_turns {
                None => true,                 // permanent — keep
                Some(0) => {                  // expired — drop
                    if matches!(e.effect_type,
                        StatusEffectType::AttackUp | StatusEffectType::DefenseUp
                        | StatusEffectType::SpeedUp) {
                        had_stat_modifier_removed = true;
                    }
                    false
                }
                Some(ref mut n) => {
                    *n -= 1;
                    if *n == 0 {
                        if matches!(e.effect_type,
                            StatusEffectType::AttackUp | StatusEffectType::DefenseUp
                            | StatusEffectType::SpeedUp) {
                            had_stat_modifier_removed = true;
                        }
                        false                 // becomes 0 → drop
                    } else {
                        true
                    }
                }
            }
        });

        if had_stat_modifier_removed {
            equip_changed.write(EquipmentChangedEvent {
                character: ev.target,
                slot: EquipSlot::None,
            });
        }

        // Diagnostic for tests / dev observability.
        let _ = pre_count;
    }
}
```

(Implementation note: the `retain_mut` body has a subtle interaction — `Some(0)` shouldn't actually appear at the start of a tick because the previous tick should have removed it. Defensive handling kept for safety; the test `tick_removes_expired_poison` verifies the contract.)

### Example 4: `apply_poison_damage` resolver

```rust
// File: src/plugins/combat/status_effects.rs (continued)

use crate::plugins::party::DerivedStats;

/// On every tick, every party member with `Poison` takes damage proportional
/// to `magnitude` (D7).
///
/// **D7 default (this example):** flat per-tick damage = `(max_hp / 20).max(1) * magnitude`.
/// Magnitude `1.0` → 5% of max_hp per tick (round down, min 1). Tunable per-source
/// via `ApplyStatusEvent.potency`.
pub fn apply_poison_damage(
    mut ticks: MessageReader<StatusTickEvent>,
    mut characters: Query<(&StatusEffects, &mut DerivedStats), With<PartyMember>>,
) {
    for ev in ticks.read() {
        let Ok((status, mut derived)) = characters.get_mut(ev.target) else { continue };

        let Some(poison) = status.effects.iter().find(|e| e.effect_type == StatusEffectType::Poison) else {
            continue;
        };

        let base = (derived.max_hp / 20).max(1);
        // Saturating arithmetic mirrors apply_pit_trap (features.rs:393-395).
        let damage = (base as f32 * poison.magnitude) as u32;
        derived.current_hp = derived.current_hp.saturating_sub(damage);
        // Pitfall 7: do NOT apply `Dead` here — defer to #15.
    }
}

/// Mirrors `apply_poison_damage` but heals.
pub fn apply_regen(
    mut ticks: MessageReader<StatusTickEvent>,
    mut characters: Query<(&StatusEffects, &mut DerivedStats), With<PartyMember>>,
) {
    for ev in ticks.read() {
        let Ok((status, mut derived)) = characters.get_mut(ev.target) else { continue };

        let Some(regen) = status.effects.iter().find(|e| e.effect_type == StatusEffectType::Regen) else {
            continue;
        };

        let base = (derived.max_hp / 20).max(1);
        let healing = (base as f32 * regen.magnitude) as u32;
        derived.current_hp = (derived.current_hp.saturating_add(healing)).min(derived.max_hp);
    }
}
```

### Example 5: Refactored `apply_poison_trap`

```rust
// Source: features.rs:412-445 (CURRENT) — refactor target.
// New shape (in features.rs, replacing the existing function):

use crate::plugins::combat::status_effects::ApplyStatusEvent;

fn apply_poison_trap(
    mut moved: MessageReader<MovedEvent>,
    floors: Res<Assets<DungeonFloor>>,
    dungeon_assets: Option<Res<DungeonAssets>>,
    active_floor: Res<ActiveFloorNumber>,
    party: Query<Entity, With<PartyMember>>,         // <-- now reads only Entity
    mut apply: MessageWriter<ApplyStatusEvent>,        // <-- writes the canonical message
    mut sfx: MessageWriter<SfxRequest>,
) {
    let Some(assets) = dungeon_assets else { return };
    let floor_handle = floor_handle_for(&assets, active_floor.0);
    let Some(floor) = floors.get(floor_handle) else { return };
    for ev in moved.read() {
        let cell = &floor.features[ev.to.y as usize][ev.to.x as usize];
        if !matches!(cell.trap, Some(TrapType::Poison)) { continue }
        for entity in &party {
            apply.write(ApplyStatusEvent {
                target: entity,
                effect: StatusEffectType::Poison,
                potency: 1.0,                            // baseline trap potency (designer)
                duration: Some(5),                       // 5-tick poison duration (D8)
            });
        }
        sfx.write(SfxRequest { kind: SfxKind::Door });   // placeholder hiss (unchanged)
    }
}
```

The refactor is tiny — drop `&mut StatusEffects` query, add `MessageWriter<ApplyStatusEvent>`, replace push with write. The system signature simplifies because we no longer need `Query<&mut StatusEffects>`.

### Example 6: `derive_stats` buff branches

```rust
// Source: character.rs:343-426 — add buff branches in the post-pass section
// (around line 400, BEFORE the existing `Dead` branch).

// ── Buff branches (NEW for #14) ────────────────────────────────────────
// Multipliers stack additively across multiple effects of the SAME variant
// would be problematic — but the merge rule (apply_status_handler) ensures
// AT MOST ONE of each variant is present. So here we just iterate.
for effect in &status.effects {
    match effect.effect_type {
        StatusEffectType::AttackUp => {
            // attack += attack * magnitude, saturating
            let bonus = (stat_attack as f32 * effect.magnitude) as u32;
            stat_attack = stat_attack.saturating_add(bonus);
        }
        StatusEffectType::DefenseUp => {
            let bonus = (stat_defense as f32 * effect.magnitude) as u32;
            stat_defense = stat_defense.saturating_add(bonus);
        }
        StatusEffectType::SpeedUp => {
            let bonus = (stat_speed as f32 * effect.magnitude) as u32;
            stat_speed = stat_speed.saturating_add(bonus);
        }
        // Poison, Sleep, Paralysis, Stone, Silence, Regen: not derive-time modifiers.
        _ => {}
    }
}

// ── Dead branch (EXISTING — keep LAST per Pitfall 4) ────────────────────
if status.has(StatusEffectType::Dead) {
    max_hp = 0;
    max_mp = 0;
}
```

(Note: the local mutable `stat_attack` etc. need to change from `let stat_attack = ...` to `let mut stat_attack = ...` at lines 385-395. Trivial edit.)

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `apply_poison_trap` directly pushes `ActiveEffect` (D12 deferred) | All sources fire `ApplyStatusEvent`; `apply_status_handler` is sole mutator | #14 (this feature) | One canonical path. Stacking rule enforced once. |
| `derive_stats` has only `Dead` branch | `derive_stats` has `AttackUp/DefenseUp/SpeedUp` branches BEFORE `Dead` | #14 | Buffs land. |
| `recompute_derived_stats_on_equipment_change` fires only on equipment changes | Also fires on stat-affecting status changes (D5α) | #14 | Buff application propagates to `DerivedStats` immediately. |

**Deprecated/outdated:**
- The doc-comment at `character.rs:230-231` "Buffs ... are deferred to #15" — **outdated as of #14.** The roadmap §14 line 779 owns them. Update the comment.
- The doc-comment at `character.rs:340-342` "#15 will add magnitude-modifying buff branches" — **outdated as of #14.** Update to "#14 added the buff branches; #15 may add additional effect types."
- The comment at `features.rs:412` "Naive push (D12) — stacking deferred to #14" — **resolved by this feature.** Remove the comment after refactor.

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | `cargo test` (Rust stdlib) |
| Config file | `Cargo.toml` test config |
| Quick run command | `cargo test -p druum --lib plugins::combat::status_effects` |
| Full suite command | `cargo test -p druum` |
| Time-deterministic helper | `TimeUpdateStrategy::ManualDuration` (per agent-memory `reference_bevy_018_time_update_strategy.md`) — NOT needed here because tick is count-based |

### Requirements → Test Map

| Requirement | Behavior | Test Type | Automated Command | File Exists? |
|-------------|----------|-----------|-------------------|--------------|
| Stacking — same effect refreshes duration | Apply Poison, apply Poison again with longer duration; assert effects.len() == 1 and duration is the new one | layer-1 logic test | `cargo test -p druum status_effects::tests::stacking_refreshes_duration` | ❌ create |
| Stacking — take higher magnitude | Apply Poison(0.5), apply Poison(1.0); assert magnitude == 1.0 | layer-1 logic test | `cargo test -p druum status_effects::tests::stacking_takes_higher_magnitude` | ❌ create |
| Stacking — Stone re-application no-op | Apply Stone, apply Stone; assert duration is unchanged | layer-2 app test | `cargo test -p druum status_effects::tests::stone_reapply_is_noop` | ❌ create |
| Tick decrements remaining_turns | Apply Poison duration=5, send 1 tick; assert duration == 4 | layer-2 app test | `cargo test -p druum status_effects::tests::tick_decrements_duration` | ❌ create |
| Tick removes effect at 0 | Apply Poison duration=1, send 1 tick; assert effect removed | layer-2 app test | `cargo test -p druum status_effects::tests::tick_removes_expired_poison` | ❌ create |
| Permanent effects don't tick | Apply Stone (None), send 5 ticks; assert effect still present | layer-2 app test | `cargo test -p druum status_effects::tests::permanent_does_not_tick` | ❌ create |
| Poison damage reduces HP | Apply Poison potency=1.0, send 1 tick; assert current_hp reduced | layer-2 app test | `cargo test -p druum status_effects::tests::poison_damages_on_tick` | ❌ create |
| Regen heals HP | Set current_hp=1, apply Regen, send 1 tick; assert current_hp increased, capped at max | layer-2 app test | `cargo test -p druum status_effects::tests::regen_heals_on_tick` | ❌ create |
| derive_stats AttackUp branch | Apply AttackUp magnitude=0.5; assert attack is 1.5x baseline | layer-1 logic test (in `character.rs::tests`) | `cargo test -p druum character::tests::derive_stats_attack_up_buffs_attack` | ❌ create |
| derive_stats order-independence | Apply AttackUp twice in different orders; assert same DerivedStats output | layer-1 logic test (in `character.rs::tests`) | `cargo test -p druum character::tests::derive_stats_status_order_independent` | ❌ create (deferred-to-#14 per `character.rs:611-615`) |
| Dungeon-step tick fires on MovedEvent | Spawn party, apply Poison, write MovedEvent, app.update() x2; assert HP reduced | layer-2 app test | `cargo test -p druum status_effects::tests::dungeon_step_fires_tick` | ❌ create |
| Pitfall 1 regression — poison_trap_applies_status flushes via handler | (Existing test must pass after refactor — likely needs +1 `app.update()`) | layer-2 app test (existing) | `cargo test -p druum cell_features::poison_trap_applies_status` | ✅ exists (`features.rs:938-978`) — adapts |

**Total #14 tests:** 11 new + 1 existing-needing-edit = **12 tests** (well within roadmap's +8-12 envelope).

### Gaps (files to create before implementation)

- [ ] `src/plugins/combat/status_effects.rs` — module + plugin + `ApplyStatusEvent` + `StatusTickEvent` + `apply_status_handler` + `tick_status_durations` + `apply_poison_damage` + `apply_regen` + `tick_on_dungeon_step` + helper predicates + `#[cfg(test)] mod tests`
- [ ] `src/plugins/combat/mod.rs` — add `mod status_effects;` and `app.add_plugins(StatusEffectsPlugin)` line
- [ ] `src/plugins/party/character.rs` — add 5 enum variants + buff branches in `derive_stats` + `derive_stats_attack_up_buffs_attack` test + `derive_stats_status_order_independent` test
- [ ] `src/plugins/dungeon/features.rs` — refactor `apply_poison_trap` signature & body
- [ ] `src/plugins/party/inventory.rs` — doc-comment update on `EquipmentChangedEvent` (Pitfall 3)

No new asset authoring required — `floor_01.dungeon.ron` already has the poison trap testbed.

---

## Decision Matrix

Categorized per the user's auto-memory rule (`feedback_user_answers_to_options.md`): **Category C** entries are genuine A/B/C user picks; **Category B** entries are recommended defaults; **Category A** entries are blocking/no-answer (none in #14).

### Category B — Recommended defaults (implementer can proceed)

| ID | Question | Recommendation | Rationale |
|----|----------|----------------|-----------|
| **D1** | Which `StatusEffectType` variants does #14 add? | Add **`AttackUp, DefenseUp, SpeedUp, Regen, Silence`** (5). Defer `Blind, Confused` to #15. | Roadmap §14 lines 778-779 names all 7 but only buff/Silence have a system in #14 reading them. `Blind`/`Confused` are name-only until #15 wires accuracy/random-target — declaring them in #14 burns save-format slots speculatively (Pitfall 5). Append at end (after `Dead`). |
| **D2** | Stacking semantics | **Same effect → refresh duration, take higher magnitude. Permanent effects (Stone/Dead) → re-application no-op.** Roadmap §14 line 780 holds. | Cleanest behavior; tested by 3 dedicated tests above. |
| **D3** | Module placement | **`src/plugins/combat/status_effects.rs` + new `StatusEffectsPlugin` registered as a sub-plugin of `CombatPlugin`.** | Roadmap line 760 names the path. CellFeaturesPlugin precedent at `main.rs:33` shows the pattern. |
| **D4** | Tick trigger architecture | **Single `StatusTickEvent` message with two emitters — #14 owns the dungeon-step emitter, #15 owns the combat-round emitter.** | Cleanest decoupling. Mirrors `MovedEvent` / `TeleportRequested` / `EncounterRequested` precedent. |
| **D5** | Stat-recompute trigger on buff change | **D5α — `apply_status_handler` writes `EquipmentChangedEvent` for buff/Dead variants.** | Reuses the existing recompute pipeline + caller-clamp. Doc-comment update on `EquipmentChangedEvent` covers naming drift. |
| **D6** | `derive_stats` buff branch placement | **Buff branches BEFORE `Dead` branch. `Dead` LAST per Pitfall 4.** | Dead-zero dominates; clarifies intent. |
| **D8** | Poison-trap `potency` value | **`potency = 1.0, duration = Some(5)`** (matches existing `POISON_TURNS = 5` constant at `features.rs:421`). | Preserves end-state behavior of existing test. |
| **D10** | `Blind`/`Confused` enum slots | **Defer to #15.** | Save-format slots are precious; declaring an enum variant with no reader is speculative API surface. |
| **D11** | `is_paralyzed`/`is_asleep`/`is_silenced` shape | **`pub fn` predicate helpers, NOT systems.** | Same shape as `EquipSlot::read`. Unit-testable without `App`. #15 imports them. |
| **D12** | `block_action_if_*` system shape | **No #14 systems for blocking — predicates only. #15 uses predicates inside `turn_manager::collect_player_actions`.** | No schedule yet exists where they could run. |
| **D13** | `Dead`-on-zero-HP application | **Defer to #15. #14 ships `pub fn check_dead_and_apply` stub.** | Pitfall 7. Keeps #14 from coupling to combat genre rules. |
| **D14** | NaN clamp on `potency` | **`clamp(0.0, 10.0)` in `apply_status_handler`.** | Pitfall 6. One-line guard. |
| **D15** | `Hash`/`Eq` on `ActiveEffect` | **Do NOT add.** | Same `f32` rationale as `DerivedStats`. |

### Category C — Genuine A/B/C user picks

| ID | Question | Options | Recommended default |
|----|----------|---------|---------------------|
| **D7** | Per-tick poison/regen damage formula | **A:** `(max_hp / 20).max(1) * magnitude` — % of max (5% baseline at magnitude=1.0). Scales with character power; weaker chars take less damage. <br> **B:** Flat per tick: `5 * magnitude`. Simple; doesn't scale. <br> **C:** Formula scales with caster INT or trap difficulty (deferred — needs caster data). | **A** (% of max, default magnitude=1.0 → 5%/tick). Wizardry/Etrian convention is %-based. The `(max_hp / 20).max(1)` floor prevents 0-damage on low-HP chars. |
| **D9** | Dungeon-step tick frequency | **A:** Every step (one tick per `MovedEvent`). Wizardry-canonical. <br> **B:** Every Nth step (e.g., every 3 steps). Less brutal. <br> **C:** Time-based (every 5 seconds). Real-time feel. | **A** (every step). Matches the project's overall Wizardry tone. |

These are the two questions where the user's answer changes implementation but neither answer is technically wrong. Per the feedback memory rule, the planner should ask before assuming.

---

## Integration Story

### What #14 ships TODAY (consumers exist)

- `ApplyStatusEvent` — written by **`apply_poison_trap`** (refactored). #14 verifies it works.
- `StatusTickEvent` — emitted by **`tick_on_dungeon_step`** (#14-owned). Triggers `tick_status_durations`, `apply_poison_damage`, `apply_regen`.
- `apply_status_handler` — sole mutator of `StatusEffects.effects`.
- `tick_status_durations` — sole decrementer of `remaining_turns`.
- `derive_stats` — handles `AttackUp/DefenseUp/SpeedUp/Dead`; buffs flow through `EquipmentChangedEvent` re-derive.
- `pub fn is_paralyzed/is_asleep/is_silenced` — predicates available for import.
- `pub fn check_dead_and_apply` (stub) — #15-callable convenience.

### What #14 LEAVES for #15 (combat)

- A **combat-round emitter** of `StatusTickEvent` — one line in `turn_manager.rs::round_end`: `for entity in alive_combatants { tick.write(StatusTickEvent { target: entity }) }`.
- Wiring of the **block-action predicates** into `turn_manager.rs::collect_player_actions`:
  ```rust
  let status = status_query.get(party_member)?;
  if is_paralyzed(status) || is_asleep(status) {
      // skip turn
  }
  ```
- Wiring of **`is_silenced`** into spell-action collection.
- Decision on whether `is_blind` adds an accuracy modifier (numeric) or a re-roll (binary).
- Decision on whether `is_confused` randomizes target or skips action.
- Auto-apply `Dead` on zero HP via the #14-shipped `check_dead_and_apply` helper.

### What #14 LEAVES for #20 (consumables)

- A "ailment-curing potion" item that writes `ApplyStatusEvent { effect: <to-cure>, duration: Some(0) }` to expire next tick — OR introduces a new `RemoveStatusEvent` message. **Defer this design to #20.** The handler shape is small either way; #14 doesn't speculate.

### What #14 LEAVES for #25 (UI)

- Status icon overlay on character portrait. Read `&StatusEffects` in the UI system, render an icon per effect. Defer entirely.

### What #14 TOUCHES outside `combat/`

- `src/plugins/party/character.rs` — +5 enum variants (lines ~242-249, append after `Dead`); buff branches in `derive_stats` (lines ~399-410); 2 new tests.
- `src/plugins/party/inventory.rs` — doc-comment update on `EquipmentChangedEvent`. Zero behavioral change.
- `src/plugins/dungeon/features.rs` — `apply_poison_trap` refactor (lines 412-445). Zero new tests; 1 existing test test continues to pass with possible `app.update()` count adjustment.

---

## Top 3 risks with mitigations

### Risk 1: `apply_status_handler` ordering against writers — same-frame readability

**What goes wrong:** A trap fires `ApplyStatusEvent` in `Update`. The handler is also in `Update`. Without explicit ordering, the handler may run before the writer in the same frame, causing the message to be deferred to the next frame. Tests pass because they call `app.update()` twice, but production behavior is "1-frame delay between trap and effect" — visible to the player as "I stepped on poison but my icon didn't update until I moved again."

**Likelihood:** Medium-high (Bevy's default system order is non-deterministic between unordered systems).

**Mitigation:** Every `ApplyStatusEvent` writer must be ordered `.before(apply_status_handler)`. The plugin build does this for in-`combat/` writers; for the cross-domain writer in `features.rs::apply_poison_trap`, add `.before(apply_status_handler)` in `CellFeaturesPlugin::build`. **Symmetric pattern: every `StatusTickEvent` emitter is `.before(tick_status_durations)`.** Document in the plugin doc-comment.

### Risk 2: Save-format breakage from variant order

**What goes wrong:** A future contributor adds `AttackUp` between `Poison` and `Sleep` in `StatusEffectType`, shifting every saved status effect's discriminant. Existing saves load garbage.

**Likelihood:** Medium (the variant order looks alphabetical, which tempts reordering).

**Mitigation:** Comment marker `// HISTORICAL APPEND ORDER — DO NOT REORDER. New variants go at end.` above the enum. Cross-reference Pitfall 5 in this document. **Add a regression test that round-trips a `StatusEffectType::Poison` value through serde and asserts the discriminant byte is `0`.** This locks the format at the test level — any reorder fails CI.

### Risk 3: `magnitude: 0.0` on Poison from existing `apply_poison_trap` — silent zero damage

**What goes wrong:** The current `apply_poison_trap` at `features.rs:439` writes `magnitude: 0.0`. If the refactor preserves this default and the new `apply_poison_damage` uses the formula `damage = base * magnitude`, **damage is always zero** — the existing test passes (it only checks the effect is present, not that damage applies on tick) but production poison does nothing.

**Likelihood:** High if the planner overlooks this.

**Mitigation:** The refactored `apply_poison_trap` must set `potency: 1.0` (NOT `0.0`) — see Example 5 above. Add a smoke test `dungeon_step_poison_trap_damages_party` that walks onto poison, takes a step, and asserts HP went down. Document in the planner doc-comment that `magnitude == 0.0` is a footgun (`apply_poison_damage` would produce zero). **Strongly consider clamping `apply_poison_damage` to a min-1 floor:** `let damage = ((base as f32 * mag) as u32).max(1);` — this matches the `(max_hp / 20).max(1)` baseline and prevents the zero-damage trap.

---

## LOC and test count estimate

Roadmap §14 budget: **+350-500 LOC**, **+8-12 tests**, **0 new deps**.

| Component | LOC estimate |
|-----------|--------------|
| `combat/status_effects.rs` (full file: plugin, 2 messages, 5 systems, 3 helpers, tests) | ~340 |
| `character.rs` (5 enum variants + buff branches + 2 new tests + doc-comment updates) | ~70 |
| `combat/mod.rs` (add_plugins line + import) | ~3 |
| `dungeon/features.rs` (apply_poison_trap refactor; signature + body) | net ~+5 (signature swap nets out) |
| `inventory.rs` (doc-comment edit) | ~5 |
| **Total** | **~423 LOC** |

Squarely in the +350-500 envelope.

| Component | Test count |
|-----------|------------|
| `status_effects::tests` (new module) | 8-9 |
| `character::tests` (buff branches + order-independence) | 2 |
| `features::app_tests::poison_trap_applies_status` (existing — adapt, +0) | 0 (modified, not new) |
| **Total** | **10-11 new tests** |

In the +8-12 envelope.

**Sanity-check:** The roadmap is realistic. The +350-500 LOC bound is generous; this plan is at ~423 (room for ~80 LOC slack). The test count is at 10-11 (room for one more if D7's per-tick formula gets a dedicated test for the magnitude=0 edge case).

---

## `ApplyStatusEvent` exact Rust type signature

```rust
// File: src/plugins/combat/status_effects.rs

/// Canonical "apply this status effect to this entity" message.
///
/// Every status source — traps (`apply_poison_trap`), enemy spells (#15),
/// player items (#20) — writes this. The single `apply_status_handler`
/// system reads it and enforces stacking semantics.
///
/// `Message`, NOT `Event` — Bevy 0.18 family rename. Read with
/// `MessageReader<ApplyStatusEvent>`. Register with
/// `app.add_message::<ApplyStatusEvent>()`.
///
/// **Field semantics:**
/// - `target`: the `Entity` receiving the effect. Typically a `PartyMember`
///   in v1 (#14); enemy entities supported once #15 ships them.
/// - `effect`: which `StatusEffectType` to apply or refresh.
/// - `potency`: magnitude. For buffs, this is the multiplier (e.g. `0.5` = +50%).
///   For tick effects (Poison/Regen), this is the per-tick magnitude. Clamped
///   to `[0.0, 10.0]` by `apply_status_handler` (Pitfall 6, defensive trust
///   boundary).
/// - `duration`: `Some(n)` for `n` ticks of effect. `None` for permanent
///   effects (Stone, Dead) — these only end via cure (#18 temple, #20 potion).
///
/// **Stacking:** If `target` already has `effect`, the handler refreshes
/// `duration` and takes `max(existing.magnitude, new.potency)`. Permanent
/// effects already present are no-ops on re-application.
///
/// **No `source` field** (deliberate). Damage attribution is captured at
/// caller-time (the writer of the message); persisting it on `ActiveEffect`
/// would force save-format migration for zero current benefit.
#[derive(Message, Clone, Copy, Debug)]
pub struct ApplyStatusEvent {
    pub target: Entity,
    pub effect: StatusEffectType,
    pub potency: f32,
    pub duration: Option<u32>,
}
```

Field-by-field rationale documented inline. **No `source: Option<Entity>`** (per B above). **`duration: Option<u32>` not `u32`** (matches `ActiveEffect.remaining_turns` shape, supports permanent effects).

---

## Open Questions

1. **D7 — Per-tick poison/regen damage formula**
   - What we know: spec line 779 says "ticks per turn"; line 781 says "verify HP and removal".
   - What's unclear: % of max vs flat vs INT-scaled.
   - Recommendation: Ask the user. Default to %-based (option A in D7) if unanswered.

2. **D9 — Dungeon-step tick frequency**
   - What we know: spec line 782 says "tick on dungeon steps".
   - What's unclear: every step vs every Nth step.
   - Recommendation: Ask the user. Default to every-step (option A in D9) if unanswered.

3. **Should `Regen` cure `Poison` (or vice-versa)?**
   - Roadmap is silent. Wizardry's neutralize-poison spell removes poison but doesn't apply regen. Wizardry's "cure light wounds" heals but doesn't remove poison.
   - Recommendation: **No interaction in #14.** Two effects coexist independently — Poison ticks down HP, Regen ticks up HP, net effect is the algebraic sum. If the user wants "Regen cures Poison" semantics, defer to #15 (combat) or #20 (consumables).

---

## Sources

### Primary (HIGH confidence)

- [project_druum_overview.md](../../.claude/agent-memory/researcher/project_druum_overview.md) — Druum project shape, Bevy 0.18.x target.
- [Druum #11 Equipment + derive_stats contract](../../.claude/agent-memory/researcher/reference_druum_11_equipment_derive_stats_contract.md) — Equipment is `Handle<ItemAsset>`, `derive_stats` is pure with `&[ItemStatBlock]`.
- [Bevy 0.18 Event vs Message split](../../.claude/agent-memory/researcher/feedback_bevy_0_18_event_message_split.md) — `Message`/`MessageReader`/`MessageWriter` are the canonical Bevy 0.18 names.
- [Bevy 0.18 input tests — three layers](../../.claude/agent-memory/researcher/feedback_bevy_input_test_layers.md) — Layer-2 test pattern for `MovedEvent`-driven systems.
- [Bevy 0.18 TimeUpdateStrategy](../../.claude/agent-memory/researcher/reference_bevy_018_time_update_strategy.md) — Test-deterministic time advance (NOT used in #14 but worth noting).
- `src/plugins/party/character.rs:1-616` — full file (Equipment, StatusEffects, derive_stats, all enum and bundle defs)
- `src/plugins/party/mod.rs:1-175` — PartyPlugin registration, debug spawn
- `src/plugins/party/inventory.rs:1-555` — Inventory, ItemKind, EquipmentChangedEvent, recompute_derived_stats_on_equipment_change
- `src/plugins/dungeon/mod.rs:1-200, 192-197, 744-879` — DungeonPlugin, MovedEvent, handle_dungeon_input
- `src/plugins/dungeon/features.rs:1-200, 412-445, 700-934` — CellFeaturesPlugin, apply_poison_trap (refactor target), test patterns
- `src/plugins/combat/mod.rs:1-19` — CombatPlugin stub (current state)
- `src/plugins/save/mod.rs:1-9` — SavePlugin empty (no #14 work)
- `src/plugins/state/mod.rs:1-89` — GameState/SubState defs
- `src/plugins/input/mod.rs:1-160` — DungeonAction enum (no #14 work)
- `src/data/items.rs:80-118` — ItemAsset shape (key_id field — no #14 work)
- `src/main.rs:1-37` — top-level plugin registration order
- `project/research/20260506-080000-feature-13-cell-features.md` — #13 research mirror for structure
- `project/orchestrator/20260506-100000-feature-13-cell-features-research-plan.md` — orchestrator format

### Secondary (MEDIUM confidence)

- (None — every claim cites a primary source above.)

### Tertiary (LOW confidence)

- (None — every recommendation is grounded in the local code or a HIGH-confidence agent-memory entry.)

---

## Metadata

**Confidence breakdown:**

- Existing scaffolding inventory (Section A-N): HIGH — every claim cites a verified file:line in the merged tree.
- Architecture decisions (D1-D6, D8, D10-D15): HIGH — recommendations grounded in existing precedents.
- Architecture decisions D7, D9: MEDIUM — both recommendations are plausible defaults but the user has a real preference call.
- Stacking semantics (D2): HIGH — survives stress-testing; matches roadmap spec line 780.
- `ApplyStatusEvent` shape: HIGH — refined from roadmap spec to add `Option<u32>` for permanent-cure cases; follows existing `ActiveEffect.remaining_turns` shape.
- Pitfalls (1-7): HIGH — every pitfall maps to a concrete code path or a save-format invariant.
- LOC estimate (~423): MEDIUM — within roadmap range; depends on how dense the test module gets.
- Test count estimate (10-11): HIGH — itemized list of 12 tests in the validation table.

**Research date:** 2026-05-07
