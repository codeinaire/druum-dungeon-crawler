---
name: DistanceFog and FogFalloff require explicit bevy::pbr import
description: bevy::prelude::* does not actually bring DistanceFog/FogFalloff into scope despite documentation claims; add explicit use bevy::pbr::{...}
type: feedback
---

Add `use bevy::pbr::{DistanceFog, FogFalloff};` explicitly rather than relying on `bevy::prelude::*`.

**Why:** The plan (and bevy_pbr-0.18.1/src/lib.rs) indicate these are in the prelude, but in practice the compiler required the explicit import path. Relying on prelude inclusion for these types leads to "unresolved import" errors at `cargo check` time.

**How to apply:** When adding fog to a camera in any dungeon or game-state module, always include the explicit import at the top of the file.
