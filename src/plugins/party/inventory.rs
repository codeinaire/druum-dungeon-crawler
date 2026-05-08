//! Inventory & equipment data layer — Feature #12.
//!
//! This module owns the item-entity model and the equipment helper functions
//! that sit on top of the `Equipment` handle-store that Feature #11 shipped
//! in `character.rs:204-219`. The doc-comment promise at `character.rs:204-205`
//! explicitly reserved this space: *"Per-instance state (enchantment, durability,
//! custom name) lands in #12 as a separate `ItemInstance` entity model."*
//!
//! ## Architecture summary
//!
//! - **`ItemKind`** — 9-variant enum that classifies item behaviour (equippable
//!   vs. consumable vs. key item). Stored on `ItemAsset`.
//! - **`EquipSlot`** — 9-variant enum mapping to the 8 `Equipment` fields plus
//!   `None` (sentinel for un-equippable items). `EquipSlot::read` /
//!   `EquipSlot::write` are the *only* places that name `Equipment` fields
//!   directly (pitfall 2 guard).
//! - **`ItemInstance`** — per-entity component wrapping a `Handle<ItemAsset>`.
//!   One entity per bag item. Future per-instance state (enchantment, durability,
//!   custom name) lives as additional components on this entity (#15+).
//! - **`Inventory`** — per-character `Vec<Entity>` component. Wizardry-style:
//!   each party member carries their own bag.
//! - **`EquipmentChangedEvent`** — `Message` emitted by `equip_item` and
//!   `unequip_item` to trigger `recompute_derived_stats_on_equipment_change`.
//! - **`equip_item` / `unequip_item` / `give_item`** — free functions (not Bevy
//!   systems) so they can be called from future callers (#21 loot, #18 shop,
//!   #25 UI). See module-level comment below helper section for the rationale.
//! - **`recompute_derived_stats_on_equipment_change`** — Bevy system subscribed
//!   to `EquipmentChangedEvent`; flattens `Equipment` slots into
//!   `Vec<ItemStatBlock>` and calls `derive_stats`.
//!
//! ## No UI, no save/load
//!
//! UI lives in Feature #25. Save/load (`MapEntities` for `Inventory(Vec<Entity>)`,
//! custom `Handle ↔ AssetPath` serde for `Equipment`) lives in Feature #23.

use std::collections::HashMap;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::data::items::{ItemAsset, ItemDb, ItemStatBlock};
use crate::plugins::party::character::{
    BaseStats, DerivedStats, Equipment, Experience, PartyMember, StatusEffects, derive_stats,
};

// ─────────────────────────────────────────────────────────────────────────────
// ItemKind — 9 variants
// ─────────────────────────────────────────────────────────────────────────────

/// Classifies the behaviour of an item.
///
/// **Validation contract:** `Weapon`, `Shield`, `Armor`, `Helm`, `Gloves`,
/// `Boots`, and `Accessory` are the 7 equippable kinds. Equipping a
/// `Consumable` or `KeyItem` is rejected with `EquipError::ItemHasNoSlot`.
///
/// **Discriminant order is locked** for save-format stability. Same rule as
/// `Class`, `Race`, and `StatusEffectType` in `character.rs`. Never reorder
/// variants.
#[derive(Reflect, Serialize, Deserialize, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ItemKind {
    /// Physical weapons (sword, mace, staff, etc.).
    #[default]
    Weapon,
    /// Off-hand shield.
    Shield,
    /// Body armour.
    Armor,
    /// Head armour.
    Helm,
    /// Hand armour.
    Gloves,
    /// Foot armour.
    Boots,
    /// Ring, amulet, or other accessory.
    Accessory,
    /// One-use item (potion, scroll). Not equippable to a slot.
    Consumable,
    /// Narrative/key item (key, quest item). Not equippable to a slot.
    KeyItem,
}

// ─────────────────────────────────────────────────────────────────────────────
// EquipSlot — 9 variants + read/write helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Maps to one of the eight `Equipment` fields plus `None` (sentinel).
///
/// `EquipSlot::None` is the value stored on `ItemAsset.slot` for
/// `Consumable` and `KeyItem` items — they have no equipment slot.
///
/// **Discriminant order is locked** for save-format stability (same rule as
/// `Class`, `Race`, `StatusEffectType` in `character.rs`). Never reorder
/// variants.
///
/// Only `EquipSlot::read` and `EquipSlot::write` name `Equipment` fields
/// directly (pitfall-2 guard — PascalCase variant vs. snake_case field).
#[derive(Reflect, Serialize, Deserialize, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EquipSlot {
    /// Sentinel — item has no equipment slot (Consumable, KeyItem).
    #[default]
    None,
    Weapon,
    Shield,
    Armor,
    Helm,
    Gloves,
    Boots,
    Accessory1,
    Accessory2,
}

impl EquipSlot {
    /// Return the handle stored in the corresponding `Equipment` field, if any.
    ///
    /// `EquipSlot::None` always returns `None` — there is no field to read.
    /// All callers must go through this method; never reach for `Equipment`
    /// fields by name outside `read`/`write` (pitfall-2 guard).
    pub fn read(self, eq: &Equipment) -> Option<&Handle<ItemAsset>> {
        match self {
            EquipSlot::None => None,
            EquipSlot::Weapon => eq.weapon.as_ref(),
            EquipSlot::Shield => eq.shield.as_ref(),
            EquipSlot::Armor => eq.armor.as_ref(),
            EquipSlot::Helm => eq.helm.as_ref(),
            EquipSlot::Gloves => eq.gloves.as_ref(),
            EquipSlot::Boots => eq.boots.as_ref(),
            EquipSlot::Accessory1 => eq.accessory_1.as_ref(),
            EquipSlot::Accessory2 => eq.accessory_2.as_ref(),
        }
    }

