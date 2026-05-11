---
name: B0002 buy/sell — inline validation in handler, free functions remain for future callers
description: When a free function needs Query<&mut T> but the calling handler already holds Query<(Entity, &mut T)>, inline the logic in the handler to avoid B0002
type: feedback
---

Bevy B0002 fires when the same system borrows `&mut T` via two different `Query` params. A free function like `buy_item(char_query: &mut Query<&mut Inventory, ...>)` cannot be called from a handler that already holds `Query<(Entity, &mut Inventory), ...>` because the two queries both request `&mut Inventory` — even though they're in the same system, Bevy's borrow checker sees two mutable borrows on the same archetype.

**Fix:** Inline the free function's validation logic directly in the handler using the existing combined query. The free functions remain exported for future callers (loot drops, future UI systems) that can supply the narrower query from their own system context.

```rust
// In handle_shop_input:
// CANNOT do: buy_item(&mut commands, char_entity, &item_id, &mut gold, &registry, &item_assets, &mut char_query)
// BECAUSE: char_query here is Query<(Entity, &mut Inventory), ...> 
//           but buy_item wants Query<&mut Inventory, ...>

// DO: inline the same validation steps:
let inv_len = char_query.get(char_entity).map(|(_, inv)| inv.0.len()).unwrap_or(MAX_INVENTORY_PER_CHARACTER);
if inv_len >= MAX_INVENTORY_PER_CHARACTER { return; }
// ... rest of inline buy logic ...
```

**Why:** Bevy's B0002 check is conservative — it sees `Query<(Entity, &mut Inventory)>` and `Query<&mut Inventory>` as conflicting even in the same system because both request mutable access to `Inventory` components.

**How to apply:** When you see a free function that takes `Query<&mut T>` being called from a system that holds `Query<(..., &mut T, ...)>`, inline the logic in the handler. Document the inlining with a comment referencing B0002.
