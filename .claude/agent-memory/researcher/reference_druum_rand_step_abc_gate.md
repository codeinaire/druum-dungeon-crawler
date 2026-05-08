---
name: Druum rand crate is transitive but NOT a direct dep — Step A/B/C gate required
description: rand 0.9.4 is in Cargo.lock transitively; adding it to Cargo.toml triggers the established Step A/B/C verification gate
type: reference
---

When researching/planning a Druum feature that needs RNG (Feature #15 combat AI, #16 encounter rolls, #20 spell effects, #22 monster spawning), know the crate state:

**`rand 0.9.4`** — IN `Cargo.lock` line 4360 (transitive — likely via `bevy_internal` or `winit` for entity ID generation). NOT in `Cargo.toml` `[dependencies]`. To use `rand::*` directly in `src/`, add it to `Cargo.toml`.

**`rand_chacha 0.9.0`** — IN `Cargo.lock` line 4368 (transitive via `rand`). For deterministic seeded RNG. Standalone use is overkill for combat (atomic — no save state mid-encounter); `rand::rngs::SmallRng::seed_from_u64(...)` is sufficient.

**`rand_distr 0.5.1`** — IN `Cargo.lock` line 4387 (transitive). For statistical distributions (Normal, Poisson). NOT needed for #15-#22.

**Step A/B/C verification gate** is REQUIRED before adding to `Cargo.toml` (per `feedback_third_party_crate_step_a_b_c_pattern.md`):

- **Step A:** `cargo add rand --dry-run` to confirm resolution (likely 0.9.x), check Bevy compatibility (rand has no Bevy dep, trivially compatible).
- **Step B:** Audit `[features]`. Defaults are `["std", "std_rng"]`. Recommend `default-features = false, features = ["std", "std_rng"]` to skip `serde` (RNG state should never serialize).
- **Step C:** Grep API: `rand::rngs::SmallRng`, `rand::Rng::gen_range`, `rand::seq::IteratorRandom::choose`, `rand::seq::SliceRandom::choose`, `rand::SeedableRng::seed_from_u64`.

**Recommended Cargo.toml addition for #15:**

```toml
rand = { version = "0.9", default-features = false, features = ["std", "std_rng"] }
```

**For tests**, use `SmallRng::seed_from_u64(42)` — fast, deterministic, sufficient for replay determinism. **For production**, use `SmallRng::from_entropy()` (or `from_os_rng` in newer rand) once at `OnEnter(GameState::Combat)`.

**Alternatives considered:**
- `bevy_math::sampling::WeightedAliasIndex` — for weighted-table draws only; doesn't give `gen_range` ergonomics. NOT a substitute for `rand`.
- `rand_chacha` standalone — overkill for atomic combat; only needed if save/load persists RNG state mid-encounter (it doesn't).

**How to apply:** when recommending RNG in a Druum research/plan doc, mark `rand` direct dep as MEDIUM-LOW confidence pending the Step A/B/C gate. Note that the lockfile already contains the resolved version, so the gate runs in 5 minutes max. Surface the resolved version + feature set the implementer should use.
