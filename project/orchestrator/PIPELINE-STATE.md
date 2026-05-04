# Pipeline State

**Task:** Drive research → plan pipeline (PAUSE at plan-approval) for Feature #9: Dungeon Lighting & Atmosphere from the dungeon crawler roadmap. Add `DistanceFog` (per-floor RON parameters), low warm `GlobalAmbientLight`, per-cell `PointLight` torches placed via `light_positions` field on `DungeonFloor`, flicker animation, shadow-cap of 4 per visible region, sample torches in `floor_01.dungeon.ron`. Reconcile with Feature #8's user-override player-attached torch (Wizardry torchlight) — KEEP / REPLACE / per-floor RON option is a Category B decision. Module split (`renderer.rs` extraction, mod.rs is now ~1355 LOC) is also Category B. Bevy 0.18.1, Δ deps = 0. Final report at plan stage MUST include plan path + concise summary; parent dispatches implementer manually because `SendMessage` does not actually resume returned agents (confirmed across Features #3-#8).

**Status:** in-progress (paused at plan-approval — parent dispatches implementer manually)
**Last Completed Step:** 2

## Artifacts

| Step | Description | Artifact                                 |
| ---- | ----------- | ---------------------------------------- |
| 1    | Research    | /Users/nousunio/Repos/Learnings/claude-code/druum/project/research/20260504-040000-feature-9-dungeon-lighting-atmosphere.md |
| 2    | Plan        | /Users/nousunio/Repos/Learnings/claude-code/druum/project/plans/20260504-050000-feature-9-dungeon-lighting-atmosphere.md |
| 3    | Implement   | (out of scope — parent dispatches manually) |
| 4    | Ship        | (out of scope)                           |
| 5    | Code Review | (out of scope)                           |

## Research Summary (Step 1)

**HIGH confidence**, every Bevy 0.18.1 lighting/fog API verified on-disk in `bevy_*-0.18.1/`. Δ deps = 0 achievable.

### CRITICAL new finding the planner needs

**`bevy::Color: Serialize/Deserialize` is feature-gated behind `bevy_color/serialize`** which Druum does NOT enable. Verified at `bevy_color-0.18.1/Cargo.toml:58-61` and `bevy-0.18.1/Cargo.toml:2322-2330`. The `bevy/3d` umbrella does NOT pull in `serialize`. A naive `pub color: Color` field deriving `Serialize/Deserialize` will fail to compile. Solution: `ColorRgb(f32, f32, f32)` wrapper struct with `into_color()` builder, keeps Δ deps = 0 + Cargo.toml byte-unchanged.

### Spec's "cap shadow-casting torches at 4 per visible region" is NOT a Bevy API

Bevy 0.18 sorts `shadows_enabled: true` lights to front, then by **entity ID** (not distance), then truncates at `max_texture_array_layers / 6`. Recommendation: author the cap into the asset (`shadows: true` on at most 3-4 entries). Zero LOC; deterministic. Explicit nearest-N system feasible (~30 LOC) but YAGNI for difficulty 2/5.

### `DistanceFog::default()` is `Linear { 0.0, 100.0 }` — invisible at dungeon scale

Always specify `falloff: FogFalloff::Exponential { density: 0.12 }` explicitly.

### Two Category B decisions for user at plan-approval

**Decision 1 — Player-torch reconciliation:**
- A. KEEP player torch + ADD cell torches (Wizardry-canonical; lights stack additively)
- B. REPLACE player torch with cell torches only (purest authoring control; floors must be densely torch-lit)
- C. Per-floor RON `carried_torch: bool` flag (matches roadmap "per-floor mood" direction; ~+20 LOC)

