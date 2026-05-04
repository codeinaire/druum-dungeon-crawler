---
name: clippy::manual_range_contains — use .contains() not comparison chain
description: Bevy tests fail clippy -D warnings when using `x >= lo && x <= hi`; must use `(lo..=hi).contains(&x)` instead
type: feedback
---

Use `(lo..=hi).contains(&x)` instead of `x >= lo && x <= hi` in all test assertions and production code.

**Why:** `clippy::manual_range_contains` is enabled under `-D warnings` (which is the project's standard gate). The comparison-chain form triggers the lint and fails the quality gate.

**How to apply:** Any time you write a numeric range check, default to the range contains form. This applies in test asserts like `assert!(intensity >= 800.0 && intensity <= 1200.0, ...)` — rewrite as `assert!((800.0..=1200.0).contains(&intensity), ...)`.
