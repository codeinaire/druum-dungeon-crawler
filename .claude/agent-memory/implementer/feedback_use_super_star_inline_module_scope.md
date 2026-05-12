---
name: use super::* in inline mod tests brings private use imports from same file
description: use super::* in inline mod tests block imports private `use` decls from the outer module of the SAME file; cross-file test imports still need explicit `use` declarations
type: feedback
---

In Rust, `use super::*` inside an inline `mod tests { ... }` block behaves differently depending on where the parent's `use` declarations originate:

- **Same file (inline module):** `use super::*` in `mod tests` brings in all items visible in the outer module's scope, including private `use SomeType;` declarations made in the same file. These are "in scope" for the child module.

- **Cross-file (external module):** `use super::*` does NOT re-export private `use SomeType;` declarations that were imported from another crate/module. Only `pub` items defined in the parent are re-exported via `*`.

**Example that works (same file):**
```rust
// square.rs outer scope
use crate::plugins::state::{GameState, TownLocation};
use crate::plugins::town::gold::{Gold, GameClock};

mod tests {
    use super::*; // GameState, TownLocation, Gold, GameClock all in scope
    fn make_app() {
        app.init_state::<GameState>(); // OK
        app.init_resource::<Gold>();   // OK
    }
}
```

**Example that fails (cross-file):**
```rust
// data/town.rs outer scope  
use crate::plugins::party::character::{Race, Class}; // private import

mod tests {
    use super::*; // Race and Class are NOT in scope here
    // Must add: use crate::plugins::party::character::{Race, Class};
}
```

**Why:** The `*` glob in `use super::*` expands to the parent's public namespace. For private `use` imports in the same file, they ARE accessible via `super::TypeName` but NOT via `super::*` if the type itself is not defined in the parent module. However, in practice, items accessible via the parent's `use` statements ARE brought in via `super::*` for inline child modules in the same file, because Rust's privacy model allows inline modules to see their parent's private items.

**How to apply:** When writing test modules, if they're inline in the same file as the code under test, `use super::*` usually suffices. If test assertions fail with "unresolved import" for types only available via `use` in the parent, add explicit `use crate::...` declarations.