**Decision 2 — Module split (mod.rs is 1355 LOC):**
- A. Stay single-file (consistent with #7/#8; grows to ~1500-1600)
- B. Extract `renderer.rs` (geometry+lighting; cleanest end state, largest diff, touches #8 code)
- C. Extract `lighting.rs` only (smallest move; doesn't touch #8 code)

### HIGH-confidence recommendations on everything else

| Question | Recommendation |
| --- | --- |
| Where flicker runs | `Update`, `run_if(in_state(GameState::Dungeon))`, filter `Query<(&mut PointLight, &Torch)>` |
| Flicker formula | `1.0 + 0.10*sin(t*6.4 + phase) + 0.05*sin(t*23 + phase*1.7)`, ±15% peak amplitude. Per-entity phase from `(x*31)^(y*17)` hash. No noise crate needed. |
| Shadow cap | Author `shadows: true` on at most 3-4 entries; trust Bevy's stable sort |
| `DungeonFloor` schema | `Vec<TorchData { x, y, color: ColorRgb, intensity, range, shadows }>` with `#[serde(default)]` |
| Per-floor fog RON | `lighting: LightingConfig { fog: FogConfig { color, density }, ambient_brightness, carried_torch }` with `#[serde(default)]`. `ColorRgb(f32, f32, f32)` wrapper for any Color. |
| Test patterns | Layer 2 from #7/#8. `TimeUpdateStrategy::ManualDuration` for deterministic flicker tests. 5th-feature `init_resource::<ButtonInput<KeyCode>>()` gotcha still applies. |

### Bevy 0.18 API verifications

1. `DistanceFog` Component, `bevy_pbr-0.18.1/src/fog.rs:49-72`. Fields: `color`, `directional_light_color`, `directional_light_exponent`, `falloff`. `#[extract_component_filter(With<Camera>)]`.
2. `FogFalloff` enum: `Linear`, `Exponential`, `ExponentialSquared`, `Atmospheric`. Plus `from_visibility(v)` Koschmieder helper.
3. `PointLight` Component, `bevy_light-0.18.1/src/point_light.rs:41-126`. Lumens-based intensity (default 1M), default range 20.0, `shadows_enabled: false` default.
4. `AmbientLight` per-camera Component (`#[require(Camera)]`); `GlobalAmbientLight` scene-wide Resource. Both still in 0.18. Druum #8 already uses `GlobalAmbientLight` correctly.
5. Shadow cap: stable sort by entity ID (not distance), truncates at `max_texture_array_layers / 6`.
6. Time API: `time.elapsed_secs()` and `time.delta_secs()`. Druum's existing animation code uses correct names.
7. No noise crate needed; sin-of-sums + per-entity hash phase produces uncorrelated flicker indistinguishable from Perlin at 60Hz.

## User Decisions

### Decision 1 — Player-torch reconciliation: OPTION A with two modifications

- KEEP the existing player-attached `PointLight` (carried torch) AND ADD cell-anchored torches per the roadmap.
- **Do NOT modify the carried torch's properties.** It stays at `intensity: 60_000.0`, `range: 12.0`, `color: srgb(1.0, 0.85, 0.55)`, `shadows_enabled: false`, parented to `DungeonCamera`. No compensating intensity drop for additive stacking under cell torches — the user explicitly accepts bright spots under sconces. User quote: *"the code is the source of truth so don't change it."* The planner should propagate this principle: where the spec/research and the existing code disagree on the carried torch, trust the code.
- **Carried torch DOES flicker.** Same flicker system as cell torches, but with a `phase_offset = π` (or any per-entity hash result that desyncs it from cell torches) so the carried torch doesn't pulse in sync with sconces.
- The schema, RON, and `spawn_torch` work for cell torches still proceeds as the roadmap describes — schema gets the `light_positions` field, `floor_01.dungeon.ron` gets 3-4 sample torch positions, etc.
- Per-floor RON `LightingConfig { fog, ambient_brightness, ... }` proceeds as the research recommended for fog + ambient. Do NOT add a `carried_torch: bool` toggle for #9 — that was Option C and was not selected. Carried torch is unconditional.

### Decision 2 — Module split: OFF-MENU OPTION D (user-proposed)

- The user rejected Options A/B/C (stay single-file, extract `renderer.rs`, extract `lighting.rs`) and proposed instead: **extract the `#[cfg(test)] mod tests { ... }` block from `src/plugins/dungeon/mod.rs` into a separate file** to cut down `mod.rs` size. User quote: *"can you move the testing code to somewhere else, that would cut down on the file size a lot."*
- Standard Rust pattern: declare `#[cfg(test)] mod tests;` in `mod.rs`, move the body to `src/plugins/dungeon/tests.rs`. This is a pure file-move (test module visibility into the parent's items still works because it's a child module).
- This refactor should be a dedicated step (or two) in the plan, ordered EARLY — before any new lighting code is added — so the lighting work lands in a smaller `mod.rs`. Suggested ordering: this becomes Step 1 or Step 2 of the plan, before any lighting implementation steps.
- Verify all existing tests still pass after the move (no behavioral change). The module move must keep test count and behavior identical (`cargo test` shows the same 61 lib + 3 integration baseline). Watch for `super::` imports inside the test module — they need to keep working from the new file location.
- This is **in addition to** all the §9 lighting work, not a substitute for it. `mod.rs` stays single-file for production code; tests just relocate.
