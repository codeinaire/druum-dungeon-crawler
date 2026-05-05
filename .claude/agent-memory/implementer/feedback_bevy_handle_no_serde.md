---
name: Bevy 0.18 Handle<T> — no Serialize/Deserialize impl
description: Handle<T> in bevy_asset-0.18.1 does NOT implement Serialize/Deserialize; any component with Handle fields cannot auto-derive serde
type: feedback
---

`Handle<T>` in `bevy_asset-0.18.1` has no `Serialize` or `Deserialize` implementation. Components with `Option<Handle<T>>` fields (like `Equipment`) cannot derive these traits — the derive macro will fail with "the trait bound `Handle<X>: serde::Serialize` is not satisfied."

**Why:** The research document for Feature #11 claimed "Handle<ItemAsset> serializes cleanly as an asset path" — this is incorrect for Bevy 0.18. Bevy's Handle does NOT have a built-in serde impl.

**How to apply:** When a component stores `Handle<T>` fields, drop `Serialize`/`Deserialize` from its derive set and document that Feature #23 (save/load) must add a custom serde impl (e.g., serialize as `Option<AssetPath>` string, re-resolve on load). All other components without Handle fields can still auto-derive serde normally.
