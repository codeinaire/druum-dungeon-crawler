# Pipeline State

**Task:** Drive research → plan pipeline (PAUSE at plan-approval) for Feature #12: Inventory & Equipment from the Druum Bevy 0.18 dungeon-crawler roadmap. Roadmap §12 at lines 638-684. Per-character `Inventory(Vec<Entity>)` Wizardry-style (NOT pooled `PartyInventory`); `Item` component on item entities with `ItemKind`/`EquipSlot`/stats/weight/value/stackable; `equip(actor,item)`/`unequip(actor,slot)`/`give_item(actor,item)` with slot validation; `EquipmentChangedEvent` (Bevy 0.18 `#[derive(Message)]`, `MessageReader<T>`) re-runs #11's pure `derive_stats`; RON-driven 8-12 starter items at `assets/items/core.items.ron`; 5-10 placeholder 32x32 PNG icons at `assets/ui/icons/items/`; +400-600 LOC; **0 new deps**; +6-10 tests. #11 dependency satisfied at origin/main (8865b26, PR #11). Stackable items punted (potions are unique entities for now); inventory UI deferred to #25; save/load remap deferred to #23. Final report at plan-approval MUST be self-contained because `SendMessage` does not actually resume returned agents (confirmed across Features #3-#11); parent dispatches implementer manually after approval.

**Status:** research+plan complete — awaiting user approval before implementer dispatch
**Last Completed Step:** 2 (plan complete; Status: Draft)

## Artifacts

| Step | Description      | Artifact                                                                                                            |
| ---- | ---------------- | ------------------------------------------------------------------------------------------------------------------- |
| 1    | Research         | /Users/nousunio/Repos/Learnings/claude-code/druum/project/research/20260505-080000-feature-12-inventory-equipment.md |
| 2    | Plan             | /Users/nousunio/Repos/Learnings/claude-code/druum/project/plans/20260505-090000-feature-12-inventory-equipment.md (Status: Draft — awaiting D4/D5/D8) |
| 6    | Pipeline summary | /Users/nousunio/Repos/Learnings/claude-code/druum/project/orchestrator/20260505-100000-feature-12-inventory-equipment-research-plan.md |
| 3    | Implement        | NOT IN SCOPE (parent dispatches after plan approval)                                                                |
| 4    | Ship             | NOT IN SCOPE                                                                                                        |
| 5    | Code Review      | NOT IN SCOPE                                                                                                        |

## User Decisions

(none yet — Category B decisions D1-D5 + research-discovered D6-D8 will be surfaced after plan lands)

## Pipeline Scope

This invocation runs research → plan → STOP. After plan approval, parent will manually dispatch implementer (per established Feature #3-#11 pattern). The orchestrator pipeline summary at the end of this run must be self-contained.

## Critical context for resumption

- **Live #11 ground truth (from research):** `Equipment` already stores `Option<Handle<ItemAsset>>` per slot (NOT `Entity` as roadmap line 647 implies — #11 Decision 3 resolved this). `derive_stats(&base, &equip_stats: &[ItemStatBlock], &status, level)` is pure and takes a flattened slice — caller owns the flatten. `inventory.rs` does NOT exist yet (NEW file). `ItemAsset` and `ItemStatBlock` exist as #11 stubs in `src/data/items.rs` ready to be fleshed out. RON loader path is `assets/items/core.items.ron` (NOT `items.ron` as roadmap line 659 says). `LoadingPlugin` is FROZEN. `EquipmentChangedEvent` derives `Message` (verified against project precedents `MovedEvent`, `SfxRequest`).
- **Recommended architecture (Option A — Hybrid):** `Equipment` keeps Handle slots (#11 shape, frozen); `Inventory(Vec<Entity>)` per-character with `ItemInstance(Handle<ItemAsset>)` on each entity. Equipped = handle, in-bag = entity.
- **Decisions D1-D8 surfaced in research:** D1 (Equipment Entity vs Handle) AUTO-RESOLVED by #11 — surface only if user asks to revert. D2 (Inventory shape — Hybrid recommended). D3 (Slot validation — `Result<(), EquipError>` recommended). D4 (8-12 starter items breakdown). D5 (debug `give_starter_items` — SKIP recommended). D6 (reverse #11 Decision 3? — surface only if user pushes back on D1). D7 (`EquipSlot` ↔ `Equipment` mapping style — hand-written match arms). D8 (placeholder icon production — ImageMagick script).
- **GitButler discipline:** implementer + shipper must use `but` not `git` (pre-commit hook on `gitbutler/workspace` blocks raw `git commit`). Working tree currently clean, on `gitbutler/workspace`, `zz` empty, local main even with origin/main.
