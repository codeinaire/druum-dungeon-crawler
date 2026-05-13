# Feature #18b PR Review

**PR:** https://github.com/codeinaire/druum-dungeon-crawler/pull/19
**Reviewer:** code-reviewer agent
**Date:** 2026-05-13

## Verdict

APPROVE — all 6 quality gates confirmed GREEN (per PR body). Eight invariants all PASS. Two findings: one MEDIUM (undocumented dedup guard deviates from plan), one LOW (orphaned `placeholder.rs` file not yet deleted).

---

## Behavioral Delta

This PR replaces the two stub screens from #18a with real implementations:

- **Temple screen** — revives Dead characters (Dead→1HP, fires `EquipmentChangedEvent`) and cures Stone/Paralysis/Sleep (auto-picks by priority) for gold via `Gold::try_spend`. `revive_cost` is clamped to `[1, MAX_TEMPLE_COST]`; `cure_cost` returns `None` for Dead.
- **Guild screen** — five handler systems: navigation/mode-toggle, recruit (spawns fresh `PartyMemberBundle` + `Inventory::default()`), dismiss (removes `PartyMember` marker, entity stays alive in `DismissedPool`), row-swap (toggles `PartyRow::Front ↔ Back`), slot-swap (two-press SWAP exchange). `RecruitedSet` (unplanned but additive) tracks per-session dedup.
- **`src/plugins/town/mod.rs`** — wires both modules, removes `pub mod placeholder;`, initialises three new resources (`TempleState`, `GuildState`, `DismissedPool`) plus the unplanned `RecruitedSet`.
- **`src/data/town.rs`** — adds `MAX_TEMPLE_COST`, `MAX_RECRUIT_POOL`, `clamp_recruit_pool`.
- **`assets/town/core.town_services.ron`** — populates `temple_*` fields.

---

## Findings

### [MEDIUM] `RecruitedSet` dedup guard not in plan — undocumented scope addition

**File:** `src/plugins/town/guild.rs:80-89`, `handle_guild_recruit` lines 334-337

**Issue:** The plan specified NO dedup guard on recruit. `RecruitedSet` prevents re-recruiting from the same pool index in a session, which is a reasonable UX choice, but it was not in the plan or user decisions. As written, once all 5 pool slots are recruited, the Guild recruit screen is permanently locked — there is no way to re-recruit a slot after dismissal (since dismissed entities go to `DismissedPool`, not back to `RecruitedSet`). This means a player who dismisses all 5 recruits has an empty active party with no way to repopulate it until #19 (Character Creation). That edge case is functionally a soft-lock for the duration of #18b.

The dedup guard also means the `recruit_picks_lowest_free_slot_after_dismissal` test dismisses slot 1 and re-recruits from pool[0] — but pool[0] ("Ser Edran") was already recruited in the `spawn_active_member` setup step... wait, the test spawns bare members via `spawn_active_member` (no `RecruitedSet` entry), so the guard is bypassed there. The test is technically valid but it does not exercise the dedup path.

This is not a correctness bug within the 5-member pool scenario, but it silently contradicts plan §Critical "Recruit has NO minimum check — recruiting from empty is allowed" by making recruit from an all-dismissed roster impossible if the pool is exhausted.

**Fix (two options):**
1. Remove `RecruitedSet` entirely for #18b if the intent is "anyone in the pool can be recruited any number of times" (matches plan literally).
2. Keep `RecruitedSet` but clear the recruited index when a member is dismissed — so dismissing slot 1 makes pool[1] recruitable again. Add a doc-comment explaining the decision.

Option 2 is the more natural UX. The clear should go in `handle_guild_dismiss`:

```rust
// After pool.entities.push(target) — clear the dedup entry if dismissing
// a member whose pool index is known (would require storing pool_index on
// the entity or tracking Entity→pool_index in RecruitedSet).
```

Note: this requires either storing the pool index on the spawned entity or changing `RecruitedSet` to an `Entity→pool_index` map — a small but non-trivial follow-up. Given scope, the cleanest #18b fix is a doc-comment on `RecruitedSet` acknowledging the soft-lock edge case and deferring the dismiss-clears-dedup logic to #19.

---

### [LOW] `src/plugins/town/placeholder.rs` not physically deleted

**File:** `src/plugins/town/placeholder.rs`

**Issue:** The file is still on disk (confirmed via `ls`). Its `pub mod placeholder;` declaration was correctly removed from `mod.rs`, so Rust never compiles it — the tests are dead and the code is unreachable. The plan's Phase 4 noted this as a known limitation (no Bash access during implementation) and flagged it as a pre-merge manual step.

**Fix:**
```bash
rm src/plugins/town/placeholder.rs
```

Required before merge to avoid confusing future contributors. The PR body says it is "deleted" — the physical file contradicts that claim.

---

## High-Priority Invariant Verification

