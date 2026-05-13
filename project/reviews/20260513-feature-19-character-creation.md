# Feature #19 — Character Creation & Class Progression: Pre-ship Code Review

**Date:** 2026-05-13
**Verdict:** WARNING
**What was reviewed:** Local changes (no PR) — diff between working tree and `main` for Feature #19.
**Files reviewed:** Full review of all new/modified files in scope.

---

## Severity Counts

| Severity | Count |
| -------- | ----- |
| CRITICAL | 0     |
| HIGH     | 1     |
| MEDIUM   | 2     |
| LOW      | 1     |

---

## HIGH Finding

### [HIGH] `Experience::default()` causes immediate ghost level-up for every recruited character

**File:** `src/plugins/town/guild.rs:423-433`

**Issue:** `handle_guild_recruit` spawns via `PartyMemberBundle { ..Default::default() }`, which leaves `Experience { level: 0, current_xp: 0, xp_to_next_level: 0 }`. On the next frame, `apply_level_up_threshold_system` evaluates `0 >= 0 && 0 < 99` → true, and triggers a level-up from 0 → 1.

The level-up loop does terminate (after 0→1, `xp_to_next_level` becomes 100, and `0 >= 100` is false). But the silent level-up has concrete side effects:
1. `current_hp` and `current_mp` are reset to max at level 0→1, overwriting the `derive_stats(... 1)` values the caller carefully computed.
2. Stats are incremented by `growth_per_level` (e.g., STR+2 for Fighter), contradicting the intent of `final_base` stats computed by the creation wizard.
3. `info!("Guild: recruited...")` shows level and stats before the ghost level-up, so the log is misleading.

Characters recruited via the new `handle_guild_create_confirm` path are pushed to `RecruitPool` and then recruited through `handle_guild_recruit`, so they are equally affected.

**Fix:** Initialize `Experience` with level 1 and the correct `xp_to_next_level` in `handle_guild_recruit`:

```rust
// Current (problematic):
commands
    .spawn(PartyMemberBundle {
        name: CharacterName(recruit.name.clone()),
        race: recruit.race,
        class: recruit.class,
        base_stats: recruit.base_stats,
        derived_stats: derived,
        party_row: recruit.default_row,
        party_slot: PartySlot(slot),
        ..Default::default()  // Experience { level: 0, xp_to_next_level: 0 }
    })
```

```rust
// Fixed:
// Compute the correct xp_to_next_level for level 1.
// Requires class_table access — use Option<Res<TownAssets>> + class_assets
// (both already available in handle_guild_recruit's caller chain),
// or initialize with a sentinel that recompute_xp_to_next_level can fix.
// Simplest defensible fix: add class_assets to handle_guild_recruit's params
// and compute xp_to_next_level from the class def, falling back to u64::MAX
// (which is safe — the threshold system won't fire until XP overflows u64).
let xp_threshold = class_assets
    .get(&assets.class_table)
    .and_then(|t| t.get(recruit.class))
    .map(|def| xp_to_next_level_for(def, 1))
    .unwrap_or(u64::MAX); // safe fallback — prevents premature level-up

commands
    .spawn(PartyMemberBundle {
        name: CharacterName(recruit.name.clone()),
        race: recruit.race,
        class: recruit.class,
        base_stats: recruit.base_stats,
        derived_stats: derived,
        party_row: recruit.default_row,
        party_slot: PartySlot(slot),
        experience: Experience {
            level: 1,
            current_xp: 0,
            xp_to_next_level: xp_threshold,
        },
        ..Default::default()
    })
```

Note: `xp_to_next_level_for` is already exported from `progression.rs` and accessible.

---

## MEDIUM Findings

### [MEDIUM] `recompute_xp_to_next_level` is exported but has no production call sites — the cache can silently stale on class change

**File:** `src/plugins/party/progression.rs:171-184`

**Issue:** `recompute_xp_to_next_level` is a `pub fn` re-exported from `party/mod.rs` with explicit documentation that it "MUST be called whenever `Experience.level` or the character's `Class` changes." But there is no production call site — the function is only defined. `apply_level_up_threshold_system` maintains the cache correctly for level-ups (via `gains.new_xp_to_next_level`), but a class-change system (when implemented) will need to call this manually, and the current state provides a false safety signal through the exported symbol.

The immediate concern is lower-stakes: there is no class-change system in #19. However, the function is advertised as a required invariant-keeper, and it being defined-but-never-called is a latent footgun for feature #21+.

