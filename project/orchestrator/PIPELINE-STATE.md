# Pipeline State

**Task:** Drive the full feature pipeline (research → plan → implement → review) for **Feature #14: Status Effects System** in this Bevy 0.18 dungeon-crawler RPG project. Roadmap §14 at `project/roadmaps/20260429-01-bevy-dungeon-crawler-roadmap.md:741-786`. Build duration-tracked status effect *systems* on top of existing data types. STOP after review unless user requests shipping.

**Status:** complete — pipeline ran research → plan → implement → review. Commits pending; ship pending user authorization.
**Last Completed Step:** 4 (review)

## Artifacts

| Step | Description | Artifact |
| ---- | ----------- | -------- |
| 1    | Research    | `project/research/20260507-115500-feature-14-status-effects-system.md` |
| 2    | Plan        | `project/plans/20260507-124500-feature-14-status-effects-system.md` (Status: Approved 2026-05-07; D7=A, D9=A) |
| 3    | Implement   | `project/implemented/20260507-120000-feature-14-status-effects-system.md` (verification GREEN; review fixes applied) |
| 4    | Review      | `project/reviews/20260507-153000-feature-14-status-effects-system.md` (Verdict: LGTM with 1 MEDIUM, 2 LOW, 1 NIT — all addressed) |
| 5    | Pipeline summary | `project/orchestrator/20260507-200000-feature-14-status-effects-system.md` |
| 6    | Ship        | NOT IN SCOPE — awaiting explicit user authorization |

## User Decisions

- **D7 = A** — Per-tick poison/regen damage formula: `((max_hp / 20).max(1) as f32 * magnitude) as u32` with `.max(1)` floor.
- **D9 = A** — Dungeon-step tick frequency: every step.

## Verification gate (executed 2026-05-07)

| Check | Result |
| ----- | ------ |
| `cargo check` | PASS (1.16s) |
| `cargo check --features dev` | PASS (2.19s) |
| `cargo test` | PASS — 159 tests (153 lib + 6 integration), 0 failed |
| `cargo test --features dev` | PASS — 162 tests, 0 failed |
| `cargo clippy --all-targets -- -D warnings` | PASS (zero warnings) |
| `cargo clippy --all-targets --features dev -- -D warnings` | PASS |
| `cargo fmt --check` | PASS (after auto-fmt applied) |
| `rg 'derive(...Event...)' status_effects.rs` | 0 matches (Message-only) |
| `rg 'EventReader/Writer' status_effects.rs tests/` | 0 matches |
| `rg 'effects.push(' src/plugins/` | only inside `apply_status_handler` and `#[cfg(test)]` fixtures |
| `Cargo.toml` / `Cargo.lock` byte-changed | NO (zero new deps) |

## Review fixes applied (2026-05-07)

After the reviewer landed verdict LGTM, three actionable findings were addressed:

- **MEDIUM** (`status_effects.rs:817-852`) — `check_dead_and_apply_writes_when_hp_zero` and `check_dead_and_apply_no_op_when_hp_positive` tests now register `system_call_check_dead.before(apply_status_handler)`, removing the `if !has_dead { app.update(); }` non-determinism. Both tests now run a single `app.update()`.
- **LOW 1** (plan: `## Frozen post-#13 / DO NOT TOUCH`) — added a fifth carve-out bullet documenting `dungeon/tests.rs`, `tests/dungeon_geometry.rs`, `tests/dungeon_movement.rs` as test-harness-only additions (D-I10/D-I11) requiring `CombatPlugin` to register `Messages<ApplyStatusEvent>`.
- **LOW 2** (`status_effects.rs:67-70`) — added Stone/Dead `duration: None` invariant to the `ApplyStatusEvent.duration` doc-comment. Caller convention; handler doesn't validate.
- **NIT** (test harness pattern discussion) — no action; flagged for awareness only.

Re-run after fixes: `cargo test` PASS (159 tests), `cargo fmt --check` PASS.

## Files changed

| File | Status | Net change |
| ---- | ------ | ---------- |
| `src/plugins/combat/mod.rs` | Modified | +18 LOC (sub-plugin wiring + module re-export) |
| `src/plugins/combat/status_effects.rs` | NEW | ~930 LOC (plugin + 19 tests + module doc) |
| `src/plugins/dungeon/features.rs` | Modified | +24 LOC (poison-trap refactor to ApplyStatusEvent + system ordering) |
| `src/plugins/dungeon/tests.rs` | Modified | +2 LOC (CombatPlugin in `make_test_app`) |
| `src/plugins/party/character.rs` | Modified | +207 LOC (5 enum variants, buff branches in `derive_stats`, 4 new tests) |
| `src/plugins/party/inventory.rs` | Modified | +14 LOC (doc-comment update on `EquipmentChangedEvent`) |
| `tests/dungeon_geometry.rs` | Modified | +2 LOC (CombatPlugin registration) |
| `tests/dungeon_movement.rs` | Modified | +2 LOC (CombatPlugin registration) |
| `project/research/...feature-14...md` | NEW | research artifact |
| `project/plans/...feature-14...md` | NEW | plan (Approved) |
| `project/implemented/...feature-14...md` | NEW | implementation summary |
| `project/reviews/...feature-14...md` | NEW | code review |
| `project/orchestrator/PIPELINE-STATE.md` | Modified | this file |
| `project/orchestrator/20260507-200000-feature-14-status-effects-system.md` | NEW | pipeline summary |
| `.claude/agent-memory/{implementer,planner,researcher}/MEMORY.md` | Modified | memory index updates |
| `.claude/agent-memory/...` | NEW | 6 new memory files (3 implementer feedback, 1 planner project, 2 researcher reference) |

## Pipeline scope

Run research → plan → implement → review. STOP at review. Ship requires explicit user authorization.