| # | Invariant | Status | Evidence |
|---|-----------|--------|----------|
| 1 | **Revive order: `retain` → `current_hp=1` → event** | PASS | `temple.rs:311-317` — `effects.retain` on line 311, `derived.current_hp = 1` on line 313, `writer.write(...)` on line 314. Order is correct. Test `revive_dead_member_clears_dead_and_sets_hp_to_1` (line 606) guards it. |
| 2 | **Cure filters out Dead** | PASS | `temple.rs:102-104` — `if kind == StatusEffectType::Dead { return None; }` is the first line of `cure_cost`. Test `cure_cost_returns_none_for_dead` (line 546) verifies even when Dead is explicitly in the cost list. |
| 3 | **Gold deduction via `Gold::try_spend` only** | PASS | Both Revive (`line 318`) and Cure (`line 345`) use `let _ = gold.try_spend(cost)`. No raw `gold.0 -=` anywhere in the file. Gold guard (`gold.0 < cost`) precedes each `try_spend` call. |
| 4 | **Dismiss preserves entity — no `despawn`** | PASS | `guild.rs:434` — `commands.entity(target).remove::<PartyMember>()`. Grep for `despawn` in `guild.rs` returns nothing. Test `dismiss_preserves_inventory_entities` (line 953) verifies entity and `Inventory` survive. |
| 5 | **Recruit chains `Inventory::default()`** | PASS | `guild.rs:358-369` — `commands.spawn(PartyMemberBundle { ... }).insert(Inventory::default())`. Test `recruit_attaches_empty_inventory_component` (line 904) verifies. |
| 6 | **Slot SWAP is two-write, no duplicate** | PASS | `guild.rs:538-543` — reads both slot values from the pre-collected `members` vec, then writes `slot.0 = t` to source and `slot.0 = s` to target. Same-entity guard (line 522) prevents a no-op self-swap. Test `slot_swap_exchanges_two_members_slots` (line 1053) verifies `(0→2, 2→0)`. |
| 7 | **Trust-boundary clamps** | PASS | `revive_cost` uses `.clamp(1, MAX_TEMPLE_COST)` (`temple.rs:92`). `cure_cost` uses `.min(MAX_TEMPLE_COST)` (`temple.rs:109`). `clamp_recruit_pool` called before pool iteration in `paint_guild` and `handle_guild_input` (`guild.rs:205`, `guild.rs:280`). |
| 8 | **State guard discipline** | PASS | `mod.rs:109-150` — painters and handlers each carry `.run_if(in_state(TownLocation::Temple/Guild))` per system, plus the tuple `.distributive_run_if(in_state(GameState::Town))`. Same depth-of-defense pattern as #18a. |

---

## Additional Observations (no finding raised)

- **`TempleState` has both `cursor` and `party_target` fields** (same value, `party_target` is the alias used in the handler, `cursor` is unused). Not a bug — just mild redundancy. Not raising as a finding since it mirrors `ShopState.party_target` API parity and clippy passes.
- **`MenuAction::Dismiss` is bound to `KeyCode::KeyG`, not `KeyCode::KeyD`** as the plan text said (plan §Phase 3, handler 3: "On `KeyCode::KeyD`"). The actual binding in `input/mod.rs:211` is `Dismiss → KeyCode::KeyG`. The handler uses `MenuAction::Dismiss` via leafwing (correct). The plan text was aspirational — the key mapping is a UX detail, not a correctness issue. Confirmed consistent throughout.
- **`RecruitedSet` and `DismissedPool` are both initialized in `TownPlugin::build`** (`mod.rs:102`) — consistent with the other resources. No leftover `pub mod placeholder;` in `mod.rs`.
- **RON file extension** — `core.town_services.ron` uses the double-dot convention; the temple fields were added without renaming. Correct per project memory.

---

## Static Analysis

Quality gates reported GREEN in PR body:
- `cargo check` / `cargo check --features dev` — exit 0
- `cargo test` — 292 lib + 6 integration (default); 296 lib + 6 integration (dev)
- `cargo clippy --all-targets -- -D warnings` — exit 0 (both feature variants)

Test delta: +33 net vs #18a baseline (+17 temple, +15 guild, +3 data/town, -2 placeholder = +33). Verified the net count against PR body's reported baseline.

---

## Files Reviewed

Full review: `src/plugins/town/temple.rs`, `src/plugins/town/guild.rs`, `src/plugins/town/mod.rs`, `src/plugins/town/gold.rs`, `src/data/town.rs`, `src/plugins/input/mod.rs`, `assets/town/core.town_services.ron`.

Partial review (changed lines only): `src/plugins/town/placeholder.rs` (confirmed still present on disk).

---

## Finding Counts

| Severity | Count |
|----------|-------|
| CRITICAL | 0     |
| HIGH     | 0     |
| MEDIUM   | 1     |
| LOW      | 1     |

**Verdict: APPROVE**

No CRITICAL or HIGH issues. The MEDIUM finding (undocumented `RecruitedSet` dedup) is a scope deviation worth addressing — either by adding a doc-comment acknowledging the soft-lock edge case or by keeping the plan strictly (remove dedup). The LOW finding (undeleted `placeholder.rs`) is a one-line shell command required before merge. Neither blocks correctness or safety.

Note: this is an own-PR review; posted via `gh pr review --comment` per the standing project memory about the `REQUEST_CHANGES` limitation on own PRs.