**Fix:** Add a `// No call sites yet — called by class-change system in #21+` doc comment to `recompute_xp_to_next_level`, or add a `#[allow(dead_code)]` attribute if clippy normally catches this. (The fact that clippy passed suggests the re-export suppresses the dead_code lint — but the intent gap remains worth documenting.)

---

### [MEDIUM] `cursor` in `handle_guild_create_input` is not clamped when `race_table`/`class_table` loads between frames

**File:** `src/plugins/town/guild_create.rs:503-517`

**Issue:** `handle_guild_create_input` moves the cursor Down by clamping to `list_len.saturating_sub(1)`. However, when the player presses Confirm in `CreateRace`, `rt.races.get(guild_state.cursor)` is used to select the race (line 524). If the asset hot-reloads between the cursor-move frame and the confirm frame (reducing the race count), the cursor can exceed the new list length and `rt.races.get(guild_state.cursor)` returns `None` — the Confirm is silently swallowed. This is acceptable behavior (the guard handles it gracefully) but the cursor now permanently points past the end of the list until the player moves it.

More concretely: whenever `race_table` finishes loading (first frame after Loading state), the cursor starts at 0 which is valid. If the player rapidly presses Down before the table loads, `list_len` returns 0 (table not ready), `0.saturating_sub(1)` underflows to `usize::MAX`, and `guild_state.cursor = (guild_state.cursor + 1).min(usize::MAX)` — wait, that path requires `list_len > 0` at line 512: `if list_len > 0`. So the guard is already there. Non-issue on Down. On Confirm, `rt.races.get(0)` with empty table returns None — also handled.

Reducing severity: this is already well-guarded. However, the cursor is not reset when re-entering `CreateRace` mode (only reset to 0 when transitioning from `Roster` via `]`). If the user somehow navigates back to CreateRace, the stale cursor is not cleared. The mode transition at line 526 `guild_state.cursor = 0` handles CreateRace → CreateClass, but not Cancel → re-enter. The Cancel path at line 487-492 does reset `guild_state.cursor = 0`, so this is actually fine. Downgrading this to LOW.

**Revised severity: LOW — see LOW findings below.**

---

### [MEDIUM] `compute_xp_from_enemies` sum can wrap on debug-mode in theoretical edge cases

**File:** `src/plugins/combat/turn_manager.rs:605-613`

**Issue:** `.sum::<u32>()` is performed on per-enemy values capped at 1,000,000 each. With more than ~4,295 enemies (4,294,967,295 / 1,000,000), the sum overflows u32. In debug mode Rust panics on integer overflow; in release mode it wraps. The outer `.min(1_000_000)` is applied after the sum and does not prevent overflow.

In the current game design (encounters of ≤ 8 enemies max), the maximum sum is 8,000,000 — well within u32::MAX (~4.3 billion). This is not triggerable with authored encounter tables. However, a crafted encounter file (or future expansion) could trigger it.

**Fix:** Change the sum type to u64, which never overflows in practice:

```rust
// Current:
enemies
    .iter()
    .map(|(d, _)| (d.max_hp / 2).min(1_000_000))
    .sum::<u32>()
    .min(1_000_000)
```

```rust
// Fixed:
enemies
    .iter()
    .map(|(d, _)| ((d.max_hp / 2) as u64).min(1_000_000))
    .sum::<u64>()
    .min(1_000_000) as u32
```

This is a one-line change with zero behavioral impact on realistic encounter sizes.

---

## LOW Findings

### [LOW] `level_up` function receives `_current: &BaseStats` and `_rng` but ignores them — mismatched forward-compatibility signal

**File:** `src/plugins/party/progression.rs:194-218`

**Issue:** Both `_current` and `_rng` are prefixed with `_` to suppress unused-variable warnings. The module doc says "reserved for future stochastic stat growth." This is acceptable today, but the function signature silently accepts a `&mut (impl rand::Rng + ?Sized)` borrow that costs the caller an RNG borrow for no current effect. Consider whether a v1 signature without `rng` is cleaner, or whether the forward-compat argument is strong enough to keep it as-is.

This is a design preference, not a bug — clippy is satisfied by the `_` prefix. No action required before ship.

---

## Pre-ship Fixes Verification

All 7 previously-reported defects are confirmed correctly addressed in the worktree:

