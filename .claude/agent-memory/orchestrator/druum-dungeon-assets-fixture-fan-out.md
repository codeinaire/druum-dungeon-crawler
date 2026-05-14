---
name: druum-dungeon-assets-fixture-fan-out
description: DungeonAssets test fixtures and init_asset registrations exist in 7+ sites across tests/ and plugins — any new world-resource consumed by a system must register in all sites or cargo check fails in test/dev builds
metadata:
  type: project
---

When a new world-resource (e.g., `SpellDb`, `SkillTreeDb`) is consumed by a Bevy system in the Druum codebase, the resource must be registered via `init_asset::<T>()` in **every** site that constructs a minimal `App` for testing or dev-mode rendering. There are at least 7 such sites:

- `tests/dungeon_movement.rs:146-154` — canonical `DungeonAssets` fixture
- `tests/dungeon_geometry.rs:150-158` — canonical `DungeonAssets` fixture
- `src/plugins/combat/ai.rs` — AI system test setup (Phase 1 caught this)
- `src/plugins/combat/encounter.rs:597` — encounter test setup (Phase 3 caught this)
- `src/plugins/combat/enemy_render.rs:723` — enemy render test setup (Phase 3 caught this)
- Plus 2+ other sites discovered during earlier feature work

**Why:** Bevy's `init_asset::<T>()` is required for `Res<Assets<T>>` to resolve. If a system gains a new `Res<Assets<T>>` parameter and even one App-constructor in test/dev code misses the `init_asset` call, `cargo check` fails non-obviously — the error message points at the system signature, not the missing init. Three separate fixup cycles across Feature #20 (Phase 1 ai.rs, Phase 3 encounter.rs + enemy_render.rs) hit this same pattern.

**How to apply:** When adding any `Res<Assets<T>>` or `ResMut<Assets<T>>` to a system that runs in non-default features or during integration tests, grep for all `init_asset::<` call sites and `App::new()` constructors in `tests/` and `src/plugins/`. Add `init_asset::<T>()` to each. Verify with `cargo check`, `cargo check --features dev-party`, and `cargo test --test '*'`. This is now a known landmine — don't ship without confirming the fan-out is complete.

Related: [[druum-fix-review-findings-before-completion]] (the user-pattern of catching these in fixup cycles rather than upfront).
