# Pipeline State — Feature #19 final

**Task:** Feature #19 — Character Creation & Class Progression. Bevy 0.18.1 first-person dungeon-crawler RPG (Druum). Guild character-creation wizard (race → class → roll/allocate stats → name → confirm), leveling/XP system in new `src/plugins/party/progression.rs`, class-change data (UI deferred), new `assets/races/core.races.racelist.ron`, extended `core.classes.ron`, combat-XP hook in `turn_manager.rs`. MVP class roster: Fighter / Mage / Priest. Roadmap source: `project/roadmaps/20260429-01-bevy-dungeon-crawler-roadmap.md` §19.

**Status:** ✅ shipped
**Branch:** `feature/19-character-creation`
**Commit:** `228c459`
**PR:** https://github.com/codeinaire/druum-dungeon-crawler/pull/20

## Artifacts

| Step | Description | Artifact |
| ---- | ----------- | -------- |
| 1 | Research    | `project/research/20260513-feature-19-character-creation.md` |
| 2 | Plan        | `project/plans/20260513-120000-feature-19-character-creation.md` |
| 3 | Implement   | `project/implemented/20260513-feature-19-character-creation.md` |
| 4 | Ship        | `project/shipper/feature-19-commit-msg.txt` + `feature-19-pr-body.md` → PR #20 |
| 5 | Code Review | `project/reviews/20260513-feature-19-character-creation.md` (1 HIGH + 2 MEDIUM fixed; 1 LOW deferred) |
| 6 | Summary     | `project/orchestrator/20260513-133936-feature-19-character-creation.md` |

## Final metrics

- **Files changed:** 26 (commit `228c459`)
- **LOC delta:** +4,921 / −25 (source ≈ 2,585; docs/artifacts ≈ 1,900; agent-memory ≈ 100)
  - vs research estimate ~835 / plan refinement ~570: source LOC came in materially higher, driven by `guild_create.rs` (1,204) and `progression.rs` (887, includes ~400+ test LOC)
- **Tests:** 322 lib + 1 integration (pre-#19 baseline ~296 → **+26**, above the +10–12 plan target)
- **Δ Cargo.toml:** 0
- **Quality gates (6/6 green):** `cargo check` ±dev, `cargo test` ±dev, `cargo clippy --all-targets ±dev -- -D warnings`

## User decisions (resolved before plan step)

Preset: "All recommendations (lightest MVP path)".

| Q | Decision | Pick |
|---|---|---|
| Q1 | Stat allocation flavor | 1B Bonus-pool with re-roll |
| Q2 | MVP class roster | 2A Fighter / Mage / Priest |
| Q3 | Race set day-one | 3A All 5 (Human / Elf / Dwarf / Gnome / Hobbit) |
| Q4 | Creation destination on Confirm | 4A Push to `RecruitPool`, auto-switch to Recruit |
| Q5 | XP curve | 5A Per-class formula `xp_to_level_2 * curve_factor^(target_level-2)` |
| Q6 | Class-change stat penalty | 6C None (data-only day-one) |
| Q7 | Level cap | 7B 99 |

## Fixes applied across the run

**Main-session pre-review fixes (post-implementation, before reviewer ran)** — 7 defects caught by local quality gates:
1. XP-curve formula off-by-one in `xp_for_level` (exponent `target_level-1` → `target_level-2`; field-name contract `xp_to_level_2` must equal `xp_for_level(2)`)
2. Stale test data in `xp_threshold_triggers_level_up` (current_xp 150 → 120 to land in [L2-thresh, L3-thresh))
3. Unused `Rng` import in `progression.rs`
4. Dead `ALL_RACES` const in `guild_create.rs` (race iteration is data-driven via `race_table.races.iter()`)
5. Clippy `collapsible_if` in `guild_create.rs`
6. Clippy `too_many_arguments` on `check_victory_defeat_flee` (`#[allow]` — canonical Bevy pattern; 5 prior instances in this codebase)
7. Clippy `needless_return` in `guild_create.rs`

**Reviewer-flagged fixes (post-review, pre-ship)** — 3 issues:
- **HIGH** Ghost level-up on recruit — `Experience::default()` zero-inits `xp_to_next_level`, drain loop sees `0>=0` and ghost-levels every recruited character. Fixed in `handle_guild_recruit` by initializing `Experience { level: 1, current_xp: 0, xp_to_next_level: xp_to_next_level_for(class_def, 1) }` explicitly. New param `class_assets: Res<Assets<ClassTable>>`. Refuses recruit with toast if asset isn't loaded. New regression test `recruit_initializes_experience_to_l1_not_default`.
- **MEDIUM** `compute_xp_from_enemies` u32 sum overflow — accumulator changed to `u64`, cast to `u32` after the final `min(1_000_000)`.
- **MEDIUM** `recompute_xp_to_next_level` orphan-export — docstring now flags it as "exported for #21+ class-change; no production call sites in #19".

**Deferred per reviewer's note:**
- **LOW** `level_up` accepts `_rng` / `_current` it ignores — forward-compat for stochastic stat growth.

## Deferred follow-ups (carried in PR body)

- Thief / Bishop / Samurai / Lord / Ninja class authoring (RON-only follow-up; UI already filters via `ClassTable::get(c).is_some()`)
- Class-change UI (#21+)
- Stochastic stat growth on level-up (the unused `_rng` parameter is the seam)
- Combat-gold reward (#21 Loot — `CombatVictoryEvent.total_gold` already plumbed, currently always 0)
- `DismissedPool::MapEntities` for save/load (#23)