1. **`xp_for_level` exponent** — `(target_level - 2) as f64` is correct at line 150. `xp_for_level(2, _)` returns `xp_to_level_2` (exponent = 0). Verified.
2. **`xp_threshold_triggers_level_up` test data** — `current_xp: 120` at line 856, which is in [100, 150) triggering exactly one level-up. Verified.
3. **`use rand` import** — only `SeedableRng` and `rngs::SmallRng` imported; no bare `Rng` import. Verified.
4. **`ALL_RACES` const removed** — not present in `guild_create.rs`. Verified.
5. **Collapsible if** at `handle_guild_create_allocate` — `if let (Some(cd), Some(rt)) = ... && let Some(race) = ... && let Some(base) = ...` chain at line 335. Verified.
6. **`#[allow(clippy::too_many_arguments)]`** on `check_victory_defeat_flee` — line 617. Verified.
7. **Needless `return;`** — not present at the end of `CreateAllocate` arm. Verified.

---

## Correctness Analysis — Key Areas

### XP curve formula (PASS)
`xp_for_level(target_level, class_def)` with `exponent = (target_level - 2) as f64`:
- L2: exponent = 0 → result = base × 1.0 = `xp_to_level_2`. Contract satisfied.
- L99: exponent = 97. With base=100, factor=1.5: ~1.3×10^17 → finite check fails → u64::MAX. Threshold system reads u64::MAX, loop condition never re-triggers at cap. Safe.

### Level-up drain-to-convergence (PASS with caveat)
`while current_xp >= xp_to_next_level && level < level_cap()` terminates because `xp_to_next_level` strictly increases each iteration (monotone curve, factor ≥ 1.0). At cap, `level < level_cap()` goes false. XP accumulates above threshold per Q7=B. Correct — except for the ghost level-up issue noted in HIGH above for newly spawned characters with `xp_to_next_level = 0`.

### `derive_stats` HP reset on level-up (PASS)
Lines 451-456: `derived.current_hp = new_derived.max_hp` (reset to max). This matches the level-up contract described in the module doc. Equipment-change path uses `.min(new_max)` instead — both are correct for their context.

### `handle_guild_create_confirm` re-validation (PASS)
Lines 764-768: `can_create_class(race, &final_base, class_def)` re-called with freshly computed `final_base` (not draft state). MAX_RECRUIT_POOL cap checked at line 785. Race/class validated before stat computation. Defense-in-depth contract from plan is fully satisfied.

### Race i16 encoding round-trip (PASS)
RON file stores 65535 for -1, 65534 for -2. `allocate_bonus_pool` applies via `field as i16` then `saturating_add_signed`. Test `allocate_bonus_applies_race_modifiers_via_saturating_add_signed` exercises this path. RON round-trip test passes. Correct.

### `CombatVictoryEvent` registration (PASS)
`PartyProgressionPlugin::build` calls `app.add_message::<CombatVictoryEvent>()` at line 485. `check_victory_defeat_flee` uses `MessageWriter<CombatVictoryEvent>` which requires prior registration. Registration confirmed present.

### Hobbit LCK+3 balance note (INFO, not a bug)
Hobbit has LCK+3 with sum of modifiers = -2+0-1-1+2+3 = +1 (net positive). All other races sum to 0. This is an intentional design asymmetry for the "lucky" trope — similar to classic Wizardry Hobbits. Not a code defect, but worth noting in the PR body for balance review.

---

## Test Coverage Assessment

25 new tests (vs plan estimate of 10-12). Coverage is meaningful, not padding:
- 3 pure-function unit tests for `xp_for_level` (including overflow, NaN edge cases)
- 2 unit tests for `level_up` (normal and cap)
- 2 unit tests for `roll_bonus_pool` (seeded determinism, range validation)
- 2 unit tests for `allocate_bonus_pool` (overflow rejection, race modifier encoding)
- 3 unit tests for `can_create_class` (min_stats, race, eligible)
- 1 integration test for `apply_level_up_threshold_system` (XP threshold triggers level-up)
- 3 RON/data tests in `races.rs` (round-trip, get, i16 encoding)
- 3 integration tests in `guild_create.rs` (confirm appends, reject below min stats, pool full blocks)
- 1 unit test in `turn_manager.rs` (XP formula verification)
- Plus 5 existing `turn_manager` app_tests that now cover the `CombatVictoryEvent` path indirectly

The `xp_threshold_triggers_level_up` integration test correctly populates TownAssets with the new `race_table` and `class_table` fields — no hidden test-wiring gap (per MEMORY.md feedback on DungeonAssets test wiring).

---

## Scope Notes

No `temple.rs` changes were observed beyond `TownAssets` mock updates (2 new fields). These are correctly scoped to #18b test-wiring updates, not #18b-PR-response fixes. Clean.

`guild.rs` contains the expected #19 additions: `GuildMode` variants, `]`-key entry point, early-return guard in `paint_guild`. No visible #18b-specific leftover hunks that would require split.
