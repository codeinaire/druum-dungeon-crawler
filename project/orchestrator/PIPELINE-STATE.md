# Pipeline State

**Task:** Drive the full pipeline (research → plan → implement → ship → review) for Feature #3: Asset Pipeline & RON Loading from the dungeon crawler roadmap. Add `bevy_common_assets` and `bevy_asset_loader` (pinned 0.18-compat versions), register custom RON extensions for dungeon/items/enemies/classes/spells, create `LoadingPlugin` driving `GameState::Loading → TitleScreen`, declarative `AssetCollection`, minimum-viable bevy_ui loading text, RON round-trip serde test, placeholder RON files under `assets/`, top-level `assets/README.md`, and hot-reload opt-in. Bevy =0.18.1. Out of scope: real schemas, egui, real title screen UI, leafwing input, RNG.
**Status:** in-progress
**Last Completed Step:** 3

## Artifacts

| Step | Description | Artifact                                 |
| ---- | ----------- | ---------------------------------------- |
| 1    | Research    | /Users/nousunio/Repos/Learnings/claude-code/druum/project/research/20260501-160000-bevy-0-18-1-asset-pipeline-feature-3.md |
| 2    | Plan        | /Users/nousunio/Repos/Learnings/claude-code/druum/project/plans/20260501-164500-bevy-0-18-1-asset-pipeline-feature-3.md |
| 3    | Implement   | /Users/nousunio/Repos/Learnings/claude-code/druum/project/implemented/20260501-194500-bevy-0-18-1-asset-pipeline-feature-3.md |
| 4    | Ship        | pending                                  |
| 5    | Code Review | pending                                  |

## Implementation Notes

Step 1 verification gate passed: `bevy_common_assets = "=0.16.0"` and `bevy_asset_loader = "=0.26.0"` — both support Bevy 0.18 natively. Versions came in higher than research predicted (`~0.14.x` / `~0.25.x`). All 6 verification commands green: `cargo check`, `cargo check --features dev`, `cargo clippy --all-targets -- -D warnings`, `cargo clippy --all-targets --features dev -- -D warnings`, `cargo test` (2 tests), `cargo test --features dev` (3 tests).

One deviation from the plan: `serde` and `ron` were declared explicitly in `Cargo.toml` rather than relying on transitive presence via Bevy. Reason: Rust 2024 edition does not permit direct use of transitive crate names in source. Versions match Cargo.lock (serde 1, ron 0.12). Captured in `.claude/agent-memory/implementer/feedback_rust_2024_transitive_deps.md`.

Implementation step was executed manually by the parent session because `SendMessage` does not resume a returned agent — the orchestrator's turn ended when it returned the plan summary. A fresh `implementer` agent was dispatched against the approved plan file.

## User Decisions

Plan-time, all 5 research open questions resolved by planner without escalation:
1. Crate version pinning — Step 1 of plan is a fail-stop verification gate before Cargo.toml edits.
2. Font in DungeonAssets — DROPPED for Feature #3, deferred to Feature #25.
3. Camera2d lifecycle — spawned on OnEnter(Loading), despawned on OnExit via shared LoadingScreenRoot marker.
4. assets/README.md — IN scope (Step 8).
5. RON commenting style — comments fine in hand-authored files; omitted from round-trip-test strings.

Awaiting user approval to proceed to Step 3 (Implement).
