---
name: Explicit impl Default for semantically-meaningful non-zero config structs
description: When a config struct has non-zero or non-false default field values, use explicit impl Default rather than #[derive(Default)] to avoid silently wrong defaults (density 0 = no fog, brightness 0 = black)
type: feedback
---

For config structs (Bevy components, RON-loaded data, plugin tunables) where the correct default values are non-zero, always use an explicit `impl Default` block rather than `#[derive(Default)]`.

**Why:** `#[derive(Default)]` produces `0.0` for `f32` fields and `false` for `bool` fields. For atmosphere/lighting configs this produces wrong behavior by default: `density: 0.0` = no fog (invisible), `ambient_brightness: 0.0` = pure black (unlit). Feature #9 (`FogConfig` and `LightingConfig`) is the canonical example in this codebase.

**How to apply:** When reviewing any new config struct in this project, verify:
1. If any field has a semantically-meaningful non-zero default (e.g., a brightness, a density, a duration, a speed multiplier), the struct must use `impl Default`, not `#[derive(Default)]`.
2. The derive trap should be documented in the struct's doc-comment so future contributors don't "simplify" it back to a derive.
3. The explicit impl must produce values that match what the doc-comment claims (e.g., "1.0 is near-black" must actually have `ambient_brightness: 1.0` in the impl).
