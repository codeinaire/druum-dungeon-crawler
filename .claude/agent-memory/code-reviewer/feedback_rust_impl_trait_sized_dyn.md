---
name: impl Trait + ?Sized required when passing dyn Trait
description: Functions taking `rng: &mut impl Rng` cannot accept `&mut dyn RngCore` (a DST) unless the bound includes `?Sized`; must use `&mut (impl Rng + ?Sized)`
type: feedback
---

When a function signature uses `rng: &mut impl Rng`, the implicit `Sized` bound on the `impl Trait` type parameter means it cannot be satisfied by a `dyn RngCore + Send + Sync` (a dynamically-sized type). Passing `&mut *rng.0` where `rng.0: Box<dyn RngCore + Send + Sync>` causes a compile error: "the size of ... cannot be statically determined".

**Why:** `impl Trait` desugars to a generic parameter with an implicit `Sized` bound. `dyn Trait` is a DST — it is `!Sized`. The bound `impl Rng + ?Sized` relaxes this and allows both concrete and DST arguments.

**How to apply:** When reviewing functions in this project that take an RNG parameter and are called from systems that hold a `CombatRng(Box<dyn RngCore + Send + Sync>)`, flag any `rng: &mut impl Rng` signature that lacks `?Sized`. The correct form is `rng: &mut (impl Rng + ?Sized)`. Observed and fixed in Feature #15 D-I13 (`targeting.rs:37`, `damage.rs:67`).
