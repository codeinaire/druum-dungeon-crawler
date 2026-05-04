---
name: bevy::utils::HashMap removed in Bevy 0.18.1
description: bevy::utils::HashMap no longer exists in Bevy 0.18.1; use std::collections::HashMap instead
type: feedback
---

Use `std::collections::HashMap` in all Bevy 0.18.1 code. `use bevy::utils::HashMap` no longer compiles.

**Why:** Bevy 0.18.1 removed the re-export of `HashMap` from `bevy::utils`. Any plan or training-data example that says `use bevy::utils::HashMap` will produce a compiler error.

**How to apply:** Replace any `use bevy::utils::HashMap` with `use std::collections::HashMap`. No behavioral difference at Druum's data scales (floor maps are 6×6 to ~30×30 — the hashbrown performance advantage of `bevy::utils::HashMap` is not relevant here).
