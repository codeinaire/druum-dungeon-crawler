# Feature #18a PR Review

**PR:** https://github.com/codeinaire/druum-dungeon-crawler/pull/18
**Reviewer:** code-reviewer agent
**Date:** 2026-05-11

## Verdict

APPROVE — all 6 quality gates confirmed GREEN locally. One MEDIUM finding (silent zero-gold sell on unloaded asset in handler) and one LOW finding (cursor state not reset on Town re-entry). No CRITICAL or HIGH issues.

## Finding Counts

- CRITICAL: 0
- HIGH: 0
- MEDIUM: 1
- LOW: 1

---

## Findings

### [MEDIUM] Sell handler silently removes item for 0 gold when asset is unloaded

**File:** `src/plugins/town/shop.rs` line ~465

**Issue:** In the `ShopMode::Sell` branch of `handle_shop_input`, when the `ItemInstance`'s asset handle resolves to `None` (asset unloaded), the code falls back to `unwrap_or(0)` and continues — removing the item from inventory and despawning the entity while the player receives 0 gold. The exported `sell_item` free function correctly returns `Err(SellError::ItemAssetMissing)` in this case, but the inlined handler path diverges silently.

In practice this requires an asset to be unloaded after being added to an inventory (extremely unlikely in the current asset pipeline), but the behavioral divergence between the free function and the handler is a correctness risk worth closing.

```rust
// shop.rs line 464-465 — handler path
let sell_price = match instances.get(item_entity) {
    Ok(instance) => item_assets.get(&instance.0).map(|a| a.value / 2).unwrap_or(0), // ← silently 0
    Err(_) => { ... return; }
};
// then falls through to: inventory.0.retain, commands.entity(item_ent).despawn(), gold.earn(0)
```

**Fix:** Guard the missing-asset case the same way `sell_item` does — return early without removing the item:

```rust
let sell_price = match instances.get(item_entity) {
    Ok(instance) => {
        match item_assets.get(&instance.0) {
            Some(asset) => asset.value / 2,
            None => {
                info!("shop sell: asset for item {:?} not loaded, aborting sell", item_entity);
                return;
            }
        }
    }
    Err(_) => {
        info!("shop sell: item entity {:?} not found", item_entity);
        return;
    }
};
```

---

### [LOW] Shop/Square cursor state persists across Town visits without reset

**File:** `src/plugins/town/mod.rs` (TownPlugin::build, no OnEnter reset for cursor resources)

**Issue:** `SquareMenuState`, `ShopState`, and `InnState` are initialized once (`init_resource`) and never reset on `OnEnter(GameState::Town)`. This is typical Wizardry cursor behavior (player returns to where they were), but it means if a player leaves Town at cursor=3 on the Shop list, returns to Town, navigates to the Square without first visiting the Shop, and an NPC event had modified the shop stock to fewer items, the cursor is already out of bounds until it's moved. The cursor-out-of-bounds case is handled safely (`nth(cursor)` returns `None`, early return), so no panic or incorrect purchase occurs — just an invisible cursor until the player presses Up/Down.

This is a cosmetic/UX note, not a correctness bug. If the intent is to always land at cursor=0 when re-entering Town, add an `OnEnter(GameState::Town)` system that resets the cursors.

---

## High-Priority Checks (all pass)

1. **Gold underflow invariant** — `Gold::try_spend` returns `Err(SpendError::InsufficientGold)` before any subtraction (`self.0 < amount` check precedes `saturating_sub`). Saturating arithmetic is defense-in-depth. CORRECT.

2. **Give-first-deduct-second (buy path)** — Both the free-function `buy_item` (steps 5→6) and the inlined handler path explicitly spawn the `ItemInstance`, push to inventory, and only then call `gold.try_spend`. On `get_mut` failure: entity is despawned, gold is NOT deducted. CORRECT.

3. **Inventory cap BEFORE gold check** — Step 3 (`inv_len >= MAX_INVENTORY_PER_CHARACTER`) returns `BuyError::InventoryFull` before step 4 (`gold.0 < price`). Both the free function and the handler match this order. CORRECT.

4. **Inn skips Dead members** — `if status.has(StatusEffectType::Dead) { continue; }` is the first check in the party iteration loop. Dead member HP is not touched. The `rest_skips_dead_party_member` test covers this. CORRECT.

5. **Clock advance / gold deduction order in Inn** — Gold is checked before iteration (step 2), party is healed (steps 3-4), gold deducted after the loop (step 5), then `next_sub.set(Square)` (step 6). No reversal of order. CORRECT.

6. **PrimaryEguiContext lifecycle** — `Camera2d`, `TownCameraRoot`, and `PrimaryEguiContext` are spawned in one `commands.spawn(...)` call. `despawn_town_camera` queries `With<TownCameraRoot>` and calls `.despawn()` (which is recursive in Bevy 0.18). Atomic cleanup, no orphan context. CORRECT.

7. **State guard discipline** — All painters in `EguiPrimaryContextPass` use `.distributive_run_if(in_state(GameState::Town))` on the tuple plus `.run_if(in_state(TownLocation::X))` per system. Handlers in `Update` mirror the same pattern. CORRECT.

8. **RON trust-boundary clamps** — `clamp_shop_stock(stock, MAX_SHOP_ITEMS=99)` is called in the painter before iterating. `services.inn_rest_cost.min(MAX_INN_COST=10_000)` is applied in the handler before the gold check. Both are also covered by unit tests. CORRECT.

9. **`sell_item` free function correctness** — The exported helper correctly returns `Err(SellError::ItemEntityMissing)` when the item is not in inventory (checked via `retain` length comparison) and `Err(SellError::ItemAssetMissing)` when asset is unloaded. CORRECT (the handler's divergence is captured in the MEDIUM finding above).

10. **Zero new Cargo dependencies** — Confirmed. `Cargo.toml` is not in the changed files. CORRECT.

---

## Static Analysis

All 6 quality gates run locally and confirmed GREEN:
- `cargo check` — exit 0
- `cargo check --features dev` — exit 0
- `cargo test` — 260 lib + 6 integration tests pass (35 new town tests all green)
- `cargo test --features dev` — 264 lib + 6 integration tests pass
- `cargo clippy --all-targets -- -D warnings` — exit 0
- `cargo clippy --all-targets --features dev -- -D warnings` — exit 0

Note: The implementation summary listed gates as "NOT RUN" (shell tool unavailable during implementation), but they were verified GREEN by the orchestrator pipeline before PR submission and confirmed by this reviewer locally.

---

## Files Reviewed

Full review: `src/plugins/town/mod.rs`, `src/plugins/town/gold.rs`, `src/plugins/town/shop.rs`, `src/plugins/town/inn.rs`, `src/plugins/town/square.rs`, `src/plugins/town/placeholder.rs`, `src/data/town.rs`, `src/data/mod.rs`, `src/plugins/loading/mod.rs`, `assets/town/shop_stock.ron`, `assets/town/recruit_pool.ron`, `assets/town/town_services.ron`.

Partial review (changed lines + immediate context): implementer memory files in `.claude/agent-memory/`.
