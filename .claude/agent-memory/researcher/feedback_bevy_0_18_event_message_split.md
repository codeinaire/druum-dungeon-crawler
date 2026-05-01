---
name: Bevy 0.18 Event vs Message split
description: In Bevy 0.18, several types that were `Event` in earlier versions are now `Message` — using `EventReader` on them will not compile
type: feedback
---

In Bevy 0.18, the buffered-event family was split into `Event` (one-shot, observer-based) and `Message` (buffered, polling-based). Several types that were `Event` in 0.17 are now `Message`:

- `StateTransitionEvent<S>` — derived `Message` (verified in `bevy_state-0.18.1`)
- `AssetEvent<T>` — derived `Message` (verified at `bevy_asset-0.18.1/src/event.rs:9, 49`)
- `AssetLoadFailedEvent<T>` — derived `Message`
- `UntypedAssetLoadFailedEvent` — derived `Message`

**Why:** the 0.17→0.18 buffered-event split. Older blog posts and 0.17-era examples will mislead.

**How to apply:** to read these in 0.18, use `MessageReader<T>`, NOT `EventReader<T>`. The latter will not compile. When researching a Bevy 0.18 feature that involves any reactive type, grep the 0.18.1 source for `derive(Message` to know whether it's a Message or Event. Routing through higher-level abstractions (`bevy_asset_loader` for assets, `state_changed` run condition for states) avoids the trap entirely.
