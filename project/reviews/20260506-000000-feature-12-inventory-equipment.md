# Review: Feature #12 — Inventory & Equipment data layer

**PR:** https://github.com/codeinaire/druum-dungeon-crawler/pull/12
**Branch:** `feature/12-inventory-equipment`
**Verdict:** WARNING
**Date:** 2026-05-06

---

## What was reviewed

Full review of 4 commits (`f4ca434` → `a152171` → `a0f88de` → `10002eb`) relative to the post-#11 `main` tip (`8865b26`). Source files received full coverage; project/orchestrator/docs files were skimmed for context only.

**Files receiving full review:**
- `src/plugins/party/inventory.rs` (929 lines)
- `src/plugins/party/mod.rs`
- `src/data/items.rs`
- `assets/items/core.items.ron`
- `tests/item_db_loads.rs`
- `tests/equip_from_loaded_items.rs`

---

## Behavioral delta

Before: `Equipment` slots held `Option<Handle<ItemAsset>>` (frozen by #11) but nothing populated them — no `ItemAsset` type registered, no bridge from `ItemDb` to `Assets<ItemAsset>`, no inventory component.

After: Full data layer live. `Inventory(Vec<Entity>)` per character; each entity carries `ItemInstance(Handle<ItemAsset>)`. `equip_item` / `unequip_item` / `give_item` helpers move items between bag and slots. `EquipmentChangedEvent` (Message) triggers `recompute_derived_stats_on_equipment_change`, which flattens slot handles via `Assets<ItemAsset>` and calls the frozen `derive_stats`. `populate_item_handle_registry` bridges the loaded `ItemDb` into `Assets<ItemAsset>` on `OnExit(GameState::Loading)`. 8 starter items authored in RON; 8 placeholder PNGs generated.

---

## Frozen-files contract

- `character.rs`: **0 lines changed** between `8865b26` and `10002eb`. ✓
- `Cargo.toml` / `Cargo.lock` / `src/main.rs`: **0 lines changed**. ✓
- `src/plugins/loading/mod.rs`: **unchanged**. ✓

---

## Static analysis

- `cargo check`: clean ✓
- `cargo test --workspace`: 114 tests pass (111 lib + 3 integration) ✓
- `cargo test --features dev`: 117 tests pass ✓
- `cargo clippy --all-targets -- -D warnings`: clean ✓
- `cargo clippy --all-targets --features dev -- -D warnings`: clean ✓
- `cargo fmt --check`: **FAIL** — one import ordering violation in `tests/equip_from_loaded_items.rs`

---

## Findings

### [HIGH] `cargo fmt --check` fails — import ordering violation

**File:** `tests/equip_from_loaded_items.rs:40`

**Issue:** `rustfmt` expects identifiers sorted case-insensitively within a use group. `Equipment` sorts after `EquipSlot` and `EquipmentChangedEvent` by that rule, but the import has `Equipment` first.

**Fix:**

```rust
// Current (fails fmt):
use druum::plugins::party::{
    DerivedStats, Equipment, EquipmentChangedEvent, EquipSlot, Inventory, ItemHandleRegistry,
    PartyMemberBundle, PartyPlugin, recompute_derived_stats_on_equipment_change,
};

// Fixed:
use druum::plugins::party::{
    DerivedStats, EquipSlot, Equipment, EquipmentChangedEvent, Inventory, ItemHandleRegistry,
    PartyMemberBundle, PartyPlugin, recompute_derived_stats_on_equipment_change,
};
```

---

### [MEDIUM] Misleading doc comment on `equip_consumable_returns_item_has_no_slot`

**File:** `src/plugins/party/inventory.rs:654-659`

**Issue:** The test doc says "Because Commands-spawned entities are deferred... `ItemMissingComponents` fires first. Both `ItemHasNoSlot` and `ItemMissingComponents` are valid failure results." But the actual test spawns the entity directly via `world.spawn()` (immediately queryable), and the assertion at line 710 is `assert_eq!(result, Err(EquipError::ItemHasNoSlot))` — only one variant is accepted. The comment describes a setup that no longer exists (it was written when the entity was spawned via `Commands`) and inaccurately implies the assertion is loose.

**Fix:**

```rust
// Replace lines 654-659:
/// An item with `kind: Consumable` (slot: None) is rejected with
/// `Err(ItemHasNoSlot)`. The item entity is spawned directly into the world
/// (immediately queryable), so the consumable check fires at step 4
/// (after resolving the asset) and returns `ItemHasNoSlot` before the
/// character query at step 6 is reached.
#[test]
fn equip_consumable_returns_item_has_no_slot() {
```

---

### [MEDIUM] `unequip_item` helper has zero test coverage

**File:** `src/plugins/party/inventory.rs`

**Issue:** The three helper functions have tests for `equip_item` (slot-none, consumable, slot-mismatch) and `give_item` (happy path), but `unequip_item` is never called in any test. The `equip_sword_raises_attack_unequip_lowers` test exercises the recompute system by directly mutating `Equipment::weapon = None` and emitting an event — it does not call `unequip_item`. The idempotent-empty-slot branch (`if let Some(h) = slot.read(...)... None => return Ok(())`) is untested. The slot-write-to-None and new-entity-spawn paths are similarly uncovered.

**Fix:** Add a Layer-1 or Layer-2 test. Minimal viable:

```rust
#[test]
fn unequip_empty_slot_is_noop_success() {
    let mut world = World::new();
    world.init_resource::<Assets<ItemAsset>>();
    MessageRegistry::register_message::<EquipmentChangedEvent>(&mut world);
    let char_entity = world
        .spawn((Equipment::default(), Inventory::default(), PartyMember))
        .id();

    world
        .run_system_once(
            move |mut commands: Commands,
                  mut char_query: Query<(&mut Equipment, &mut Inventory), With<PartyMember>>,
                  mut writer: MessageWriter<EquipmentChangedEvent>| {
                let result = unequip_item(
                    &mut commands,
                    char_entity,
                    EquipSlot::Weapon,
                    &mut char_query,
                    &mut writer,
                );
                assert_eq!(result, Ok(()), "unequipping empty slot should be Ok");
            },
        )
        .expect("run_system_once must succeed");
}
```

---

### [MEDIUM] Layer-3 test has no timeout guard in `GameState::TitleScreen`

**File:** `tests/equip_from_loaded_items.rs:94-110`

**Issue:** The 30-second timeout is registered only for `GameState::Loading`. After the state transitions to `TitleScreen`, `assert_attack_then_exit` loops indefinitely when `chars.single()` returns `Err`. In normal runs this is not a problem because the entity appears on the first `Update` frame. But if `spawn_test_character_with_loaded_sword` panics mid-execution (e.g., `registry.get("rusty_sword").expect(...)` fails after `Loading` has exited), the entity is never spawned, and the test hangs without a diagnostic timeout or message.

**Fix:** Add a second timeout for `TitleScreen`:

```rust
.add_systems(
    Update,
    timeout_title_screen.run_if(in_state(GameState::TitleScreen)),
)
```

```rust
fn timeout_title_screen(time: Res<Time>) {
    if time.elapsed_secs_f64() > 10.0 {
        panic!(
            "assert_attack_then_exit did not see the test character in 10s — \
             check spawn_test_character_with_loaded_sword"
        );
    }
}
```

---

### [LOW] `equip_item` slot-eviction path has no test

**File:** `src/plugins/party/inventory.rs:302-308`

**Issue:** Step 7 of `equip_item` — "if a previous item occupies the slot, push it back to the bag" — is never exercised by any test. The three Layer-1 tests all either reject early or use an empty slot. The Layer-2 `equip_emits_message_via_helper` starts with an empty `Equipment`. The eviction path spawns a new `ItemInstance` entity and pushes it to `inventory.0`, which is the most structurally complex branch in the helper. Worth covering as a Layer-2 test when test-count headroom permits.

---

### [LOW] `give_item` `CharacterMissingComponents` error path has no test

**File:** `src/plugins/party/inventory.rs:382-398`

**Issue:** The error branch of `give_item` (character entity missing `Inventory`/`PartyMember`) is not tested. The cleanup path — `commands.entity(item_entity).despawn()` before returning `Err` — is the interesting part: it relies on the spawn and despawn being in the same `Commands` buffer. Not flagging for correctness (the logic is right), but the behavior-under-error path has no assertion. The plan listed this as a test to write; it was apparently deferred.

---

## Design-level observations

### Bridge memory footprint (accepted)

`populate_item_handle_registry` clones every `ItemAsset` from `Assets<ItemDb>` into `Assets<ItemAsset>`. For 8 items this is negligible (~8 small POD structs). The doc comment acknowledges this ("revisit if catalog grows past a few hundred"). If the game's item count grows significantly, a shared-reference model would be preferable, but for v1 this is the right call.

### Hot-reload not supported (accepted)

The registry is built once on `OnExit(Loading)` and never refreshed. The doc comment and plan both acknowledge this. The `warn!` in `recompute_derived_stats_on_equipment_change` catches the case where a handle resolves to `None` post-reload. Correct decision for v1.

### `stackable: bool` dead data (accepted)

No system reads `stackable` in v1. Clippy does not flag public struct fields as dead code. The field is intentional forward-compat per the roadmap and plan. Clean per the project's own standards.

### Layer-3 test design: custom `TestItemAssets` vs `LoadingPlugin`

The test bypasses `LoadingPlugin` (which requires `AudioPlugin` → `DefaultPlugins`) and uses a custom `TestItemAssets` collection loading only `Handle<ItemDb>`. The bridge (`populate_item_handle_registry`) is the **production code** — not mocked. The test correctly documents this tradeoff in the module-level doc comment. The risk is that a future change to `LoadingPlugin` (e.g., changing the `OnExit(Loading)` trigger logic) could diverge from what the test exercises. For v1, the test as written is the right pragmatic choice. A refactored `LoadingPlugin` that exposes a `HeadlessLoadingPlugin` would allow a tighter test in future.

### LOC overage

The ~1305-line actual vs ~530-line plan estimate is largely doc comments, implementation-discovery-driven test harness verbosity (World-level `run_system_once` with full boilerplate), and the unplanned `ItemHandleRegistry` bridge. None of this represents scope creep; the doc density matches the project convention set by #11. The `inventory.rs` file at 929 lines is approaching the large-file threshold (800 lines per checklist), but splitting now — before `equip_item` has production callers from #21/#25 — would be premature.

---

## Review Summary

| Severity | Count |
| -------- | ----- |
| CRITICAL | 0     |
| HIGH     | 1     |
| MEDIUM   | 3     |
| LOW      | 2     |

**Verdict: WARNING**

The `cargo fmt` failure is the only hard blocker — it will fail CI if a format check gate exists. All other findings are test-coverage gaps or a misleading doc comment; none affect production correctness.

The core data layer is solid: frozen-file contract honored, `#[derive(Message)]` used correctly throughout, `EquipSlot::read/write` cleanly encapsulates the PascalCase→snake_case mapping, `recompute_derived_stats_on_equipment_change` correctly implements the caller-clamp pattern, and the bridge fix (commit `10002eb`) is the right approach to the `Assets<ItemAsset>` gap that unit tests couldn't catch. Merge after fixing the fmt violation; the MEDIUM issues are good-faith follow-up candidates for the next PR or as review-nit commits.
