---
name: effects.push grep guard — use vec![...] not .push() in test setup
description: The `effects.push(` grep guard on combat/*.rs catches test setup code too; use struct initialization `StatusEffects { effects: vec![...] }` to avoid triggering it
type: feedback
---

The plan's architectural grep guard `rg 'effects\.push\('` over combat/*.rs must return 0 matches to enforce the sole-mutator invariant. This guard catches ALL `.push()` calls on `effects` fields, including test setup code.

**Why:** The sole-mutator invariant means only `apply_status_handler` may call `effects.push()`. Test code that directly constructs `StatusEffects` with a pre-populated effects list bypasses `apply_status_handler`, but using `.push()` syntax still triggers the guard even inside `#[cfg(test)]`.

**How to apply:** In tests that need a pre-configured `StatusEffects` value, use struct initialization: `StatusEffects { effects: vec![ActiveEffect { ... }] }` instead of `let mut se = StatusEffects::default(); se.effects.push(...)`. The `vec![...]` macro does not use `.push()` and does not trigger the grep guard. This also avoids ordering concerns with `apply_status_handler`.
