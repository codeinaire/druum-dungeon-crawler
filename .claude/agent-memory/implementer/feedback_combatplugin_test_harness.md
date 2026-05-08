---
name: CombatPlugin required in ALL test harnesses that include CellFeaturesPlugin or DungeonPlugin (post-Feature-#14)
description: Any make_test_app() anywhere in the repo that includes CellFeaturesPlugin or DungeonPlugin must also include CombatPlugin; there are at least 4 harnesses (features.rs, dungeon/tests.rs, tests/dungeon_geometry.rs, tests/dungeon_movement.rs)
type: feedback
---

Post-Feature-#14, `CellFeaturesPlugin` registers `apply_poison_trap` with `.before(apply_status_handler)`. The `apply_status_handler` system is registered by `StatusEffectsPlugin`, which is a sub-plugin of `CombatPlugin`.

Any test app that includes `CellFeaturesPlugin` (or `DungeonPlugin` + traps) must also include `CombatPlugin` to register the systems it orders against. Without it, the `.before(apply_status_handler)` ordering constraint has nothing to resolve against and `ApplyStatusEvent` messages go unread — 7 dungeon tests panic on `MessageWriter<ApplyStatusEvent>::messages failed validation`.

**Known harnesses requiring CombatPlugin (post-#14):**
- `src/plugins/dungeon/features.rs::make_test_app()` (fixed via D-I4)
- `src/plugins/dungeon/tests.rs::make_test_app()` (fixed via D-I10)
- `tests/dungeon_geometry.rs` helper (fixed via D-I11)
- `tests/dungeon_movement.rs` helper (fixed via D-I11)

**Why:** When adding any plugin that introduces cross-plugin `.before(...)`/`.after(...)` ordering, ALL test harnesses in the repo are affected — not just the one nearest the modified code. The D-I10/D-I11 failures only surfaced when `cargo test` actually ran; they were not caught by static analysis.

**How to apply:** When adding a new plugin that orders against another plugin's systems, immediately audit every test harness in the repo:
```
rg 'fn make_test_app|fn build_test_app|App::new\(\)' src/ tests/
```
Add the required plugin to every harness found. For `CombatPlugin` specifically, use:
- In unit tests (src/): `crate::plugins::combat::CombatPlugin`
- In integration tests (tests/): `druum::plugins::combat::CombatPlugin`
