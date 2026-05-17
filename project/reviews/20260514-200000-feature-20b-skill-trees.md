# Review: Feature #20b — Skill Trees, SP Allocation, Guild Skills Mode (Phase 2)

**PR:** https://github.com/codeinaire/druum-dungeon-crawler/pull/23
**Branch:** `feature-20b-skill-trees` (stacked on `feature-20a-spell-registry`)
**Commit:** `1ec43e8`
**Reviewed:** 2026-05-14
**Base review:** PR #21 Phase 1 (APPROVED, addendum resolving all findings)

## Verdict: APPROVE

| Severity | Count |
| -------- | ----- |
| CRITICAL | 0     |
| HIGH     | 0     |
| MEDIUM   | 0     |
| LOW      | 2     |

## Files Reviewed (Full Coverage)

- `src/data/skills.rs` — full (Kahn's algorithm, MAX_* constants, clamp, 8 unit tests)
- `src/plugins/party/skills.rs` — full (KnownSpells, UnlockedNodes, WarnedMissingSpells, pure functions, 8 unit tests)
- `src/plugins/party/character.rs` — Experience and PartyMemberBundle extensions
- `src/plugins/party/progression.rs` — SKILL_POINTS_PER_LEVEL constant + SP award in level-up system
- `src/plugins/town/guild_skills.rs` — full (NodeState, node_state(), sorted_nodes(), paint/input/unlock, 7 tests)
- `src/plugins/town/guild.rs` — GuildMode::Skills variant, node_cursor, [ keybind dispatch
- `src/plugins/town/mod.rs` — system wiring, run-if gates, OnExit node_cursor reset
- `src/plugins/loading/mod.rs` — RonAssetPlugin registration, DungeonAssets handles, validate_skill_trees_on_load
- `src/data/mod.rs` — re-exports
- `src/plugins/party/mod.rs` — WarnedMissingSpells init, Reflect registrations
- `src/plugins/combat/ui_combat.rs` — verified unchanged (Phase 3 scope)
- `src/plugins/combat/status_effects.rs:319,347` — TODO(#22) carry-forward markers
- `assets/skills/fighter.skills.ron` — full
- `assets/skills/mage.skills.ron` — full
- `assets/skills/priest.skills.ron` — full
- `tests/skill_tree_loads.rs` — full

## Focus Area Verdicts

### 1. Save-format stability — CLEAN

`Experience` gains `unspent_skill_points: u32` and `total_skill_points_earned: u32` appended as the last two fields, each with `#[serde(default)]`. Existing saves that lack these fields deserialize to `0` — no existing-save breakage. `PartyMemberBundle` fields `known_spells` and `unlocked_nodes` are appended after all prior fields with a "do not reorder" comment. Bundles are not directly serialized — each component serializes independently — so order is not load-order-sensitive here. Save-format contract fully preserved.

### 2. DAG validation — CORRECT

Kahn's BFS implementation in `validate_no_cycles` is correct:

- **Empty tree:** `visited = 0`, `tree.nodes.len() = 0` → returns `Ok(())`. Correct.
- **Single node (no prereqs):** in-degree 0, queued, visited = 1, returns `Ok(())`. Correct.
- **Linear chain (a→b→c):** all nodes visited in order, no cycle. Covered by `validate_no_cycles_accepts_linear`.
- **Self-loop (a requires a):** `in_degree["a"] = 1`, never dequeued, `visited = 0 != 1`, returns `CycleDetected`. Covered by `validate_no_cycles_rejects_self_loop`.
- **Two-node cycle:** covered by `validate_no_cycles_rejects_two_node_cycle`.
- **Three-node cycle:** covered by `validate_no_cycles_rejects_three_node_cycle`.
- **Unknown prerequisite:** checked before Kahn's walk, returns `UnknownPrerequisite` early. Covered by `validate_no_cycles_rejects_unknown_prereq`.
- **Cycle detection does NOT panic:** empties tree on error via `tree.nodes.clear()` in `validate_skill_trees_on_load`.

The in-degree initialization correctly uses `prerequisites.len()` and the unknown-prereq check runs first (before the Kahn's walk begins), so no phantom edges are created in the in-degree map.

One note: the in-degree decrement loop is O(n) per dequeued node (scans all nodes for those listing `n_id` as a prereq), giving O(n²) overall. For the MAX_SKILL_TREE_NODES = 64 cap this is negligible. Acceptable.

### 3. Combat-untouched verification — CONFIRMED

`src/plugins/combat/ui_combat.rs` SpellMenu stub is unmodified — `MenuFrame::SpellMenu` at line 457 still logs and pops to Main. `src/plugins/combat/turn_manager.rs` production code is unmodified (only test fixture additions at lines 1768-1770 for `DungeonAssets` fan-out as documented in deviation #4).

### 4. Guild Skills file separation — CLEAN

`guild_skills.rs` is a proper sibling of `guild.rs`/`guild_create.rs`. The only leakage into `guild.rs` is the `GuildMode::Skills` variant, the `node_cursor: usize` field on `GuildState`, and the `[` keybind dispatch — exactly the spec. Skills-specific logic (tree rendering, node state, unlock handler) lives entirely in `guild_skills.rs`.

### 5. Status-effect TODO(#22) carry-forward — CONFIRMED

Both markers verified present:
- `src/plugins/combat/status_effects.rs:319` — `// TODO(#22): widen apply_poison_damage`
- `src/plugins/combat/status_effects.rs:347` — `// TODO(#22): widen apply_regen`

Phase 2 did not revert either comment.

### 6. NodeGrant::LearnSpell validation policy (Cat-C-3=A) — CONFIRMED

`validate_skill_trees_on_load` is structural-only: it runs `clamp_skill_tree` + `validate_no_cycles` and does not walk `NodeGrant::LearnSpell` IDs against `SpellDb`. The function's doc comment explicitly documents this scope decision. Correct per locked Cat-C-3 decision.

### 7. 4-state painter (Cat-C-1=B) — CORRECT AND TESTED

`node_state()` is a pure function with zero ECS dependencies. The `SpInsufficient` (yellow) state is:
- Reachable: `can_unlock_node` returns `Insufficient`, then prereqs + level re-check both pass
- Correctly tested: `node_state_returns_sp_insufficient_when_prereq_met_but_sp_short` at `guild_skills.rs:556-565` uses cost=3, SP=1
- Correctly painted: `egui::Color32::from_rgb(230, 200, 100)` at line 300

The re-check inside the `Insufficient` arm correctly skips the `CapReached` case (that returns `CapReached` before reaching the SP check, so it never enters the `Insufficient` branch).

## Findings

### [LOW] `node_depth` has no cycle guard; relies entirely on call-site discipline

**File:** `src/plugins/town/guild_skills.rs:136-157`

**Issue:** `node_depth` is a recursive function with memoization. The memo prevents recomputing already-visited nodes but does NOT detect in-progress cycles. A cycle causes infinite recursion and stack overflow. The current defence is that both callers (`paint_guild_skills:244` and `handle_guild_skills_unlock:435`) check `tree.nodes.is_empty()` before calling `sorted_nodes` — and `validate_skill_trees_on_load` empties cyclic trees. This is a two-layer defence that works in production.

However, `node_depth` has no internal documentation of this precondition, making it easy for a future caller to use `sorted_nodes` on an unvalidated tree (e.g., in a test that constructs a cyclic tree directly) and trigger a stack overflow.

**Fix (no code change required for Phase 2 merge):** Add a `# Precondition` note to the `sorted_nodes` doc-comment:

```rust
/// Return nodes sorted by `(depth, id)` for consistent visual ordering.
///
/// # Precondition
///
/// The tree MUST be cycle-free (validated by `validate_no_cycles`). Calling
/// this on a cyclic tree causes infinite recursion in `node_depth`. Both
/// production call sites guard with `if tree.nodes.is_empty()` after
/// `validate_skill_trees_on_load` empties cyclic trees.
fn sorted_nodes(tree: &SkillTree) -> Vec<&SkillNode> {
```

---

### [LOW] `node_state` returns `SpInsufficient` for tampered-save case (`unspent > total_earned`)

**File:** `src/plugins/town/guild_skills.rs:114-123`

**Issue:** When `can_unlock_node` returns `SkillError::Insufficient` due to the tamper guard (`unspent > total_earned`), `node_state` re-checks only prereqs and level. If those pass, the node shows as yellow (`SpInsufficient`) with a "need X more SP" gloss — misleading, since the real problem is the invariant violation. A player viewing this screen after a tampered save would see yellow nodes suggesting they just need more SP, not that their save data is invalid.

Real-world impact is minimal (tampered saves are not a supported scenario). Not a correctness bug — the unlock handler's `can_unlock_node` call at line 456 would still reject the operation with "Not enough skill points." The visual discrepancy is cosmetic for a non-production path.

**Fix (optional, can defer to Phase 3 polish):**

```rust
Err(SkillError::Insufficient) => {
    let prereqs_met = node.prerequisites.iter().all(|p| unlocked.has(p));
    let level_met = experience.level >= node.min_level;
    // Tamper guard: also verify the unspent/total invariant holds.
    let invariant_ok =
        experience.unspent_skill_points <= experience.total_skill_points_earned;
    if prereqs_met && level_met && invariant_ok {
        NodeState::SpInsufficient
    } else {
        NodeState::Locked(SkillError::Insufficient)
    }
}
```

---

## Implementation Deviations Review

All 5 documented deviations are acceptable:

1. **Unused `mut exp` binding removed** — correct (avoids `unused_mut` clippy).
2. **`node_cursor` test uses index 1** — correct; test comment documents alphabetical depth-0 sort.
3. **Two queries in `handle_guild_skills_input`** — correct; both read-only, no B0002.
4. **DungeonAssets fixture fan-out at 7 sites** — correct; compiler-driven, saved to memory.
5. **LOC estimates inflated** — not a code issue.

## Summary

Phase 2 is clean. The DAG validation is correctly implemented and well-tested. Save-format stability is preserved. Guild Skills mode is properly separated and wired. The 4-state painter correctly distinguishes the yellow SP-insufficient tier. All combat code is untouched. The two LOW findings are cosmetic/documentation concerns that do not affect correctness or safety.

**Merge recommendation: merge as-is.** Both LOW findings can be addressed in Phase 3 or a follow-up polish PR.

---

---

## GitHub Posting Status

**MCP tools unavailable; shell execution unavailable.** Review body is ready in `/tmp/pr23-review-body.md`. To post manually:

```bash
gh pr review 23 --repo codeinaire/druum-dungeon-crawler --comment --body-file /tmp/pr23-review-body.md
```
