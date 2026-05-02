---
PR: #5 — Add input system with leafwing-input-manager (Feature #5)
Branch: 5-input-system-leafwing
Date: 2026-05-02
Reviewer: code-reviewer agent
Verdict: WARNING
---

## Verdict: WARNING

One MEDIUM finding (misleading inline comment on the dungeon arrow keymap).
All static analysis clean, all 33 tests pass (32 default + 1 dev-gated F9),
full integration test passes. No CRITICAL or HIGH issues.

## Severity Counts

| Severity | Count |
|----------|-------|
| CRITICAL | 0     |
| HIGH     | 0     |
| MEDIUM   | 1     |
| LOW      | 0     |

## Key Finding

### [MEDIUM] Misleading comment on dungeon arrow bindings

**File:** `src/plugins/input/mod.rs:136`

The comment says "arrows STRAFE per modern convention" but the actual bindings
map ArrowUp → MoveForward and ArrowDown → MoveBackward (same as W/S).
Only ArrowLeft/ArrowRight strafe. The correct description is
"WASD ≡ Arrows (parallel movement keys)" per the approved PR brief.

**Suggested fix:**

```rust
// Movement — WASD and arrow keys are parallel: both sets cover
// forward/backward/strafe. Q/E are turn-only (no arrow alternates).
```

## What Was Reviewed

- PR #5 on `codeinaire/druum-dungeon-crawler`
- Files: `Cargo.toml`, `Cargo.lock`, `src/main.rs`,
  `src/plugins/input/mod.rs`, `src/plugins/mod.rs`
- Confirmed `src/plugins/state/mod.rs` was NOT touched (F9 cycler intact)
- Static analysis: `cargo check`, `cargo check --features dev`,
  `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy --all-targets --features dev -- -D warnings` — all green
- Tests: `cargo test` (32 pass), `cargo test --features dev` (33 pass)

## Coverage

All 5 changed files reviewed in full. No partial-coverage files.

## Checklist Summary

| Checklist Item | Result |
|----------------|--------|
| Pinned `=0.20.0`, `default-features = false, features = ["keyboard","mouse"]` | PASS |
| Three action enums with correct derives, no DevAction | PASS |
| Interact = F, arrows parallel to WASD for movement | PASS (code correct, comment misleading) |
| ActionsPlugin::build: 3×InputManagerPlugin + 3×init_resource + 3×insert_resource | PASS |
| F9 cycler unchanged (state/mod.rs not in diff) | PASS |
| Tests use full InputPlugin + Buttonlike::press() | PASS |
| 5 tests: 1 smoke + 4 injection | PASS |
| Cargo.lock: 7 new leafwing packages only, no version bumps to bevy/ron/serde | PASS |
| No EventReader<KeyboardInput> (Bevy 0.18 rename trap) | PASS |
| No rand calls | PASS |
| No run_if on Plugin add | PASS |
| No dead code | PASS |
