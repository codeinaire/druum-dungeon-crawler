---
name: Rust 2024 let-chains — clippy collapsible_if with if-let inside guard
description: Nested `if outer { if let Some(x) = ... }` must be collapsed to `if outer && let Some(x) = ...` in edition="2024" (Rust 2024 stabilized let-chains)
type: feedback
---

In Rust edition="2024", clippy's `collapsible_if` lint fires on patterns like:

```rust
if y == floor.height - 1 {
    if let Some(mat) = wall_material(...) {
        // ...
    }
}
```

The fix is to use let-chain syntax (stabilized in Rust 2024):

```rust
if y == floor.height - 1
    && let Some(mat) = wall_material(...)
{
    // ...
}
```

**Why:** Rust 2024 edition stabilizes let-chains (`if cond && let x = expr`), so clippy considers the nested form always collapsible.

**How to apply:** Any time you write a guard condition followed by an inner `if let`, consider the let-chain form first. In this codebase (edition="2024"), the nested form will always get a clippy error.
