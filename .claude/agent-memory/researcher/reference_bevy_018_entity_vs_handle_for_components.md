---
name: Bevy 0.18 — Entity vs Handle vs Relationship for components that reference other things
description: For component fields that reference other entities/assets, Bevy 0.18 has 3 distinct patterns with different save/load and lifetime properties — Entity (raw, dangling-risk, requires MapEntities), Handle<Asset> (serializable as path, no dangling), Relationship trait (canonical for true relations like ChildOf)
type: reference
---

When designing Druum components that reference other ECS data (e.g., `Equipment` slots referencing items, party-formation referencing characters, save-marked-entity referencing world), there are three distinct Bevy 0.18 patterns with very different downstream costs.

**Pattern 1: Raw `Entity` field**

```rust
#[derive(Component, Serialize, Deserialize)]
struct Equipment { weapon: Option<Entity>, ... }
```

Source confirmation: `bevy_ecs-0.18.1/src/entity/mod.rs:45-49` — "Note that this means an Entity id may refer to an entity that has since been despawned!" The dangling-Entity risk is real and documented.

For save/load (Druum Feature #23), this pattern REQUIRES implementing `MapEntities` on the component (`bevy_ecs-0.18.1/src/entity/map_entities.rs:22-55`). The trait remaps source-world entity IDs to destination-world entity IDs during deserialize.

**Pattern 2: `Handle<Asset>` field**

```rust
#[derive(Component, Serialize, Deserialize)]
struct Equipment { weapon: Option<Handle<ItemAsset>>, ... }
```

`Handle<T>` serializes as an `AssetPath` string. No dangling-risk because handle resolution goes through `Assets<T>` which returns `None` for missing assets. No `MapEntities` impl needed for save/load.

Tradeoff: cannot represent per-instance state (enchantment, durability). For per-instance state, layer a separate entity carrying `ItemInstance(Handle<ItemAsset>, Enchantment, ...)`.

**Pattern 3: `Relationship` trait derive**

```rust
#[derive(Component)]
#[relationship(relationship_target = Children)]
struct ChildOf { #[relationship] parent: Entity, internal: u8 }
```

Source: `bevy_ecs-0.18.1/src/relationship/mod.rs:27-77`. This is Bevy's canonical answer for "Component that points to another entity AND maintains the inverse relationship." Auto-maintains a `RelationshipTarget` component on the pointed-to entity via component hooks. The cleanest pattern for true bidirectional relations.

NOT appropriate for one-shot ownership patterns (like Equipment slots) because the inverse `Equipped<By>` collection adds complexity.

**Decision matrix:**

| Pattern | When to use | Save/load cost | Per-instance state cost |
|---------|-------------|----------------|------------------------|
| Raw `Entity` | Dynamic per-instance state mandatory; team commits to MapEntities | High (MapEntities + dangling guards) | Free (entity is already mutable) |
| `Handle<Asset>` | Static asset definitions; per-instance state shallow | Free (serde) | Requires layered `ItemInstance` pattern |
| `Relationship` trait | True bidirectional ownership (parent/child, attacker/target) | Medium (auto-managed but still entity-based) | Free |

**How to apply:** When researching ANY Druum feature that adds a component-with-reference field, surface this matrix. Default-recommend `Handle<Asset>` for asset-static references (Equipment, Spell library, etc.) because save/load is "free." Recommend raw `Entity` only when the team has explicitly committed to `MapEntities` for save/load. Recommend `Relationship` only for bidirectional cases (rare in DRPG outside the existing parent/child).
