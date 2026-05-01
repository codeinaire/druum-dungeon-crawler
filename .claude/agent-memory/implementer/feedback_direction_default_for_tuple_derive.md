---
name: Enum Default required when enum is inside a tuple field on a Default-derived struct
description: If a struct derives Default and has a tuple field (u32, u32, MyEnum), MyEnum must also derive Default — Rust tuple Default requires all elements to implement Default
type: feedback
---

When `#[derive(Default)]` is on a struct that contains a tuple field like `pub entry_point: (u32, u32, Direction)`, ALL elements of the tuple must implement `Default`. Rust derives `Default` for tuples only when every element type implements `Default`.

**Why:** The compiler error is `E0277: the trait bound 'MyEnum: std::default::Default' is not satisfied` in the `Default` derive macro expansion. Tuples are covered by blanket impls that require `Default` on each element.

**How to apply:** If a plan specifies an enum's derive list without `Default` but the enum is used as part of a field in a `Default`-deriving struct (including inside tuples, `Option<>`, or `Vec<>`), add `Default` to the enum derive and mark one variant as `#[default]`. Choose the variant that is the natural "no-op" or "initial state" value (e.g. `#[default] North` for a `Direction` enum that matches the grid's top-left origin convention).
