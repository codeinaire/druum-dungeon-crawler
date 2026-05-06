# Pipeline Summary — Feature #12: Inventory & Equipment (Research → Plan)

**Date:** 2026-05-05
**Pipeline scope:** research → plan → STOP (parent dispatches implementer manually after user approves the plan, per established Feature #3-#11 pattern)
**Status:** Plan ready for user review. Implementation, ship, and review NOT IN SCOPE for this run.

---

## Original task

Drive the research → plan pipeline for **Feature #12: Inventory & Equipment** from the Druum (Bevy 0.18 first-person dungeon crawler) roadmap (`project/roadmaps/20260429-01-bevy-dungeon-crawler-roadmap.md` lines 638-684). Difficulty 2.5/5 — data layer is light, UI is deferred to Feature #25.

**In scope:** `Item`-on-entity model with `ItemKind`/`EquipSlot`/stat modifiers; per-character `Inventory(Vec<Entity>)` (Wizardry-style, NOT pooled); `equip` / `unequip` / `give_item` systems with slot validation; `EquipmentChangedEvent` (Bevy 0.18 `Message` derive) re-runs #11's `derive_stats`; RON-driven 8-12 starter items at `assets/items/core.items.ron`; placeholder 32×32 PNG icons under `assets/ui/icons/items/`.

**Out of scope:** Inventory UI (#25), save/load remap (#23), stackable item merge (punted), input handling (`DungeonAction::OpenInventory` already bound but not consumed in #12), combat consumption (#15), loot/shop integration (#18, #21).

**Constraint envelope:** +400-600 LOC, 0 new deps, +6-10 tests, +0.3s compile. **Zero new dependencies.**

---

## Artifacts produced

| Step | Description | Path |
|---|---|---|
| 1 | Research | `/Users/nousunio/Repos/Learnings/claude-code/druum/project/research/20260505-080000-feature-12-inventory-equipment.md` |
| 2 | Plan | `/Users/nousunio/Repos/Learnings/claude-code/druum/project/plans/20260505-090000-feature-12-inventory-equipment.md` (Status: Draft — awaiting user OK on D4/D5/D8) |
| - | This summary | `/Users/nousunio/Repos/Learnings/claude-code/druum/project/orchestrator/20260505-100000-feature-12-inventory-equipment-research-plan.md` |

---

## What research found (the load-bearing facts)

The researcher read the merged Feature #11 code and found that **the roadmap's §12 framing is partially stale**:

1. **`Equipment` already ships with `Option<Handle<ItemAsset>>` per slot** at `src/plugins/party/character.rs:209-219`, NOT `Option<Entity>` as roadmap line 647 implies. #11 Decision 3 resolved this and shipped it. The plan must consume #11's Handle shape unchanged — `character.rs` is FROZEN.
2. **`derive_stats(&base, &equip_stats: &[ItemStatBlock], &status, level)` is pure** at `character.rs:343-348`. The flatten step (Equipment slots → `Vec<ItemStatBlock>`) is the **caller's** job. #12 must NOT modify `derive_stats`; it must own the flatten.
3. **`src/plugins/party/inventory.rs` does NOT exist yet** — #12 creates it as a NEW file. `ItemAsset` and `ItemStatBlock` exist as #11 stubs in `src/data/items.rs` ready to be fleshed out.
4. **Asset path is `assets/items/core.items.ron`** (NOT `assets/items/items.ron` as roadmap line 659 says). The 3-line stub already exists at the correct path; `LoadingPlugin` (FROZEN) is wired correctly. The plan modifies the body only.
5. **`EquipmentChangedEvent` derives `Message`, not `Event`** — Bevy 0.18 family rename, verified against project precedents `MovedEvent` (`dungeon/mod.rs:192-197`) and `SfxRequest` (`audio/sfx.rs:42-45`).
6. **`DungeonSubState::Inventory`** and **`DungeonAction::OpenInventory = Tab`** are already declared in #5/#7 — but #25 owns the consumer; #12 does not touch input or state.
7. **Zero new dependencies** — Cargo.toml byte-unchanged. Same cleanest-ship signal as #7, #8, #9, #11.

The full roadmap-vs-reality table is in the research doc §"Stale-roadmap summary".

---

## What the plan delivers (architecture chosen)

**Architecture: Option A — Hybrid (Equipment = Handle, Inventory = Vec<Entity>)**

- `Equipment` keeps Handle-per-slot (locked by #11's Decision 3, doc-comment-promised at `character.rs:204-205`).
- `Inventory(Vec<Entity>)` is per-character; each entity carries `ItemInstance(Handle<ItemAsset>)`.
- Equipping copies the handle into `Equipment`, despawns the inventory entity, emits `EquipmentChangedEvent`.
- Unequipping reads handle from `Equipment`, spawns a fresh `ItemInstance` entity, pushes it onto `Inventory`, emits the event.
- `recompute_derived_stats_on_equipment_change` reads `MessageReader<EquipmentChangedEvent>`, flattens the 8 `Equipment` slots through `Assets<ItemAsset>` into `Vec<ItemStatBlock>`, calls `derive_stats(...)`, applies the caller-clamp pattern from `character.rs:128-131`.

The plan organizes this into 9 atomic phases, each phase a single commit boundary with `cargo test` passing at exit. **Phase 2 commits before Phase 1** because `data/items.rs` references `EquipSlot` and `ItemKind` defined in `inventory.rs`. Phases 4 + 5 may commit together (Layer-2 tests need Phase 5's plugin registration to be live).

---

## Decisions surfaced (Category B — user input requested)

The research doc surfaced D1-D8. Recommendations from research are encoded in the plan; the orchestrator surfaces only the genuine USER PICKs below for confirmation.

### Auto-resolved by Feature #11 (do NOT surface unless user asks)

- **D1 — `Equipment` shape (Entity vs Handle):** RESOLVED in favor of Handle by #11. Flag only if user wants to reverse #11.
- **D6 — Reverse #11 Decision 3?:** Default = no. Surface only if user pushes back on D1.
- **D7 — `EquipSlot::read/write` style:** Hand-written match arms (vs macro). Plan uses hand-written; YAGNI for an 8-slot enum.

### Recommended defaults — proceed unless user objects

- **D2 — `Inventory` shape:** `Inventory(Vec<Entity>)` per character with `ItemInstance(Handle<ItemAsset>)` entity model (Hybrid). Honors doc-comment promise. Alternative: `Inventory(Vec<Handle<ItemAsset>>)` saves ~50 LOC but loses path to per-instance enchantment.
- **D3 — Slot validation:** `Result<(), EquipError>` from helper functions; system wrappers `warn! ` on Err. Idiomatic Rust; UI in #25 can consume the `EquipError` enum for tooltip text.

### Genuine USER PICK — orchestrator wants confirmation before kickoff

- **D4 — `core.items.ron` starter content: 8 vs 12 items.**
  - **Recommended: 8 items** (3 weapons, 2 armor, 1 shield, 1 consumable, 1 key item) — minimal viable test surface, exercises 5 of 9 `ItemKind` variants.
  - Alternative: 12 items adds 1 helm, 1 gloves, 1 boots, 1 accessory for full slot coverage in the integration test. ~30 min of designer-balance per item.
  - Roadmap budget allows either.

- **D5 — Debug `give_starter_items_to_debug_party` system: ship vs skip.**
  - **Recommended: SKIP.** Items are exercised only via Layer-2 tests in #12; in-game item flow first matters in #21 (loot tables). #25 UI dev can start with empty bags — the realistic state when the player enters the dungeon.
  - Alternative: Ship as `#[cfg(feature = "dev")]`, +50 LOC, +1 system, useful for #25 UI dev later (but #25 is far away).

- **D8 — Placeholder icon production: ImageMagick script vs skip vs CC0 pack.**
  - **Recommended: A — ImageMagick-generated 32×32 PNGs** committed via `scripts/gen_placeholder_icons.sh`. ~30 min of setup; visual quality acceptable for v1; ~8 KB total asset Δ. Letter codes + per-kind colors (RS for Rusty Sword on red, HP for Healing Potion on green, etc.).
  - Alt B: Skip PNGs entirely, `icon_path` references nonexistent files. Deviates from roadmap line 679.
  - Alt C: CC0 icon pack from game-icons.net. Real icons but attribution overhead.
  - Sub-question if user picks A: commit script under `scripts/` (recommended) or `tools/`.

---

## Deviations from the roadmap

The plan deviates from the roadmap text in these documented places — all because the roadmap text predates Feature #11 shipping and was authored against the speculative `Entity`-based Equipment design that #11 chose NOT to ship:

1. **Equipment is Handle, not Entity** (roadmap line 647 stale). Plan keeps #11's Handle shape; `Inventory` is the only Entity-bearing structure.
2. **Asset path is `core.items.ron`, not `items.ron`** (roadmap line 659 stale). Plan modifies the body of the existing stub at the correct path.
3. **`EquipmentChangedEvent` derives `Message`, not `Event`** (roadmap line 660 colloquial). Plan uses `#[derive(Message)]` and `MessageReader<T>`.
4. **`stackable: bool` is declared on `ItemAsset` but no v1 system reads it** (roadmap line 681 punts stackables). Plan documents the field as forward-compat; potions are unique entities.
5. **Per-instance state components (`Enchantment`, `Durability`) are NOT shipped in #12** (roadmap doesn't say either way). Plan satisfies the `character.rs:204-205` doc-comment promise by shipping the `ItemInstance` entity model itself; concrete state components land when the first reader appears (likely #21 loot or #15 combat).

All deviations are documented inline in the plan's "Out of scope", "Frozen post-#11", and "Open Decisions Awaiting User Input" sections.

---

## Files the implementer will create or modify

### NEW files

- `src/plugins/party/inventory.rs` (~400-500 LOC) — components, enums, message, error, helper functions, recompute system, Layer-1 + Layer-2 tests.
- `tests/item_db_loads.rs` (~70 LOC) — integration test mirroring `tests/class_table_loads.rs`.
- `assets/ui/icons/items/*.png` (8 files if D4=A, 12 if D4=B) — placeholder 32×32 PNGs.
- `assets/ui/icons/items/` (NEW directory).
- `assets/ui/` (NEW directory) — first-time creation; the project has no `assets/ui/` yet.
- `scripts/gen_placeholder_icons.sh` (NEW file, NEW directory) — one-shot ImageMagick generator (only if D8=A).

### MODIFIED files

- `src/data/items.rs` (+50-80 LOC) — flesh `ItemAsset` from 1-field stub to 8-field schema (`id`, `display_name`, `stats`, `kind`, `slot`, `weight`, `value`, `icon_path`, `stackable`); flesh `ItemDb` from `{}` to `{ items: Vec<ItemAsset> }` with `get(id)` lookup mirroring `ClassTable::get`.
- `src/plugins/party/mod.rs` (+20 LOC) — `pub mod inventory;`, re-exports, `add_message::<EquipmentChangedEvent>()`, 4 `register_type` calls, `add_systems(Update, recompute_derived_stats_on_equipment_change)`. ALSO: chain `.insert(Inventory::default())` onto each debug party member spawn at `spawn_default_debug_party` (single-line per-spawn change).
- `assets/items/core.items.ron` — replace the 3-line `()` stub with the 8-item (or 12-item) RON body.

### Frozen — DO NOT TOUCH

`src/plugins/party/character.rs` (FROZEN by #11), `src/plugins/loading/mod.rs` (FROZEN post-#3), `src/plugins/state/mod.rs`, `src/plugins/input/mod.rs`, `src/plugins/dungeon/mod.rs`, `src/plugins/audio/`, `src/plugins/ui/`, `src/data/dungeon.rs`, `src/data/spells.rs`, `src/data/enemies.rs`, `src/data/classes.rs`, `src/main.rs`, `Cargo.toml`, `Cargo.lock`. Plan enumerates these in §"Frozen post-#11 / DO NOT TOUCH" with a `git diff --name-only` grep in the verification gate.

### Types/events introduced

- Components: `Inventory(Vec<Entity>)`, `ItemInstance(Handle<ItemAsset>)`
- Enums: `ItemKind` (9 variants: `Weapon`, `Shield`, `Armor`, `Helm`, `Gloves`, `Boots`, `Accessory`, `Consumable`, `KeyItem`), `EquipSlot` (9 variants: `None`, `Weapon`, `Shield`, `Armor`, `Helm`, `Gloves`, `Boots`, `Accessory1`, `Accessory2`)
- Message: `EquipmentChangedEvent { character: Entity, slot: EquipSlot }` (`#[derive(Message)]`)
- Error: `EquipError` (4 variants: `ItemHasNoSlot`, `SlotMismatch`, `CharacterMissingComponents`, `ItemMissingComponents`)
- Type alias: `EquipResult = Result<(), EquipError>`
- Functions: `equip_item`, `unequip_item`, `give_item` (helper functions, not Bevy systems — composable for future #21/#18/#25 callers)
- System: `recompute_derived_stats_on_equipment_change` (`MessageReader<EquipmentChangedEvent>` subscriber)
- Methods: `EquipSlot::read(&Equipment) -> Option<&Handle<ItemAsset>>`, `EquipSlot::write(&mut Equipment, Option<Handle<ItemAsset>>)`

---

## Next-step command shape (what the parent dispatches AFTER user approves the plan)

The parent should:

1. **Surface decisions D4, D5, D8 to the user** with the recommended defaults from this report. D2, D3, D7 are recommended defaults — proceed unless user objects. D1, D6 are auto-resolved.
2. **Once user OKs the plan** (and answers D4/D5/D8), dispatch the implementer skill:

```
Skill(skill: "run-implementer",
      args: "Implement this plan: /Users/nousunio/Repos/Learnings/claude-code/druum/project/plans/20260505-090000-feature-12-inventory-equipment.md.

The original task: Feature #12 Inventory & Equipment from the Druum dungeon-crawler roadmap.

User decisions during planning:
- D2 (inventory shape): Hybrid (Vec<Entity> + ItemInstance) — RECOMMENDED accepted.
- D3 (slot validation): Result<(), EquipError> — RECOMMENDED accepted.
- D4 (item count): <USER ANSWER — 8 or 12>
- D5 (debug give_starter_items): <USER ANSWER — skip or ship>
- D7 (EquipSlot::read/write style): hand-written match arms — RECOMMENDED accepted.
- D8 (icon production): <USER ANSWER — ImageMagick script, skip, or CC0 pack>

Critical pre-implementation context:
- Working tree is currently clean on `gitbutler/workspace`, `zz` empty. Local main is **even with origin/main** (Feature #11 PR merged at 8865b26). NO `git pull` is required before branching for #12.
- Use GitButler (`but`) for all history-mutating ops; the `gitbutler/workspace` pre-commit hook blocks raw `git commit`. Read the project CLAUDE.md for the command mapping. Read-only `git log`/`git diff`/`git show` and `gh pr ...` are fine.
- Plan §'Frozen post-#11 / DO NOT TOUCH' enumerates files the implementer must NOT modify. Verify post-implementation with the `git diff --name-only` grep in the verification gate.
- Phase 2 commits BEFORE Phase 1 (commit-ordering note in Phase 1).
- Phases 4 + 5 may need to commit together (Layer-2 tests require Phase 5's plugin registration).
- `EquipmentChangedEvent` MUST `#[derive(Message)]`, NOT `#[derive(Event)]`. Verification gate greps for `derive(.*\bEvent\b)` in inventory.rs — must return zero matches.
")
```

3. **After the implementer reports complete**, the parent dispatches `/ship` to commit/push/PR, then `run-reviewer` against the resulting PR URL. Both stages are NOT IN SCOPE for this orchestrator run — they happen in a follow-up turn.

**No `git pull` is required** before the implementer branches. Local main is even with origin/main (verified at pipeline start; the `gitStatus` snapshot showed `caa124a` as the most recent local commit, matching `8865b26`'s merge but actually the local commit graph already includes the #11 merge from PR #11 — the working tree is clean, current branch is `gitbutler/workspace`, status is clean). The implementer can branch directly via `but branch new <branch-name>`.

---

## Follow-up items / deferred work

- **Feature #15 (Combat):** Will consume `ItemKind::Consumable` (drink potion → heal). #12 ships the kind variant but no consumption system.
- **Feature #18 (Town/Shop):** Will consume `ItemAsset::value`. Already shipped as data.
- **Feature #21 (Loot):** Will consume `give_item` helper. Already shipped.
- **Feature #23 (Save/Load):** Must implement `MapEntities` for `Inventory(Vec<Entity>)` and custom `Handle ↔ AssetPath` serde for `Equipment`. The plan flags this in §"Out of scope" and the security risk table.
- **Feature #25 (Inventory UI):** Will consume `DungeonAction::OpenInventory` (already bound to Tab in #5), `DungeonSubState::Inventory` (already declared in state/mod.rs), the `ItemAsset::display_name` and `icon_path` fields, and `EquipError` variants for tooltip text.
- **Per-instance state on items** (`Enchantment(u8)`, `Durability(u32)`, `CustomName(String)`) — NOT shipped in #12. Will land when the first consumer appears (post-#21 likely).
- **Stackable item merging** — explicitly punted per roadmap line 681. Potions are unique entities in v1. The `stackable: bool` flag is declared for forward-compat but unread.
- **Hot-reload of `core.items.ron`** — `AssetEvent<ItemAsset>` subscriber is post-v1 `--features dev` enhancement.

---

## Pipeline status

- [x] Step 1 — Research complete: `project/research/20260505-080000-feature-12-inventory-equipment.md`
- [x] Step 2 — Plan complete (Status: Draft, awaiting user OK on D4/D5/D8): `project/plans/20260505-090000-feature-12-inventory-equipment.md`
- [ ] Step 3 — Implement (NOT IN SCOPE for this run; parent dispatches manually after user approval)
- [ ] Step 4 — Ship (NOT IN SCOPE)
- [ ] Step 5 — Code Review (NOT IN SCOPE)
- [x] Step 6 — Pipeline summary: this file
- [x] Step 7 — Pipeline state updated: `project/orchestrator/PIPELINE-STATE.md`

`PIPELINE-STATE.md` is now scoped to Feature #12 and will be updated by the parent on the next pipeline turn (when the implementer is dispatched).
