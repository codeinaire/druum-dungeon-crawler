---
name: RON 0.11 — [f32;3] fixed-size arrays serialize as tuples (r,g,b) not sequences [r,g,b]
description: Hand-authored RON files must use (r,g,b) parenthesis notation for Rust [T;N] fixed-size arrays; bracket [r,g,b] causes ExpectedStructLike parse error
type: feedback
---

RON 0.11.0 maps Rust types as follows:
- `Vec<T>` / `&[T]` / slices → RON sequence `[...]` (bracket notation)
- `[T; N]` fixed-size arrays → RON tuple `(...)` (parenthesis notation, same as Rust tuples)
- Rust structs → RON struct-like `(...)`

Hand-authoring a fixed-size array field like `placeholder_color: [f32; 3]` with `[0.4, 0.6, 0.3]` in a `.ron` file causes a `SpannedError { code: ExpectedStructLike }` at runtime.

**Why:** The `ron` serde implementation treats `[T; N]` as a fixed-length tuple for serialization purposes. `ron::ser::to_string` produces `(0.4, 0.6, 0.3)` for `[f32; 3]`, which is why round-trip unit tests pass (they use programmatic serialization) while file-read tests fail (the hand-authored file used brackets). First hit in Feature #17 `core.enemies.ron`.

**How to apply:** Whenever authoring a RON file with a Rust field typed as `[T; N]` (fixed-size array), use `(value1, value2, ...)` with parentheses, NOT `[value1, value2, ...]`. For `Vec<T>` or dynamic-length sequences, brackets `[...]` are correct.
