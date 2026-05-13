---
name: reference-druum-rng-pattern-canonical
description: Druum's canonical RNG pattern — pure fns take `rng: &mut (impl Rng + ?Sized)`, prod uses Box<dyn RngCore>, tests use ChaCha8Rng::seed_from_u64
metadata:
  type: reference
---

Druum has a single, project-wide RNG pattern for deterministic random math, established by Feature #15/#16 and inherited by all subsequent features (verified across 5+ files 2026-05-13).

**The pattern:**

1. **Pure functions** take `rng: &mut (impl rand::Rng + ?Sized)`. The `?Sized` is REQUIRED to permit `&mut *boxed_rng.0` from a `Box<dyn RngCore + Send + Sync>` (DST passes). Without `?Sized` you get a cryptic compile error at the call site.

2. **Production resources** wrap `Box<dyn rand::RngCore + Send + Sync>`. Canonical example `combat/encounter.rs:91-96`:
```rust
pub struct EncounterRng(pub Box<dyn rand::RngCore + Send + Sync>);
impl Default for EncounterRng {
    fn default() -> Self { Self(Box::new(rand::rngs::SmallRng::from_os_rng())) }
}
```

3. **Tests inject `rand_chacha::ChaCha8Rng::seed_from_u64(seed)` directly** to the pure function. NO need to insert the resource. Tests are byte-for-byte deterministic with a fixed seed.

**Cargo.toml line items (verified):**
- `rand = { version = "0.9", default-features = false, features = ["std", "std_rng", "small_rng", "os_rng"] }` (line 37)
- `[dev-dependencies] rand_chacha = { version = "0.9", default-features = false, features = ["std"] }` (line 40)

**rand 0.9 API notes (NOT 0.8 — codebase is on 0.9):**
- `gen_range` → `random_range`
- `rand::distributions` → `rand::distr`
- `WeightedIndex` → `rand::distr::weighted::WeightedIndex` (verified `data/encounters.rs:92`)
- `Rng::random::<f32>()` is the new name for `Rng::gen::<f32>()`

**Files using the pattern (verification points):**
- `src/data/encounters.rs:78-96` — `pick_group` weighted-random
- `src/plugins/combat/damage.rs:62-67` — `damage_calc` variance + crit
- `src/plugins/combat/targeting.rs:37` — target selection
- `src/plugins/combat/encounter.rs:85-96` — `EncounterRng` resource definition
- `src/plugins/combat/ai.rs:206` — `seed_rng(app, seed)` test helper

**Why:**
- Deterministic tests (single test failure points at one seed, not RNG noise).
- The same pure function runs in production (with `SmallRng`) and tests (with `ChaCha8Rng`) — no behavioral divergence.
- No `thread_rng()` anywhere — avoids global state and ensures save-replay determinism is possible later.

**How to apply (for any new feature needing RNG):** mirror `EncounterRng` exactly. Name the resource `<Feature>Rng`. Production handler calls the pure function with `&mut *resource.0`; tests call the pure function with `&mut ChaCha8Rng::seed_from_u64(seed)`. Feature #19 follows this — `ProgressionRng` for bonus-pool roll + level-up RNG.

See [[reference-druum-rand-step-abc-gate]] for the historical context on adding `rand` as a direct dep.
