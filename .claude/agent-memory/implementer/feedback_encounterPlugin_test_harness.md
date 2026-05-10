---
name: EncounterPlugin test harness requirements post-#16
description: Any test app with CombatPlugin (which includes EncounterPlugin) needs init_asset EncounterTable plus add_message EncounterRequested when CellFeaturesPlugin is absent
type: feedback
---

When `CombatPlugin` is included in a test app after Feature #16, `EncounterPlugin` is automatically included as a sub-plugin. This requires two additional initializations in test harnesses that don't include `CellFeaturesPlugin`:

1. `app.init_asset::<crate::data::EncounterTable>();` — `handle_encounter_request` takes `Res<Assets<EncounterTable>>`
2. `app.add_message::<crate::plugins::dungeon::features::EncounterRequested>();` — `EncounterPlugin` reads/writes this message, which is normally registered by `CellFeaturesPlugin`

**Why:** `EncounterPlugin` is registered inside `CombatPlugin::build` as a sub-plugin, so it's invisible at the call site. Any test that includes `CombatPlugin` without `CellFeaturesPlugin` must explicitly initialize both.

**How to apply:** When writing new test apps (or adding `CombatPlugin` to an existing test harness), always add both lines unless `CellFeaturesPlugin` is already in the plugin list. Affected harnesses as of #16: `turn_manager.rs::app_tests::make_test_app`, `ui_combat.rs::app_tests::make_test_app`. Harnesses that include `CellFeaturesPlugin` do NOT need the second line (e.g., `encounter.rs::app_tests::make_test_app`).
