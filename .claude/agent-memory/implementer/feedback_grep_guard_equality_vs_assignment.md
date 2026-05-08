---
name: Plan grep guards — `current_hp.*=` matches equality checks too
description: The pattern `current_hp.*=` matches `current_hp == 0` (comparison) as well as `current_hp = x` (assignment); fix by using `< 1` instead of `== 0` for u32 zero-checks when this guard applies
type: feedback
---

Plan-level grep guards using `current_hp.*=` are intended to catch HP mutations in AI code, but the `.*=` regex also matches the `=` inside `==` comparison operators.

**Why:** `rg 'current_hp.*='` matches `derived.current_hp == 0` because `.*` matches whitespace and `=` is found in `==`. This is a regex pitfall — the intent is to catch `current_hp = x` (assignment), but comparisons are caught too.

**How to apply:** When the plan includes a `current_hp.*=` grep guard requiring 0 matches, any `current_hp == 0` read-only check in the guarded file will fail it. Rewrite as `current_hp < 1` (semantically identical for u32). Applies to any `u32` zero-check field that appears in a grep guard targeting `fieldname.*=`.
