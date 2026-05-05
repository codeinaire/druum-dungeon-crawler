---
name: cfg-gated use statements — scope imports inside cfg blocks to avoid unused-import warnings
description: Imports only needed in cfg(feature="dev") blocks should be placed inside the block, not at file level, to avoid unused-import warnings in default builds
type: feedback
---

If a `use` statement is only consumed inside a `#[cfg(feature = "dev")]` block, placing it at file level produces `unused import` warnings in default builds (where the block is excluded). This will fail `-D warnings`.

**Why:** Rust's warning system sees the import even though the consuming code is gated. The import is unused from the default-feature compiler's perspective.

**How to apply:** Place the `use` statement inside the `#[cfg(feature = "dev")] { ... }` block itself:

```rust
#[cfg(feature = "dev")]
{
    use crate::plugins::state::GameState;
    app.add_systems(OnEnter(GameState::Dungeon), spawn_default_debug_party);
}
```

This is cleaner than adding `#[allow(unused_imports)]` at file level and avoids leaking the import into default builds.
