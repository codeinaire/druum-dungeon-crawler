---
name: Ron version split — bevy_common_assets 0.11 vs project 0.12
description: bevy_common_assets 0.16.0 links against ron 0.11.0 internally; this project explicitly depends on ron 0.12; round-trip tests use 0.12 while the loader uses 0.11
type: feedback
---

`bevy_common_assets 0.16.0` declares `ron 0.11.0` as its internal dep (aliased as `serde_ron`). The project's explicit `ron = "0.12"` in `Cargo.toml` resolves to `0.12.1`. Cargo carries both as separate semver-incompatible copies in the lockfile.

**Why:** This matters because round-trip tests in `src/data/dungeon.rs` serialize/deserialize using `ron 0.12`, while `bevy_common_assets`' actual `RonAssetPlugin` loader uses `ron 0.11` internally. For empty struct bodies (`()`), the format is identical. But once Feature #4 adds real fields to `DungeonFloor`, the test may pass while the loader silently produces different output.

**How to apply:** When reviewing Feature #4 (or any PR that adds fields to stub asset schemas), flag this: the round-trip test should either (a) also assert what `bevy_common_assets`' loader produces (run a real App load), or (b) be updated to use `ron 0.11` for the deserialize step to match the loader's version. The plan note about this is in `project/implemented/20260501-194500-bevy-0-18-1-asset-pipeline-feature-3.md` under "Implementation Discoveries #2".

Observed in: Feature #3, PR #3, `Cargo.lock` (confirmed dual ron entries: `ron 0.11.0` for `bevy_common_assets`, `ron 0.12.1` for `druum`).