    /// Write `handle` into the corresponding `Equipment` field.
    ///
    /// `EquipSlot::None` is a no-op (defensive — callers must validate before
    /// calling `write`). All callers must go through this method; never write
    /// `Equipment` fields by name outside `read`/`write` (pitfall-2 guard).
    pub fn write(self, eq: &mut Equipment, handle: Option<Handle<ItemAsset>>) {
        match self {
            EquipSlot::None => {} // no-op: no field maps to None
            EquipSlot::Weapon => eq.weapon = handle,
            EquipSlot::Shield => eq.shield = handle,
            EquipSlot::Armor => eq.armor = handle,
            EquipSlot::Helm => eq.helm = handle,
            EquipSlot::Gloves => eq.gloves = handle,
            EquipSlot::Boots => eq.boots = handle,
            EquipSlot::Accessory1 => eq.accessory_1 = handle,
            EquipSlot::Accessory2 => eq.accessory_2 = handle,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ItemInstance component
// ─────────────────────────────────────────────────────────────────────────────

/// Per-entity component wrapping the asset handle for a bag item.
///
/// One entity per inventory item (Wizardry-style — even two copies of the same
/// potion are separate entities). Future per-instance state (enchantment,
/// durability, custom name) arrives as additional components on this entity in
/// #15+.
///
/// **No `Serialize`/`Deserialize`:** `Handle<T>` does not implement serde in
/// Bevy 0.18. Feature #23 (save/load) must bridge `ItemInstance` entities via
/// `MapEntities` + custom asset-path serde, same as the #23 plan for `Equipment`.
/// See `character.rs:196-202` for the same rationale on `Equipment`.
#[derive(Component, Reflect, Default, Debug, Clone, PartialEq)]
pub struct ItemInstance(pub Handle<ItemAsset>);

// ─────────────────────────────────────────────────────────────────────────────
// Inventory component
// ─────────────────────────────────────────────────────────────────────────────

/// Per-character bag — a `Vec<Entity>` of `ItemInstance` entities.
///
/// Wizardry-style: each party member carries their own bag. An entity in this
/// `Vec` carries an `ItemInstance` component.
///
/// **No `Serialize`/`Deserialize`:** Entity IDs are not stable across sessions.
/// Feature #23 must implement `MapEntities` for this component to remap IDs
/// after loading. Tracked in the #12 plan §Out of scope.
///
/// **Feature #23 note:** `Inventory(Vec<Entity>)` deserialization must bound
/// the `Vec` length to guard against crafted save files (security flag from
/// #12 plan §Security).
#[derive(Component, Reflect, Default, Debug, Clone, PartialEq)]
pub struct Inventory(pub Vec<Entity>);

// ─────────────────────────────────────────────────────────────────────────────
// EquipmentChangedEvent — Message (Bevy 0.18 rename)
// ─────────────────────────────────────────────────────────────────────────────

/// Emitted by:
///
/// - `equip_item` and `unequip_item` (`inventory.rs`) — equipment slot
///   changed.
/// - `apply_status_handler` (Feature #14, `combat/status_effects.rs`) when a
///   status change affects derived stats (`AttackUp`/`DefenseUp`/`SpeedUp`/
///   `Dead`). The `slot` field is `EquipSlot::None` in that case (sentinel
///   for "stat-changed, source not an equipment slot").
/// - `tick_status_durations` (#14) when an expiring effect was a stat-
///   modifier — same `EquipSlot::None` sentinel.
///
/// Triggers `recompute_derived_stats_on_equipment_change`, which re-runs
/// `derive_stats` (which sees the new buff branches in #14).
///
/// **`#[derive(Message)]`, NOT `#[derive(Event)]`** — Bevy 0.18 family rename.
/// Use `MessageReader<EquipmentChangedEvent>` to subscribe, and
/// `app.add_message::<EquipmentChangedEvent>()` to register. The canonical
/// project precedent is `MovedEvent` at `dungeon/mod.rs:192-197`.
///
/// The `...Event` suffix is a genre-familiarity convention that matches
/// `MovedEvent`; `SfxRequest` is the counter-example (message without suffix).
/// **Naming impurity acknowledged** (the type also fires for non-equipment
/// stat changes after #14); rename deferred to #25 polish.
#[derive(Message, Clone, Copy, Debug)]
pub struct EquipmentChangedEvent {
    pub character: Entity,
    pub slot: EquipSlot,
}

// ─────────────────────────────────────────────────────────────────────────────
// EquipError + EquipResult
// ─────────────────────────────────────────────────────────────────────────────

/// Why an `equip_item` or `unequip_item` call failed.
///
/// The UI in Feature #25 can use these variants for tooltip text
/// (e.g., "Can't equip a potion into an armor slot").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EquipError {
    /// The item's `slot` is `EquipSlot::None`, OR its `kind` is `Consumable`
    /// or `KeyItem`, OR the caller asked to equip into `EquipSlot::None`.
    ItemHasNoSlot,
    /// The item's `slot` does not match the requested `EquipSlot` (e.g.,
    /// trying to put a sword into the armor slot).
    SlotMismatch,
    /// The character entity is missing `Equipment`, `Inventory`, or both
    /// (not a `PartyMember` entity, or components were stripped).
    CharacterMissingComponents,
    /// The item entity is missing `ItemInstance`, or the asset it points to
    /// is not loaded yet.
    ItemMissingComponents,
}

/// Convenience alias — every helper returns this.
pub type EquipResult = Result<(), EquipError>;

// ─────────────────────────────────────────────────────────────────────────────
// Helper functions — equip_item, unequip_item, give_item
//
// These are FREE FUNCTIONS, not Bevy systems. Bevy systems are consumed by the
// scheduler; free functions can be called from any future caller — #21 loot
// drops, #18 shop, #25 UI drag-and-drop — without scheduling gymnastics.
// The cost is that callers must supply the query borrows; this is idiomatic
// Rust (see research §Pattern 6).
// ─────────────────────────────────────────────────────────────────────────────

/// Move `item_entity` from `character`'s `Inventory` into the `slot` of their
/// `Equipment`. Emits `EquipmentChangedEvent` on success.
///
/// If a previous item occupies `slot`, it is automatically unequipped —
/// a new `ItemInstance` entity is spawned and pushed into `inventory.0`.
/// That new entity is **not queryable in the same frame** (pitfall 1):
/// `Commands::spawn` is deferred until `apply_deferred`.
///
/// Returns `Err(EquipError::...)` on any validation failure; the wrapping
/// Bevy system must `warn!` on `Err`.
///
/// Does NOT emit `EquipmentChangedEvent` on failure.
#[allow(clippy::too_many_arguments)]
pub fn equip_item(
    commands: &mut Commands,
    character: Entity,
    item_entity: Entity,
    slot: EquipSlot,
    items: &Assets<ItemAsset>,
    instances: &Query<&ItemInstance>,
    char_query: &mut Query<(&mut Equipment, &mut Inventory), With<PartyMember>>,
    writer: &mut MessageWriter<EquipmentChangedEvent>,
) -> EquipResult {
    // 1. Reject EquipSlot::None from caller.
    if slot == EquipSlot::None {
        return Err(EquipError::ItemHasNoSlot);
    }

    // 2. Read ItemInstance from the item entity.
    let instance = instances
        .get(item_entity)
        .map_err(|_| EquipError::ItemMissingComponents)?;

    // 3. Resolve the asset.
    let asset = items
        .get(&instance.0)
        .ok_or(EquipError::ItemMissingComponents)?;

    // 4. Reject items that have no equip slot (Consumable, KeyItem, or
    //    EquipSlot::None authored in the asset).
    if asset.slot == EquipSlot::None
        || matches!(asset.kind, ItemKind::Consumable | ItemKind::KeyItem)
    {
        return Err(EquipError::ItemHasNoSlot);
    }

    // 5. Reject slot mismatch (e.g., sword into armor slot).
    if asset.slot != slot {
        return Err(EquipError::SlotMismatch);
    }

    // 6. Get the character's components.
    let (mut equipment, mut inventory) = char_query
        .get_mut(character)
        .map_err(|_| EquipError::CharacterMissingComponents)?;

    // 7. If a previous item occupies the slot, push it back to the bag.
    //    The new entity is NOT queryable this frame (pitfall 1 — Commands
    //    are deferred). Downstream consumers read inventory after apply_deferred.
    if let Some(prev_handle) = slot.read(&equipment).cloned() {
        let evicted = commands.spawn(ItemInstance(prev_handle)).id();
        inventory.0.push(evicted);
    }

    // 8. Write the new handle into the equipment slot.
    slot.write(&mut equipment, Some(instance.0.clone()));

    // 9. Remove item_entity from inventory list.
    inventory.0.retain(|&e| e != item_entity);

    // 10. Despawn the inventory entity (it is now represented by the slot handle).
    commands.entity(item_entity).despawn();

    // 11. Emit the change event.
    writer.write(EquipmentChangedEvent { character, slot });

    Ok(())
}

/// Remove the item from `slot` in `character`'s `Equipment`, moving it back
/// into their `Inventory`. Emits `EquipmentChangedEvent` on a non-empty slot.
///
/// Unequipping an empty slot is a **no-op success** (`Ok(())`). This is
/// intentional: idempotent helpers are safer for UI callers.
///
/// The newly-spawned `ItemInstance` entity is **not queryable in the same
/// frame** (pitfall 1).
pub fn unequip_item(
    commands: &mut Commands,
    character: Entity,
    slot: EquipSlot,
    char_query: &mut Query<(&mut Equipment, &mut Inventory), With<PartyMember>>,
    writer: &mut MessageWriter<EquipmentChangedEvent>,
) -> EquipResult {
    // 1. Reject EquipSlot::None.
    if slot == EquipSlot::None {
        return Err(EquipError::ItemHasNoSlot);
    }

    // 2. Get the character's components.
    let (mut equipment, mut inventory) = char_query
        .get_mut(character)
        .map_err(|_| EquipError::CharacterMissingComponents)?;

    // 3. If the slot is empty, this is a no-op success (idempotent).
    let handle = match slot.read(&equipment).cloned() {
        Some(h) => h,
        None => return Ok(()),
    };

    // 4. Spawn a new inventory entity for the un-equipped item.
    let returned = commands.spawn(ItemInstance(handle)).id();
    inventory.0.push(returned);

    // 5. Clear the slot.
    slot.write(&mut equipment, None);

    // 6. Emit the change event.
    writer.write(EquipmentChangedEvent { character, slot });

    Ok(())
}

/// Add a new `ItemInstance` entity (wrapping `handle`) to `character`'s
/// `Inventory`.
///
/// **Does NOT emit `EquipmentChangedEvent`** — this changes only the inventory
/// bag, not the equipment loadout. Stats do not change until the player equips
/// the item via `equip_item`.
///
/// Future callers: #21 (loot drops), #18 (shop), #25 (UI give-item).
pub fn give_item(
    commands: &mut Commands,
    character: Entity,
    handle: Handle<ItemAsset>,
    char_query: &mut Query<&mut Inventory, With<PartyMember>>,
) -> EquipResult {
    // 1. Spawn the item entity first.
    let item_entity = commands.spawn(ItemInstance(handle)).id();

    // 2. Push onto the character's inventory.
    match char_query.get_mut(character) {
        Ok(mut inventory) => {
            inventory.0.push(item_entity);
        }
        Err(_) => {
            // Cleanup: despawn the just-spawned entity so it doesn't leak.
            commands.entity(item_entity).despawn();
            return Err(EquipError::CharacterMissingComponents);
        }
    }

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// recompute_derived_stats_on_equipment_change — Bevy system
// ─────────────────────────────────────────────────────────────────────────────

/// Re-run `derive_stats` for any character whose equipment changed this frame.
///
/// Subscribed via `app.add_systems(Update, recompute_derived_stats_on_equipment_change)`
/// in `PartyPlugin::build`. Registered in Phase 5.
///
/// **Flatten step:** iterates the 8 `Equipment` slots, resolves each
/// `Option<Handle<ItemAsset>>` via `Res<Assets<ItemAsset>>`, and pushes the
/// resulting `ItemStatBlock` into a `Vec`. Defensive: if an asset is not yet
/// loaded (`items.get(handle)` returns `None`), the slot is **skipped** with a
/// `warn!` — pushing `ItemStatBlock::default()` would silently produce 0 stats
/// and mask the failure (pitfall 3 guard).
///
/// **Caller-clamp pattern** (research §Pattern 7, `character.rs:128-131`):
/// `derive_stats` returns `current_hp = max_hp`. After a re-derive, we clamp
/// `current_hp = old_current_hp.min(new_max_hp)` to avoid instant-refill when
/// equipping a high-VIT item mid-combat.
///
/// **#15 carve-out (D-A5):** the original `With<PartyMember>` filter was
/// dropped so this same recompute system applies to enemy entities as well.
/// Enemies spawn with `Equipment::default()` and `Experience::default()`
/// (see `combat/enemy.rs::EnemyBundle`) so the query shape matches. The
/// flatten step over `Equipment` slots is a no-op for empty equipment,
/// so the system simply re-runs `derive_stats` for any character receiving
/// an `EquipmentChangedEvent` — including buffs/debuffs applied via
/// the `EquipSlot::None` sentinel from #14's `apply_status_handler`
/// (Pitfall 11 of #15).
pub fn recompute_derived_stats_on_equipment_change(
    mut events: MessageReader<EquipmentChangedEvent>,
    items: Res<Assets<ItemAsset>>,
    mut characters: Query<(
        &BaseStats,
        &Equipment,
        &StatusEffects,
        &Experience,
        &mut DerivedStats,
    )>,
) {
    for ev in events.read() {
        let Ok((base, equip, status, xp, mut derived)) = characters.get_mut(ev.character) else {
            // Character may have been despawned between event emission and read.
            // Not a panic — log at trace level and continue.
            continue;
        };

        // Flatten equipped items into a Vec<ItemStatBlock>.
        let mut equip_stats: Vec<ItemStatBlock> = Vec::with_capacity(8);
        let slots: [(&str, Option<&Handle<ItemAsset>>); 8] = [
            ("weapon", equip.weapon.as_ref()),
            ("armor", equip.armor.as_ref()),
            ("shield", equip.shield.as_ref()),
            ("helm", equip.helm.as_ref()),
            ("gloves", equip.gloves.as_ref()),
            ("boots", equip.boots.as_ref()),
            ("accessory_1", equip.accessory_1.as_ref()),
            ("accessory_2", equip.accessory_2.as_ref()),
        ];

        for (slot_name, maybe_handle) in slots {
            if let Some(handle) = maybe_handle {
                match items.get(handle) {
                    Some(asset) => equip_stats.push(asset.stats),
                    None => {
                        // Pitfall 3 guard: asset not yet loaded (or hot-reloaded
                        // away). Skip rather than pushing zero stats.
                        warn!(
                            "ItemAsset not loaded; recompute will produce wrong stats. Slot: {:?}",
                            slot_name
                        );
                    }
                }
            }
        }

        // Re-derive stats.
        let new = derive_stats(base, &equip_stats, status, xp.level);

        // Caller-clamp: preserve current HP/MP within the new max.
        let old_current_hp = derived.current_hp;
        let old_current_mp = derived.current_mp;
        *derived = new;
        derived.current_hp = old_current_hp.min(derived.max_hp);
        derived.current_mp = old_current_mp.min(derived.max_mp);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ItemHandleRegistry — bridge from loaded ItemDb into Assets<ItemAsset>
// ─────────────────────────────────────────────────────────────────────────────

/// Lookup table from item ID (e.g., `"rusty_sword"`) to its `Handle<ItemAsset>`
/// inside `Assets<ItemAsset>`. Populated once on `OnExit(GameState::Loading)`
/// by [`populate_item_handle_registry`]. Read by future systems that mint
/// items at runtime — loot drops (#21), shop purchases (#18), starting gear (#19).
///
/// Exists because `ItemDb` is loaded as one container asset
/// (`Handle<ItemDb>` via `RonAssetPlugin::<ItemDb>` in LoadingPlugin) holding
/// `Vec<ItemAsset>`, while `Equipment` slots are `Option<Handle<ItemAsset>>`
/// (frozen by #11 Decision 3). Without re-inserting each item into
/// `Assets<ItemAsset>`, no production code path can produce a working handle.
///
/// Hot-reload of `core.items.ron` is **not** supported in v1 — the registry
/// is built once on Loading exit and never refreshed.
#[derive(Resource, Default, Debug)]
pub struct ItemHandleRegistry {
    handles: HashMap<String, Handle<ItemAsset>>,
}

impl ItemHandleRegistry {
    /// Look up the asset handle for a given item ID. `None` if the ID does
    /// not exist in any loaded `ItemDb`.
    pub fn get(&self, id: &str) -> Option<&Handle<ItemAsset>> {
        self.handles.get(id)
    }

    /// Number of items in the registry.
    pub fn len(&self) -> usize {
        self.handles.len()
    }

    /// True when the registry holds zero items.
    pub fn is_empty(&self) -> bool {
        self.handles.is_empty()
    }
}

/// Iterate every loaded `ItemDb`, clone each contained `ItemAsset` into
/// `Assets<ItemAsset>`, and record the resulting handle in
/// [`ItemHandleRegistry`] keyed by `ItemAsset::id`.
///
/// Registered on `OnExit(GameState::Loading)` so it runs exactly once after
/// `bevy_asset_loader` reports all collections `LoadedWithDependencies`. The
/// registry is cleared at the start to keep the system idempotent if it ever
/// runs more than once (e.g., a hypothetical Loading-state re-entry).
///
/// Each `ItemAsset` is cloned (cheap — small POD struct). Acceptable for
/// v1 (~10 items); revisit if the catalog grows past a few hundred.
pub fn populate_item_handle_registry(
    item_dbs: Res<Assets<ItemDb>>,
    mut item_assets: ResMut<Assets<ItemAsset>>,
    mut registry: ResMut<ItemHandleRegistry>,
) {
    registry.handles.clear();
    let mut total = 0;
    let mut db_count = 0;
    for (_db_id, item_db) in item_dbs.iter() {
        db_count += 1;
        for item in &item_db.items {
            let handle = item_assets.add(item.clone());
            registry.handles.insert(item.id.clone(), handle);
            total += 1;
        }
    }
    info!(
        "ItemHandleRegistry populated: {} items from {} ItemDb(s)",
        total, db_count
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::message::MessageRegistry;
    use bevy::ecs::system::RunSystemOnce;

    // ── EquipSlot::read/write round-trips ─────────────────────────────────────

    /// For each of the 8 equippable slots, write a weak Handle, then read it
    /// back and assert it is the same value. Verifies the PascalCase→snake_case
    /// mapping in `read`/`write` (pitfall-2 guard).
    #[test]
    fn equip_slot_read_write_round_trip() {
        let handle: Handle<ItemAsset> = Handle::default();
        let slots = [
            EquipSlot::Weapon,
            EquipSlot::Shield,
            EquipSlot::Armor,
            EquipSlot::Helm,
            EquipSlot::Gloves,
            EquipSlot::Boots,
            EquipSlot::Accessory1,
            EquipSlot::Accessory2,
        ];
        for slot in slots {
            let mut eq = Equipment::default();
            slot.write(&mut eq, Some(handle.clone()));
            let read_back = slot.read(&eq).cloned();
            assert_eq!(
                read_back,
                Some(handle.clone()),
                "slot {:?} write→read round-trip failed",
                slot
            );
        }
    }

    /// `EquipSlot::None.read(...)` always returns `None`.
    #[test]
    fn equip_slot_none_read_returns_none() {
        assert_eq!(EquipSlot::None.read(&Equipment::default()), None);
    }

    /// `EquipSlot::None.write(...)` does not modify any field of `Equipment`.
    #[test]
    fn equip_slot_none_write_is_noop() {
        let mut eq = Equipment::default();
        let before = eq.clone();
        EquipSlot::None.write(&mut eq, Some(Handle::default()));
        assert_eq!(
            eq, before,
            "EquipSlot::None.write must not modify Equipment"
        );
    }

    // ── EquipError variants via helper validation ─────────────────────────────

    /// Calling `equip_item` with `slot == EquipSlot::None` returns
    /// `Err(ItemHasNoSlot)` before any asset or entity lookup.
    #[test]
    fn equip_slot_none_from_caller_returns_item_has_no_slot() {
        let mut world = World::new();
        world.init_resource::<Assets<ItemAsset>>();
        // Register the EquipmentChangedEvent resource so MessageWriter resolves.
        MessageRegistry::register_message::<EquipmentChangedEvent>(&mut world);

        // Run inside a one-shot system so we get proper Commands + Queries.
        world
            .run_system_once(
                |mut commands: Commands,
                 items: Res<Assets<ItemAsset>>,
                 instances: Query<&ItemInstance>,
                 mut char_query: Query<(&mut Equipment, &mut Inventory), With<PartyMember>>,
                 mut writer: MessageWriter<EquipmentChangedEvent>| {
                    // Use PLACEHOLDER entities — they will never be found in queries
                    // (that's fine: slot==None check fires before any lookup).
                    let char_entity = Entity::PLACEHOLDER;
                    let item_entity = Entity::PLACEHOLDER;
                    let result = equip_item(
                        &mut commands,
                        char_entity,
                        item_entity,
                        EquipSlot::None, // caller passes None slot — rejected at step 1
                        &items,
                        &instances,
                        &mut char_query,
                        &mut writer,
                    );
                    assert_eq!(result, Err(EquipError::ItemHasNoSlot));
                },
            )
            .expect("run_system_once must succeed");
    }

    /// An item with `kind: Consumable` is rejected. The asset is added directly
    /// (not via Commands) so `instances.get(item_entity)` resolves on the same
    /// system run; the rejection therefore comes from the item's `kind`, not
    /// from a missing `ItemInstance` lookup, and asserts precisely
    /// `Err(ItemHasNoSlot)`.
    #[test]
    fn equip_consumable_returns_item_has_no_slot() {
        let mut world = World::new();
        world.init_resource::<Assets<ItemAsset>>();
        MessageRegistry::register_message::<EquipmentChangedEvent>(&mut world);

        // Add the consumable asset directly (not via Commands) so it's
        // immediately resolvable via Assets::get.
        let consumable_handle = {
            let mut item_assets = world.resource_mut::<Assets<ItemAsset>>();
            item_assets.add(ItemAsset {
                id: "healing_potion".to_string(),
                display_name: "Healing Potion".to_string(),
                kind: ItemKind::Consumable,
                slot: EquipSlot::None,
                stats: ItemStatBlock::default(),
                weight: 0,
                value: 50,
                icon_path: String::new(),
                stackable: false,
                ..Default::default()
            })
        };
        // Spawn the item entity directly so it's queryable via ItemInstance.
        let item_entity = world.spawn(ItemInstance(consumable_handle)).id();

        world
            .run_system_once(
                move |mut commands: Commands,
                      items: Res<Assets<ItemAsset>>,
                      instances: Query<&ItemInstance>,
                      mut char_query: Query<
                    (&mut Equipment, &mut Inventory),
                    With<PartyMember>,
                >,
                      mut writer: MessageWriter<EquipmentChangedEvent>| {
                    // Pass EquipSlot::Weapon (a valid caller slot) so the rejection
                    // comes from the consumable's kind, not the caller's slot.
                    let char_entity = Entity::PLACEHOLDER;
                    let result = equip_item(
                        &mut commands,
                        char_entity,
                        item_entity,
                        EquipSlot::Weapon,
                        &items,
                        &instances,
                        &mut char_query,
                        &mut writer,
                    );
                    // Asset is `Consumable` with `slot: None` → ItemHasNoSlot fires
                    // after resolving ItemInstance but before querying the character.
                    assert_eq!(result, Err(EquipError::ItemHasNoSlot));
                },
            )
            .expect("run_system_once must succeed");
    }

    /// An item with `slot: Weapon` equipping into `EquipSlot::Armor` returns
    /// `Err(SlotMismatch)`.
    #[test]
    fn equip_sword_in_armor_slot_returns_slot_mismatch() {
        let mut world = World::new();
        world.init_resource::<Assets<ItemAsset>>();
        MessageRegistry::register_message::<EquipmentChangedEvent>(&mut world);

        // Add the asset and spawn the entity directly so both are immediately
        // queryable (no deferred Commands flush needed).
        let sword_handle = {
            let mut item_assets = world.resource_mut::<Assets<ItemAsset>>();
            item_assets.add(ItemAsset {
                id: "rusty_sword".to_string(),
                display_name: "Rusty Sword".to_string(),
                kind: ItemKind::Weapon,
                slot: EquipSlot::Weapon,
                stats: ItemStatBlock {
                    attack: 5,
                    ..Default::default()
                },
                weight: 2,
                value: 10,
                icon_path: String::new(),
                stackable: false,
                ..Default::default()
            })
        };
        let item_entity = world.spawn(ItemInstance(sword_handle.clone())).id();

        world
            .run_system_once(
                move |mut commands: Commands,
                      items: Res<Assets<ItemAsset>>,
                      instances: Query<&ItemInstance>,
                      mut char_query: Query<
                    (&mut Equipment, &mut Inventory),
                    With<PartyMember>,
                >,
                      mut writer: MessageWriter<EquipmentChangedEvent>| {
                    // No character entity — but the slot mismatch fires BEFORE the
                    // character query, so CharacterMissingComponents is not reached.
                    let char_entity = Entity::PLACEHOLDER;
                    let result = equip_item(
                        &mut commands,
                        char_entity,
                        item_entity,
                        EquipSlot::Armor, // mismatched: sword belongs in Weapon slot
                        &items,
                        &instances,
                        &mut char_query,
                        &mut writer,
                    );
                    assert_eq!(result, Err(EquipError::SlotMismatch));
                },
            )
            .expect("run_system_once must succeed");
        // sword_handle kept alive in outer scope to prevent asset GC.
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Layer-2 App-level integration tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod app_tests {
    use super::*;
    use crate::plugins::party::{PartyMemberBundle, PartyPlugin};
    use crate::plugins::state::StatePlugin;
    use bevy::asset::AssetPlugin;
    use bevy::ecs::system::RunSystemOnce;
    use bevy::state::app::StatesPlugin;

    fn make_test_app() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            StatesPlugin,
            StatePlugin,
            PartyPlugin,
        ));
        // Register ItemAsset so Assets<ItemAsset> is available in the world.
        // AssetPlugin does NOT auto-register Asset-derived types; each type must
        // be explicitly registered (same pattern as dungeon/tests.rs:161-166).
        app.init_asset::<ItemAsset>();
        // dev feature requires ButtonInput<KeyCode> — only init if dev feature is active.
        #[cfg(feature = "dev")]
        app.init_resource::<ButtonInput<KeyCode>>();
        app
    }

    /// Equipping a sword should increase `DerivedStats::attack`; clearing the
    /// slot and re-deriving should drop it back.
    #[test]
    fn equip_sword_raises_attack_unequip_lowers() {
        let mut app = make_test_app();

        // Seed an ItemAsset for the sword.
        let sword_handle = {
            let mut item_assets = app.world_mut().resource_mut::<Assets<ItemAsset>>();
            item_assets.add(ItemAsset {
                id: "test_sword".to_string(),
                display_name: "Test Sword".to_string(),
                kind: ItemKind::Weapon,
                slot: EquipSlot::Weapon,
                stats: ItemStatBlock {
                    attack: 10,
                    ..Default::default()
                },
                weight: 1,
                value: 1,
                icon_path: String::new(),
                stackable: false,
                ..Default::default()
            })
        };

        // Spawn a character with Inventory.
        let char_entity = app
            .world_mut()
            .spawn((PartyMemberBundle::default(), Inventory::default()))
            .id();

        // Manually set equipment and emit the change event to exercise the
        // recompute system (not the helper functions — those are tested separately).
        app.world_mut()
            .entity_mut(char_entity)
            .get_mut::<Equipment>()
            .unwrap()
            .weapon = Some(sword_handle.clone());

        app.world_mut().write_message(EquipmentChangedEvent {
            character: char_entity,
            slot: EquipSlot::Weapon,
        });

        app.update();

        let derived = *app
            .world()
            .entity(char_entity)
            .get::<DerivedStats>()
            .unwrap();
        assert_eq!(derived.attack, 10, "sword attack should be 10 after equip");

        // Clear the slot and re-emit.
        app.world_mut()
            .entity_mut(char_entity)
            .get_mut::<Equipment>()
            .unwrap()
            .weapon = None;

        app.world_mut().write_message(EquipmentChangedEvent {
            character: char_entity,
            slot: EquipSlot::Weapon,
        });

        app.update();

        let derived2 = *app
            .world()
            .entity(char_entity)
            .get::<DerivedStats>()
            .unwrap();
        assert_eq!(derived2.attack, 0, "attack should drop to 0 after unequip");
    }

    /// `equip_item` helper correctly emits `EquipmentChangedEvent`.
    #[test]
    fn equip_emits_message_via_helper() {
        let mut app = make_test_app();

        // Seed an ItemAsset.
        let sword_handle = {
            let mut item_assets = app.world_mut().resource_mut::<Assets<ItemAsset>>();
            item_assets.add(ItemAsset {
                id: "msg_sword".to_string(),
                display_name: "Msg Sword".to_string(),
                kind: ItemKind::Weapon,
                slot: EquipSlot::Weapon,
                stats: ItemStatBlock::default(),
                weight: 0,
                value: 0,
                icon_path: String::new(),
                stackable: false,
                ..Default::default()
            })
        };

        // Spawn character + item entity directly into the world.
        let item_entity = app
            .world_mut()
            .spawn(ItemInstance(sword_handle.clone()))
            .id();
        let char_entity = app
            .world_mut()
            .spawn((PartyMemberBundle::default(), Inventory(vec![item_entity])))
            .id();

        // Run equip_item inside a one-shot system.
        app.world_mut()
            .run_system_once(
                move |mut commands: Commands,
                      items: Res<Assets<ItemAsset>>,
                      instances: Query<&ItemInstance>,
                      mut char_query: Query<
                    (&mut Equipment, &mut Inventory),
                    With<PartyMember>,
                >,
                      mut writer: MessageWriter<EquipmentChangedEvent>| {
                    let result = equip_item(
                        &mut commands,
                        char_entity,
                        item_entity,
                        EquipSlot::Weapon,
                        &items,
                        &instances,
                        &mut char_query,
                        &mut writer,
                    );
                    assert_eq!(result, Ok(()));
                },
            )
            .expect("run_system_once must succeed");

        // After update, the recompute system runs; the message was consumed.
        // We verify indirectly: no panic, and the equipment slot has the handle.
        app.update();

        let eq = app.world().entity(char_entity).get::<Equipment>().unwrap();
        assert!(
            eq.weapon.is_some(),
            "Equipment::weapon should be populated after equip_item"
        );
    }

    /// `give_item` pushes a new entity into `Inventory`.
    #[test]
    fn give_item_pushes_to_inventory() {
        let mut app = make_test_app();

        let potion_handle = {
            let mut item_assets = app.world_mut().resource_mut::<Assets<ItemAsset>>();
            item_assets.add(ItemAsset {
                id: "give_potion".to_string(),
                display_name: "Give Potion".to_string(),
                kind: ItemKind::Consumable,
                slot: EquipSlot::None,
                stats: ItemStatBlock::default(),
                weight: 0,
                value: 10,
                icon_path: String::new(),
                stackable: false,
                ..Default::default()
            })
        };

        let char_entity = app
            .world_mut()
            .spawn((PartyMemberBundle::default(), Inventory::default()))
            .id();

        app.world_mut()
            .run_system_once(
                move |mut commands: Commands,
                      mut char_query: Query<&mut Inventory, With<PartyMember>>| {
                    let result = give_item(
                        &mut commands,
                        char_entity,
                        potion_handle.clone(),
                        &mut char_query,
                    );
                    assert_eq!(result, Ok(()));
                },
            )
            .expect("run_system_once must succeed");

        // Commands are applied on update.
        app.update();

        // After update, inventory should have 1 entity.
        // NOTE: Commands are applied after the system completes, so the entity
        // is in the Inventory vec after update.
        let inv = app.world().entity(char_entity).get::<Inventory>().unwrap();
        assert_eq!(
            inv.0.len(),
            1,
            "Inventory should have 1 item after give_item"
        );
    }

    /// `unequip_item` helper clears the slot and pushes the handle back to
    /// the bag. Exercises the helper directly — `equip_sword_raises_attack_unequip_lowers`
    /// only mutates `Equipment::weapon = None` in place, so it never covers
    /// `unequip_item`'s body.
    #[test]
    fn unequip_item_helper_returns_to_inventory() {
        let mut app = make_test_app();

        let sword_handle = {
            let mut item_assets = app.world_mut().resource_mut::<Assets<ItemAsset>>();
            item_assets.add(ItemAsset {
                id: "unequip_sword".to_string(),
                display_name: "Unequip Sword".to_string(),
                kind: ItemKind::Weapon,
                slot: EquipSlot::Weapon,
                stats: ItemStatBlock::default(),
                weight: 0,
                value: 0,
                icon_path: String::new(),
                stackable: false,
                ..Default::default()
            })
        };

        // Spawn the character with the sword already equipped, bag empty.
        let char_entity = app
            .world_mut()
            .spawn((
                PartyMemberBundle {
                    equipment: Equipment {
                        weapon: Some(sword_handle.clone()),
                        ..Default::default()
                    },
                    ..Default::default()
                },
                Inventory::default(),
            ))
            .id();

        app.world_mut()
            .run_system_once(
                move |mut commands: Commands,
                      mut char_query: Query<
                    (&mut Equipment, &mut Inventory),
                    With<PartyMember>,
                >,
                      mut writer: MessageWriter<EquipmentChangedEvent>| {
                    let result = unequip_item(
                        &mut commands,
                        char_entity,
                        EquipSlot::Weapon,
                        &mut char_query,
                        &mut writer,
                    );
                    assert_eq!(result, Ok(()));
                },
            )
            .expect("run_system_once must succeed");

        // Commands flush on update — that's when the new ItemInstance entity
        // becomes visible in the Inventory vec.
        app.update();

        let eq = app.world().entity(char_entity).get::<Equipment>().unwrap();
        assert!(
            eq.weapon.is_none(),
            "Equipment::weapon should be None after unequip_item"
        );
        let inv = app.world().entity(char_entity).get::<Inventory>().unwrap();
        assert_eq!(
            inv.0.len(),
            1,
            "Inventory should hold the un-equipped item (1 entity), got {}",
            inv.0.len()
        );

        // The new bag entity should carry the original sword handle.
        let returned = inv.0[0];
        let returned_instance = app
            .world()
            .entity(returned)
            .get::<ItemInstance>()
            .expect("returned entity should carry an ItemInstance");
        assert_eq!(
            returned_instance.0, sword_handle,
            "returned ItemInstance should hold the original sword handle"
        );
    }

    /// Equipping a second weapon while a first is already in the slot must
    /// evict the previous occupant back to the bag — covers step 7 of
    /// `equip_item`. `equip_emits_message_via_helper` starts with an empty
    /// slot, so it never exercises the eviction path.
    #[test]
    fn equip_item_evicts_previous_slot_occupant_to_inventory() {
        let mut app = make_test_app();

        let first_handle = {
            let mut item_assets = app.world_mut().resource_mut::<Assets<ItemAsset>>();
            item_assets.add(ItemAsset {
                id: "first_sword".to_string(),
                display_name: "First Sword".to_string(),
                kind: ItemKind::Weapon,
                slot: EquipSlot::Weapon,
                stats: ItemStatBlock::default(),
                weight: 0,
                value: 0,
                icon_path: String::new(),
                stackable: false,
                ..Default::default()
            })
        };
        let second_handle = {
            let mut item_assets = app.world_mut().resource_mut::<Assets<ItemAsset>>();
            item_assets.add(ItemAsset {
                id: "second_sword".to_string(),
                display_name: "Second Sword".to_string(),
                kind: ItemKind::Weapon,
                slot: EquipSlot::Weapon,
                stats: ItemStatBlock::default(),
                weight: 0,
                value: 0,
                icon_path: String::new(),
                stackable: false,
                ..Default::default()
            })
        };

        // Spawn: first sword equipped; second sword sits as an inventory entity
        // ready to be equipped.
        let second_item_entity = app
            .world_mut()
            .spawn(ItemInstance(second_handle.clone()))
            .id();
        let char_entity = app
            .world_mut()
            .spawn((
                PartyMemberBundle {
                    equipment: Equipment {
                        weapon: Some(first_handle.clone()),
                        ..Default::default()
                    },
                    ..Default::default()
                },
                Inventory(vec![second_item_entity]),
            ))
            .id();

        app.world_mut()
            .run_system_once(
                move |mut commands: Commands,
                      items: Res<Assets<ItemAsset>>,
                      instances: Query<&ItemInstance>,
                      mut char_query: Query<
                    (&mut Equipment, &mut Inventory),
                    With<PartyMember>,
                >,
                      mut writer: MessageWriter<EquipmentChangedEvent>| {
                    let result = equip_item(
                        &mut commands,
                        char_entity,
                        second_item_entity,
                        EquipSlot::Weapon,
                        &items,
                        &instances,
                        &mut char_query,
                        &mut writer,
                    );
                    assert_eq!(result, Ok(()));
                },
            )
            .expect("run_system_once must succeed");

        app.update();

        // After equip: weapon slot holds the new handle; the bag holds exactly
        // one entity — the evicted previous occupant. The original
        // second_item_entity has been despawned (consumed by the slot).
        let eq = app.world().entity(char_entity).get::<Equipment>().unwrap();
        assert_eq!(
            eq.weapon.as_ref(),
            Some(&second_handle),
            "Equipment::weapon should hold the second (newly-equipped) handle"
        );
        let inv = app.world().entity(char_entity).get::<Inventory>().unwrap();
        assert_eq!(
            inv.0.len(),
            1,
            "Inventory should hold exactly the evicted previous occupant, got {}",
            inv.0.len()
        );

        let evicted = inv.0[0];
        let evicted_instance = app
            .world()
            .entity(evicted)
            .get::<ItemInstance>()
            .expect("evicted entity should carry an ItemInstance");
        assert_eq!(
            evicted_instance.0, first_handle,
            "evicted ItemInstance should hold the previous (first) sword handle"
        );
    }

    /// Calling `give_item` against an entity that lacks `Inventory` /
    /// `PartyMember` must return `Err(CharacterMissingComponents)` AND
    /// despawn the just-spawned `ItemInstance` so it doesn't leak.
    #[test]
    fn give_item_with_missing_character_cleans_up_spawned_entity() {
        let mut app = make_test_app();

        let potion_handle = {
            let mut item_assets = app.world_mut().resource_mut::<Assets<ItemAsset>>();
            item_assets.add(ItemAsset {
                id: "leak_potion".to_string(),
                display_name: "Leak Potion".to_string(),
                kind: ItemKind::Consumable,
                slot: EquipSlot::None,
                stats: ItemStatBlock::default(),
                weight: 0,
                value: 0,
                icon_path: String::new(),
                stackable: false,
                ..Default::default()
            })
        };

        // Spawn an entity WITHOUT Inventory or PartyMember — char_query will
        // fail to resolve it, triggering the cleanup branch.
        let bogus_entity = app.world_mut().spawn_empty().id();

        // Pre-condition: zero ItemInstance entities in the world.
        assert_eq!(
            app.world_mut()
                .query::<&ItemInstance>()
                .iter(app.world())
                .count(),
            0,
            "world should start with zero ItemInstance entities"
        );

        app.world_mut()
            .run_system_once(
                move |mut commands: Commands,
                      mut char_query: Query<&mut Inventory, With<PartyMember>>| {
                    let result = give_item(
                        &mut commands,
                        bogus_entity,
                        potion_handle.clone(),
                        &mut char_query,
                    );
                    assert_eq!(result, Err(EquipError::CharacterMissingComponents));
                },
            )
            .expect("run_system_once must succeed");

        // Both the spawn and the cleanup despawn are queued as Commands; one
        // update flushes them. The spawn-then-despawn must net to zero.
        app.update();

        assert_eq!(
            app.world_mut()
                .query::<&ItemInstance>()
                .iter(app.world())
                .count(),
            0,
            "ItemInstance entity must be cleaned up when give_item fails — \
             leak indicates the cleanup despawn in give_item was skipped"
        );
    }
}
