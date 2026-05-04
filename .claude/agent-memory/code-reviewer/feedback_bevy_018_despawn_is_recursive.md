---
name: Bevy 0.18 despawn() is recursive — children are cleaned up automatically
description: In Bevy 0.18, commands.entity(e).despawn() recursively despawns all children. No separate despawn_recursive call needed.
type: feedback
---

In Bevy 0.18, `commands.entity(e).despawn()` is recursive by default. All child entities are despawned automatically. This applies to `PlayerParty` (spawned with `children![Camera3d, ...]`) — the `DungeonCamera` child is cleaned up when the parent is despawned, no extra query needed.

**Why:** Bevy 0.18 changed the default `despawn()` behavior to be recursive. Previous Bevy versions had separate `despawn()` (non-recursive) and `despawn_recursive()` methods. In 0.18 there is only `despawn()` and it is always recursive.

**How to apply:** When reviewing despawn logic in Bevy 0.18 Druum code, do NOT flag `commands.entity(e).despawn()` on parent entities as missing recursive cleanup. The children are handled. Only flag it if the intent was to keep children alive (unusual pattern).
