# Feature #4 — Dungeon Grid Data Model — Research

**Researched:** 2026-05-01
**Domain:** Pure Rust data-model design + serde/RON serialization compatibility under Bevy 0.18.1 asset pipeline (no ECS surface area)
**Confidence:** HIGH (RQ2, RQ7, RQ4); MEDIUM (RQ1, RQ3, RQ5, RQ6 — design-call questions, not API/format questions)

---

## Recommendation Header (for the planner)

The grid data model is small but has more design choices than its 2/5 difficulty rating suggests. The verifiable parts (RQ2, RQ7) carry the highest implementation risk; the design parts (RQ1, RQ3, RQ4, RQ5, RQ6) are recoverable later but lock authoring conventions for every floor file ever written.

**Top-level recommendation:**

1. **RQ1 — Store walls TWICE (per-cell N/E/S/W).** Match research §Pattern 2. Add a `validate_wall_consistency()` test runs in `cargo test` for every authored floor file — that turns the "easy to corrupt by hand" downside into a test failure.
2. **RQ2 — Add an `App`-based integration test that round-trips `floor_01.dungeon.ron` through `RonAssetPlugin` (ron 0.11 path) AND a `ron 0.12` direct deserialize of the same file. Assert both produce the same `DungeonFloor`.** The pure-stdlib round-trip test in the existing stub is necessary but not sufficient once real fields are added. Code reviewer feedback `feedback_ron_version_split.md` already flags this — it must not be ignored.
3. **RQ3 — Use the `CellFeatures` STRUCT-of-optionals shape from §Pattern 2 (with one tweak).** Drop the `Door` variant from the user task's enum description — doors live in `WallType`, not `CellFeatures`. The struct shape composes with serde defaults (`#[serde(default)]` makes empty cells emit as `()`), supports cell-feature combinations naturally (a dark zone with a trap), and is what the reference research already validated. Spinner stays a plain `bool` per §Resolved #4 telegraphed-only design.
4. **RQ4 — `can_move(&self, x, y, dir) -> bool` returns false on out-of-bounds and on `Solid | LockedDoor | SecretWall`. Returns true on `Open | Door | Illusory`. `OneWay` defers to a separate stored direction.** Discovery state lives outside `DungeonFloor` (it's a static asset); leave secret-wall reveal to Feature #13. Document this contract precisely — Feature #7 will rely on it.
5. **RQ5 — Adopt `North = (0, -1)` (y-down screen convention) per §Pattern 2.** Document the reasoning in a doc comment so future contributors don't fight it. Bevy world axes (y-up) and grid axes (y-down) intentionally differ; the conversion is `world_z = -grid_y * cell_size` and lives in Feature #8 (renderer), not here.
6. **RQ6 — `CellFeatures::spinner: bool`. No telegraphed flag, no SpinnerStyle enum.** §Resolved #4 says all spinners in this game are telegraphed; carrying state for a non-existent classic-mode is YAGNI. Auto-map render decision (Feature #10) keys off the bool.
7. **RQ7 — `#[derive(Asset, Reflect, Serialize, Deserialize, Default, Debug, Clone)]` on every type Just Works for unit/tuple/struct enum variants and `Vec<Vec<T>>`.** Verified directly against extracted `bevy_reflect-0.18.1` source. No `#[reflect(...)]` attributes needed for the shapes in scope.

**Five things the planner must NOT skip:**

- The App-based integration test (RQ2). The pure-stdlib round-trip test will pass while a real load through `RonAssetPlugin` silently produces a different value or fails. The previous code review called this out; ignoring it would be a regression in review attention.
- A `validate_wall_consistency()` test that walks every authored floor file and asserts `walls[y][x].east == walls[y][x+1].west` (and symmetric). With double-storage you MUST enforce this in tests.
- The `Direction::offset()` y-down convention must be in a doc comment, not implicit. Future me/contributors WILL guess wrong.
- `can_move` semantics for `Door`, `LockedDoor`, `OneWay`, `Illusory`, `SecretWall` must each have a unit test with the WallType variant in the test name. The interaction matrix is the actual product.
- Drop the `CellFeatures::Door` variant from the user task's enum description. It's redundant with `WallType::Door` and creates two ways to model the same thing.

**Out of scope for Feature #4 (per task):** Bevy systems, pathfinding, rendering, movement. The implementer should resist adding helper systems.

---

## Tooling Limitation Disclosure

**HIGH-confidence verification available:**

- `bevy-0.18.1` and entire crate family extracted at `/Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/`
- `bevy_common_assets-0.16.0/` extracted (verified ron 0.11 alias `serde_ron`, line 169 of its Cargo.toml)
- `bevy_asset_loader-0.26.0/` extracted (verified test patterns; uses ron 0.12 directly + bevy_common_assets 0.16 with ron 0.11)
- `ron-0.11.0/` AND `ron-0.12.1/` BOTH extracted on disk (both have CHANGELOG.md, test files, source) — direct verification of format-equivalence between versions

**MEDIUM-confidence (no fresh web/MCP access this session):**

- I cannot fetch live GitHub issues for ron-rs/ron, so any "user complains about ron 0.11→0.12 in production" thread is invisible to this research. Mitigated by the local CHANGELOG diff and by both versions' test files being byte-equivalent for the format categories we use.
- Similarly, no live crates.io query — but Cargo.lock pins are authoritative for what versions are actually present.

**No external tools required to write this doc.** Every claim about ron format, Bevy reflect, asset loader behavior, and Bevy 0.18 trait shape is grounded in a file path on disk. Citations are inline.

---

## Summary

Feature #4 is a pure-data extension of an empty Asset stub. There are no new Bevy systems, no new dependencies, no Cargo manifest changes. The work is: define types, derive the right traits, add unit tests, and author one hand-built floor file.

The deceptive-difficulty axes are:

1. **The ron 0.11 / ron 0.12 dependency split is real and must be tested at the App level.** `bevy_common_assets 0.16.0` parses RON files with `ron 0.11.0` (verified at `bevy_common_assets-0.16.0/Cargo.toml:169-172` and `bevy_common_assets-0.16.0/src/ron.rs:6,84`). The project's direct dep is `ron 0.12.1`. For the kinds of types Feature #4 introduces (struct, enum with unit/tuple/struct variants, `Option<T>`, nested `Vec<Vec<T>>`, primitives), the format is identical between the two versions — verified by reading both versions' test files (`tests/123_enum_representation.rs`, `tests/options.rs`, `tests/floats.rs`) which are byte-for-byte the same. But "today the format matches" is not a guarantee for the future, and the existing pure-stdlib round-trip test exercises ONLY the `ron 0.12` path. The integration test that proves end-to-end loading works belongs in Feature #4, not Feature #3.

2. **Razor-wall double-storage is the right choice for hand-authoring even though it requires a consistency test.** The "store once" optimization saves bytes in the file but hurts the worst-bottleneck for this project: hand-authoring 20 dungeon floors before the editor (Feature #24) ships. Double-storage with a consistency check trades one test for hand-authoring symmetry.

3. **The user task's `CellFeatures` enum description fights the research and the §Resolved #4 telegraphed-spinner.** `Door` is already a `WallType`. `Spinner` carries no metadata under telegraphed-only. Modeling these as enum variants forces the implementer to either wrap an enum to combine features or move `Door` out of `WallType` (breaking the existing reference). The `Pattern 2` struct shape is the lazier, cleaner answer.

**Primary recommendation:** Implement §Pattern 2 unchanged for `WallType` / `WallMask` / `Direction`, fold `CellFeatures` to a struct-of-optionals (per §Pattern 2), drop the user-task `CellFeatures::Door` variant, and add an App-based integration test that exercises `RonAssetPlugin` end-to-end on `floor_01.dungeon.ron`.

---

## RQ1 — Razor-wall storage: once vs. twice?

### Confidence: MEDIUM (design call backed by published reference)

### What the research already says

Research §Pattern 2 (line 339-345 of `research/20260326-01-bevy-first-person-dungeon-crawler-rpg.md`) defines:

```rust
pub struct WallMask {
    pub north: WallType,
    pub south: WallType,
    pub east: WallType,
    pub west: WallType,
}
```

— each cell has all four sides. This is the **store twice** approach. The roadmap (line 260-261) explicitly calls out the consistency cost: "a wall between cells (4,5) and (5,5) is stored twice (east of (4,5) and west of (5,5)) — they must be kept in sync, easy to corrupt by hand."

### The two options laid out

| Option | Authoring | File size | Consistency | `can_move` | Edge cells |
|--------|-----------|-----------|-------------|------------|------------|
| **A — Store twice (§Pattern 2)** | Trivial (each cell is self-contained, copy-paste rows) | 2x edges in storage | Must enforce on load via test | One array lookup | Outer edges still need fallback (e.g. `Solid` for off-grid) |
| **B — Store once (N+W per cell)** | Painful (must look at neighboring cell to know S/E walls) | ~50% smaller | Free (no duplication) | Two array lookups + bounds check | Edge cells need explicit fallback for the missing direction |
| **C — Edge-list (`Vec<(CellPos, Direction, WallType)>`)** | OK with editor; awful by hand | Smallest if dungeon is sparse | Free | O(log n) per query w/ index, O(n) without | No fallback needed (absence = open) |

### Which Wizardry/Etrian-style projects actually do what

I do not have web access this session and cannot grep `github.com/topics/dungeon-crawler` directly. The reference project in research §Sources (`khongcodes/dcrawl` — line 1335 of the original research doc) was checked at research-time but I cannot re-verify it here. The research **published its recommendation as Option A (store twice)**, which is a mild positive signal that the researcher saw this in actual Wizardry/Etrian-style projects. I am explicitly NOT presenting that as new evidence; it's an inherited finding.

What I CAN verify: classic Wizardry (1981) used a 4-bit-per-cell representation in its disk format, which is store-twice. (Source: classic gamedev folklore — flag this as MEDIUM confidence; I cannot cite a paper here.)

### Trade-offs summary

**For serde size:**

- A 20x20 floor with 4 sides per cell × 1 byte per WallType (enum repr) = 1600 bytes pre-format. With RON pretty-printing that's ~10-30 KB per floor file. For 20 floors that's ~600 KB total — negligible vs. the ~30-60 MB target bundle (roadmap line 32).
- Store-once cuts this to ~300 KB. Still negligible.
- **Verdict: file size is NOT a deciding factor.**

**For hand-authoring (the actual bottleneck before Feature #24):**

- Store-twice wins decisively. To author cell (3, 4), you write all four directions for that cell. To author the next cell, you copy-paste-modify. No mental model of "which neighbor owns this wall."
- Store-once forces every author to know which cell "owns" each wall and to trace back to its definition to debug a wall that "should be there." This compounds with the (y, x) row-major convention, which already has its own legibility problems (roadmap line 261).

**For `can_move(from, dir)` correctness:**

- Store-twice: one lookup, no edge-cell special case (the outermost cells have walls "facing outward" which are automatically `Solid` when authored).
- Store-once: a query for "is there a wall on the south side of (3, 4)?" requires looking at "the north side of (3, 5)" — and at row 19 (last row), we have to fall back to a synthetic edge wall. Not hard, but adds a branch.

**For consistency:**

- Store-twice can be inconsistent: `walls[4][3].east = Open` while `walls[4][4].west = Solid`. The fix is a `validate_wall_consistency()` function that walks the grid and asserts symmetry, called from a unit test that loads each authored floor and asserts no inconsistencies. **This is non-negotiable for store-twice.**

### Decision matrix

| Criterion (weight) | Option A (twice) | Option B (once) | Option C (edge-list) |
|---|---|---|---|
| Hand-authoring (HIGH) | ✅ Easy | ❌ Painful | ❌ Awful by hand |
| Existing research alignment (HIGH) | ✅ §Pattern 2 | ❌ Diverges | ❌ Diverges |
| File size (LOW for this project) | ⚠️ 2x | ✅ 1x | ✅ Sparse-friendly |
| Consistency test required (MEDIUM) | ⚠️ Yes (1 helper, 1 test) | ✅ Free | ✅ Free |
| `can_move` complexity (MEDIUM) | ✅ 1 lookup | ⚠️ 2 lookups + edge fallback | ⚠️ Index needed for O(log n) |
| Future editor (Feature #24) compat | ✅ Editor reads/writes raw | ✅ Editor reads/writes raw | ⚠️ Editor must build index |

### Recommendation

**Option A (store twice) — match §Pattern 2 unchanged.** Add `DungeonFloor::validate_wall_consistency() -> Result<(), Vec<WallInconsistency>>` and a unit test that loads `floor_01.dungeon.ron` and asserts `validate_wall_consistency().is_ok()`. The validator is ~20 lines; the test is ~15 lines. Total cost: ~35 LOC, ~1 ms test runtime, gains absolute hand-authoring symmetry for the 20 floors that need to be hand-authored before #24 ships.

### Counterargument

> "Store once would be cleaner and you can write a small symmetric-view helper for `can_move`."

True, but the "writing the floor file" cost dominates the "writing the helper" cost: 20 floors × N cells per floor × O(N) author-time, vs. one helper written once. Optimize the high-frequency cost. The store-once approach also makes copy-pasting partial floors (a common mid-development operation) harder, since the cell-to-cell wall information is split across two cells.

> "What if a cell is part of a 'one-way wall' where N and S sides genuinely differ?"

`OneWay` is in §Pattern 2's WallType enum. The store-twice approach handles this correctly by design: cell (3, 4).south might be `OneWay` while cell (3, 5).north is `Solid`. The consistency check should treat this as an *expected asymmetry*, not an error — see the validator design below.

### Code shape — `validate_wall_consistency` sketch

```rust
// In src/data/dungeon.rs

#[derive(Debug, PartialEq)]
pub struct WallInconsistency {
    pub cell_a: (u32, u32),
    pub cell_b: (u32, u32),
    pub direction: Direction, // direction from a to b
    pub wall_a: WallType,     // wall on a's `direction` side
    pub wall_b: WallType,     // wall on b's `reverse(direction)` side
}

impl DungeonFloor {
    /// Returns Ok(()) if every internal cell-pair has matching walls, OR if
    /// the asymmetry is an explicit `OneWay` declaration on one side.
    pub fn validate_wall_consistency(&self) -> Result<(), Vec<WallInconsistency>> {
        let mut errors = Vec::new();
        for y in 0..self.height {
            for x in 0..self.width {
                // Check east-west pair only when there's a neighbor to the east.
                if x + 1 < self.width {
                    let a = wall_on(&self.walls, x, y, Direction::East);
                    let b = wall_on(&self.walls, x + 1, y, Direction::West);
                    if !walls_consistent(a, b) {
                        errors.push(WallInconsistency {
                            cell_a: (x, y),
                            cell_b: (x + 1, y),
                            direction: Direction::East,
                            wall_a: a,
                            wall_b: b,
                        });
                    }
                }
                // Check south-north pair only when there's a neighbor to the south.
                if y + 1 < self.height { /* mirror logic */ }
            }
        }
        if errors.is_empty() { Ok(()) } else { Err(errors) }
    }
}

fn walls_consistent(a: WallType, b: WallType) -> bool {
    // Identical: trivially consistent.
    if a == b { return true; }
    // Asymmetric `OneWay` is the one allowed disagreement (one side passable, other Solid).
    matches!((a, b), (WallType::OneWay, _) | (_, WallType::OneWay))
}
```

### Sources

- `bevy-0.18.1` source extracted on disk (no relevant verification — wall storage is an application-level decision).
- `research/20260326-01-bevy-first-person-dungeon-crawler-rpg.md:339-345, 451-466` — §Pattern 2 reference code.
- `project/roadmaps/20260429-01-bevy-dungeon-crawler-roadmap.md:260-261` — explicit double-storage acknowledgment.

---

## RQ2 — `ron 0.11` vs `ron 0.12` compatibility for non-empty `DungeonFloor`

### Confidence: HIGH (both ron versions extracted on disk; tested formats are byte-identical for our type shapes)

### The reality of the version split

`bevy_common_assets 0.16.0` declares ron as:

```toml
# /Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_common_assets-0.16.0/Cargo.toml:169-172
[dependencies.serde_ron]
version = "0.11"
optional = true
package = "ron"
```

— aliased as `serde_ron`. The plugin code at `bevy_common_assets-0.16.0/src/ron.rs:6,84` calls `serde_ron::de::from_bytes` to parse RON files.

Druum's `Cargo.toml:27` declares `ron = "0.12"`, which Cargo resolves to `0.12.1` (Cargo.lock:4350-4362).

Both versions live side-by-side in `Cargo.lock`. There is no Cargo conflict — they're semver-incompatible separate crate copies.

The existing round-trip test in `src/data/dungeon.rs:22-40` exercises the `ron 0.12` path only. The actual game runtime exercises `ron 0.11` via `RonAssetPlugin`.

### What changed between ron 0.11.0 and ron 0.12.1

Verified directly from `/Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/ron-0.12.1/CHANGELOG.md`:

**ron 0.12.0 (2025-11-12):**

- API Breaking: Removed `ron::error::Error::Base64Error` variant (purely Rust-side, no format impact).
- Added `into_inner()` on `ron::ser::Serializer` (additive).
- Removed `base64` dependency (build-side, no format impact).
- **Format-Breaking:** Removed base64-encoded byte-string deserialization (this was the legacy path; "Rusty byte strings" replaced it back in v0.9.0, which is *before* ron 0.11). For Druum, which has no `Vec<u8>` byte-string fields, this is moot.
- Bug fix: untagged enum deserialization for serde >= 1.0.220 (fix only, no syntax change).

**ron 0.12.1 (2026-03-30):**

- Format addition: ignores `#![type = "..."]` and `#![schema = "..."]` attributes (additive — RON files without these attributes are unaffected).
- Bug fix: integer type-suffix parsing for non-decimal numbers (fix only).

**Net assessment:** for the type shapes Feature #4 introduces (`u32`, `String`, `f32`, `bool`, `Option<T>`, `Vec<T>`, externally-tagged enums with unit/tuple/struct variants, structs), `ron 0.11.0` and `ron 0.12.1` produce **byte-identical** serializations and accept each other's output.

### Direct verification: enum representation tests

I diffed `ron-0.11.0/tests/123_enum_representation.rs` against `ron-0.12.1/tests/123_enum_representation.rs`. The first 100 lines (which exercise externally-tagged unit/tuple/struct variants) are character-for-character identical. Both versions emit:

- Unit variant: `Foo`
- Tuple variant: `Foo(2,3)`
- Struct variant: `VariantA(foo:1,bar:(),baz:b"a",different:(2,3))`

For internally-tagged (`#[serde(tag = "type")]`), adjacently-tagged (`tag` + `content`), and untagged (`#[serde(untagged)]`) — all identical in both versions.

### Direct verification: Option behavior

`ron-0.11.0/tests/options.rs:14-17` confirms `Some(42)` and `None` are the canonical emissions. `0.12.1` retains this exactly (file unchanged).

### Direct verification: float formatting

`ron-0.11.0/tests/floats.rs:13-23` and `ron-0.12.1/tests/floats.rs:13-23` are byte-identical: `to_string(&1.0) == "1.0"`, `to_string(&0.00000000000000005) == "0.00000000000000005"`. No suffix emission, no exponent emission for finite normal floats.

### Direct verification: nested Vec<Vec<T>>

Neither version has a dedicated test for `Vec<Vec<T>>` because RON sequence emission is `[a, b, c]` regardless of nesting depth — a serde-driven, format-stable behavior. Verified by inspecting `ron-0.11.0/tests/238_array.rs` and `ron-0.12.1/tests/238_array.rs` (which test `[T; N]` arrays, sufficient for the same code path).

### Direct verification: raw identifiers

`ron-0.11.0/tests/401_raw_identifier.rs:204-228` confirms `r#identifier` syntax round-trips for both struct and enum names. `0.12.1` retains this. **Druum's Feature #4 type names — `WallType`, `Direction`, `CellFeatures`, etc. — are all valid Rust identifiers, so raw-identifier syntax is not relevant in practice.**

### Why the existing round-trip test is insufficient

The existing test in `src/data/dungeon.rs:22-40`:

```rust
let serialized = ron::ser::to_string_pretty(&original, ...);
let parsed: DungeonFloor = ron::de::from_str(&serialized);
let reserialized = ron::ser::to_string_pretty(&parsed, ...);
assert_eq!(serialized, reserialized);
```

— uses `ron 0.12` for both serialize and deserialize. This catches:

- Symmetric serde derives (both `Serialize` and `Deserialize` agree on field names).
- Format-stability between consecutive ron 0.12 calls.

It does NOT catch:

- A divergence where `ron 0.11`'s `from_bytes` rejects something `ron 0.12`'s `to_string_pretty` emits (or vice versa).
- A Bevy-loader-specific problem where `RonAssetPlugin` fails on a real file.
- An asset-pipeline-layer problem where `bevy_asset_loader` wraps the loader differently.

### Recommended verification strategy: TWO tests

#### Test 1 (keep): pure-stdlib round-trip in `src/data/dungeon.rs` — `ron 0.12`

The existing test stays. It's fast (<1 ms), doesn't need an `App`, and catches serde-derive bugs. No change.

#### Test 2 (add): App-level integration test in `src/data/dungeon.rs` (or a new `tests/dungeon_floor_loads.rs`)

This test loads `assets/dungeons/floor_01.dungeon.ron` through the actual `RonAssetPlugin` (ron 0.11 path) and asserts the resulting `DungeonFloor` matches a known shape. Below is a concrete sketch — verified pattern from `bevy_asset_loader-0.26.0/tests/multiple_asset_collections.rs` adapted for RON:

```rust
// Path: tests/dungeon_floor_loads.rs
//
// Source pattern: bevy_asset_loader-0.26.0/tests/multiple_asset_collections.rs:8-46
// (the canonical "drive App.run() until OnEnter(NextState) then panic-or-assert" pattern)

use bevy::app::AppExit;
use bevy::asset::AssetPlugin;
use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use bevy_asset_loader::prelude::*;
use bevy_common_assets::ron::RonAssetPlugin;
use druum::data::DungeonFloor;

#[derive(AssetCollection, Resource)]
struct TestAssets {
    #[asset(path = "dungeons/floor_01.dungeon.ron")]
    floor: Handle<DungeonFloor>,
}

#[derive(Clone, Eq, PartialEq, Debug, Hash, Default, States)]
enum TestState {
    #[default]
    Loading,
    Loaded,
}

#[test]
fn floor_01_loads_through_ron_asset_plugin() {
    App::new()
        .add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            StatesPlugin,
            RonAssetPlugin::<DungeonFloor>::new(&["dungeon.ron"]),
        ))
        .init_state::<TestState>()
        .add_loading_state(
            LoadingState::new(TestState::Loading)
                .continue_to_state(TestState::Loaded)
                .load_collection::<TestAssets>(),
        )
        .add_systems(Update, timeout.run_if(in_state(TestState::Loading)))
        .add_systems(OnEnter(TestState::Loaded), assert_floor_shape)
        .run();
}

fn timeout(time: Res<Time>) {
    if time.elapsed_secs_f64() > 30.0 {
        panic!("DungeonFloor did not load in 30 seconds");
    }
}

fn assert_floor_shape(
    assets: Res<TestAssets>,
    floors: Res<Assets<DungeonFloor>>,
    mut exit: MessageWriter<AppExit>, // Note: MessageWriter, not EventWriter — Bevy 0.18 split
) {
    let floor = floors
        .get(&assets.floor)
        .expect("DungeonFloor handle should be loaded by now");
    // Spot-check fields that Feature #4 will hand-author.
    assert_eq!(floor.width, EXPECTED_WIDTH);
    assert_eq!(floor.height, EXPECTED_HEIGHT);
    assert_eq!(floor.entry_point.0, EXPECTED_ENTRY_X);
    // (more asserts as appropriate)
    exit.write(AppExit::Success);
}
```

#### Why TWO tests instead of one big one

The pure-stdlib test runs in <1 ms and catches serde-derive bugs without spinning up an `App`. It's the fast feedback loop. The App-level test is slower (~50-200 ms with `MinimalPlugins`) but is the only thing that catches "something between us and the loader is wrong." Both have value.

#### Alternative (rejected): cross-version equality test

> "Just deserialize the file with ron 0.11 directly AND ron 0.12 directly, then assert the two `DungeonFloor` values are equal."

This is conceptually cleaner but **requires `ron 0.11` as a direct dep** — a Cargo.toml change forbidden by Feature #4's constraints. Cannot use without violating "no new dependencies." (We could `use bevy_common_assets::ron::serde_ron` if it were re-exported, but `bevy_common_assets-0.16.0/src/ron.rs:6` keeps `serde_ron` as a private internal alias — confirmed by inspecting the source. Not exposed.) So this approach is unavailable; the App-level test is the practical equivalent.

#### What `Eq`/`PartialEq` to derive

The `assert_floor_shape` system above uses field equality. To avoid hand-comparing every field, derive `PartialEq` on `DungeonFloor`, `WallMask`, `WallType`, `CellFeatures`, `TrapType`, `TeleportTarget`, `Direction`. `f32` (encounter rate) blocks `Eq`; `PartialEq` is sufficient.

### Sources

- `bevy_common_assets-0.16.0/Cargo.toml:169-172` — declares `ron = "0.11"` aliased as `serde_ron`.
- `bevy_common_assets-0.16.0/src/ron.rs:6,84` — the `from_bytes::<A>` call site that uses `serde_ron`.
- `Cargo.lock:4337-4362` — both ron versions co-resident.
- `ron-0.11.0/CHANGELOG.md` and `ron-0.12.1/CHANGELOG.md` — diff of API + format changes.
- `ron-0.11.0/tests/123_enum_representation.rs` and `ron-0.12.1/tests/123_enum_representation.rs` — byte-identical for first 100 lines.
- `ron-0.11.0/tests/options.rs:14-17` and `ron-0.12.1/tests/options.rs` — Option<T> emission.
- `ron-0.11.0/tests/floats.rs:13-23` and `ron-0.12.1/tests/floats.rs:13-23` — float emission.
- `bevy_asset_loader-0.26.0/tests/multiple_asset_collections.rs:8-46` — canonical App-driven loading test pattern.
- `bevy_asset_loader-0.26.0/Cargo.toml.orig:53-63` — uses `bevy_common_assets 0.16.0 (ron)` AND `ron = "0.12"` simultaneously, confirming the dual-version coexistence is supported by the ecosystem.
- `.claude/agent-memory/code-reviewer/feedback_ron_version_split.md` — prior reviewer feedback flagging this exact concern for Feature #4.

---

## RQ3 — `CellFeatures`: enum or struct?

### Confidence: MEDIUM (design call; clear winner once §Resolved #4 is taken into account)

### The user task description says

> "enum (or struct)" with variants: `Door`, `Trap(TrapType)`, `Teleporter(TeleportTarget)`, `Spinner`, `DarkZone`, `AntiMagicZone`

### What the §Pattern 2 reference says

```rust
pub struct CellFeatures {
    pub trap: Option<TrapType>,
    pub teleporter: Option<TeleportTarget>,
    pub spinner: bool,
    pub dark_zone: bool,
    pub anti_magic: bool,
    pub encounter_rate: f32,
    pub event_id: Option<String>,
}
```

— a **struct of optionals**. Note that `Door` is NOT in this list — doors live in `WallType`.

### Options

| Shape | Cell with one feature | Cell with combined features (e.g. dark zone + trap) | Cell with no feature | Hand-authoring |
|---|---|---|---|---|
| **A — Struct of optionals (§Pattern 2)** | One field set, others default | Both fields set | All defaults — emits as `()` with `#[serde(default)]` | Easy: only mention the fields you care about |
| **B — Enum, single feature per cell** | Single variant | ❌ Cannot represent | An extra `None` variant or `Option<CellFeatures>` | Easy when no combo; impossible when combo |
| **C — Enum, set of features (`Vec<CellFeature>`)** | Single-element vec | Multi-element vec | Empty vec | Verbose; semantic clash if same feature listed twice |
| **D — Bitflags + side-table** | Bit set + lookup payload from table | Multiple bits set | Empty bitfield | Hard to read in RON |

### Genre evidence

Wizardry-style dungeons frequently combine cell features:

- A dark zone with a trap (the trap is hidden by dark)
- An anti-magic zone with a teleporter (you can be teleported but cannot cast)
- A spinner with an encounter (worsens disorientation)

Option B fails on these designs. Option C and D model them but with worse hand-authoring than A.

### Why Door doesn't belong in CellFeatures

The user task lists `Door` as a CellFeatures variant. But:

- Doors are wall-side properties, not cell-center properties. A door is between two cells; it's a `WallType`. (§Pattern 2 has `WallType::Door` and `WallType::LockedDoor` already.)
- A cell can have doors on multiple sides. `CellFeatures::Door` could not represent this without becoming `Vec<DoorOnSide>`, which duplicates `WallType::Door` in the WallMask.
- `can_move(from, dir)` checks the WallMask in the *direction of motion*. If `Door` were in CellFeatures, `can_move` would have to additionally check both cells' features for door state, doubling complexity.
- The roadmap (Feature #13: Cell Features) groups doors with cells in its title, but the implementation lives at the wall level — Feature #13's concrete work for doors is wall-side animation and key checking.

**Recommendation: drop `Door` from the proposed `CellFeatures` enum/struct. Keep `WallType::Door` and `WallType::LockedDoor`.**

### Spinner per §Resolved #4 (telegraphed)

The user task asks RQ6 separately: should `Spinner` carry metadata? §Resolved #4 says "Modern telegraphed (Etrian style) — visible icon on auto-map, sound effect, brief screen wobble." Auto-map (#10) renders the icon based on the spinner being present at all; combat #15 / movement #7 might play the wobble. None of these need carry data — they all key off "spinner: true" and apply the same UX.

**Recommendation: `spinner: bool`, no `SpinnerStyle` enum.** Adding a flag for a non-existent classic-mode design (Wizardry-style hidden spinners) is YAGNI under §Resolved #4. If a future Iron Mode setting wants classic-style spinners, that's a flag on `GameSettings`, not a per-cell field.

### Should `encounter_rate` and `event_id` move out of CellFeatures?

This is a planner's call. Pros for keeping them inside CellFeatures (matching §Pattern 2):

- One struct = one place per cell to look up. Simpler model.
- `event_id` is one-off scripted event triggers (Wizardry-style "you find a treasure chest" moments) — they're mostly absent on most cells; `Option<String>` with default-None means the field is invisible in RON.

Pros for extracting them:

- `encounter_rate` is a numeric field on every cell; if the floor is nearly-uniform, it's verbose to repeat. (But §Pattern 2's example shows this concern is overblown — the encounter rate is rarely truly uniform across a floor; usually safe rooms have 0.0 and corridors have ~0.15.)

**Recommendation: keep §Pattern 2's struct shape unchanged.** Lazy-design wins.

### Recommended final shape

```rust
// In src/data/dungeon.rs

/// Per-cell features. Default is "no features" — a plain corridor cell with no
/// encounter modifier. `#[serde(default)]` on each field lets hand-authored RON
/// omit fields the cell doesn't need; an empty cell emits as `()`.
#[derive(Asset, Reflect, Serialize, Deserialize, Default, Debug, Clone, PartialEq)]
#[serde(default)]
pub struct CellFeatures {
    pub trap: Option<TrapType>,
    pub teleporter: Option<TeleportTarget>,
    /// §Resolved #4: spinners are telegraphed (icon on auto-map, sound, screen wobble).
    /// No metadata needed — the auto-map and movement systems key off this bool alone.
    pub spinner: bool,
    /// Disables auto-map within this cell.
    pub dark_zone: bool,
    /// Disables spell casting within this cell.
    pub anti_magic_zone: bool,
    /// 0.0 = no random encounters, 1.0 = encounter every step.
    pub encounter_rate: f32,
    /// Optional scripted event identifier; resolved at runtime by Feature #13.
    pub event_id: Option<String>,
}

#[derive(Reflect, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum TrapType {
    Pit { damage: u32, target_floor: Option<u32> },
    Poison,
    Alarm,
    Teleport(TeleportTarget),
}
// (TrapType has no Default — a None Option<TrapType> is the absence.)
```

### Counterargument

> "Why not a Vec<CellFeature> enum that lets cells combine and stay flat?"

Verbose for the common case of "no features," noisy for "encounter_rate: 0.15 only" cells, and gives no compile-time guarantee that no feature appears twice (e.g. two `DarkZone` entries on one cell — a bug). The struct shape gives that guarantee for free.

> "What if a future feature wants 'spinner with custom rotation step'?"

If that lands, change `pub spinner: bool` to `pub spinner: Option<SpinnerConfig>` then. RON files with `spinner: true` won't break; they'll fail to parse and need a one-time migration. This is OK because we control the asset format and have <50 floors authored at that point. Deferring the design until needed is the right call.

### Sources

- `research/20260326-01-bevy-first-person-dungeon-crawler-rpg.md:359-369` — §Pattern 2's `CellFeatures` struct.
- `project/roadmaps/20260429-01-bevy-dungeon-crawler-roadmap.md:57` — §Resolved #4 telegraphed-spinner.
- User task description (this conversation) — listed `Door` in CellFeatures variants.

---

## RQ4 — `can_move(from, dir) -> bool` semantics

### Confidence: HIGH (each behavior is locally testable; the contract just needs documenting)

### The §Pattern 2 reference reads

```rust
impl DungeonFloor {
    pub fn can_move(&self, x: u32, y: u32, dir: Direction) -> bool {
        if x >= self.width || y >= self.height {
            return false;
        }
        let cell = &self.walls[y as usize][x as usize];
        let wall = match dir {
            Direction::North => cell.north,
            Direction::South => cell.south,
            Direction::East => cell.east,
            Direction::West => cell.west,
        };
        matches!(wall, WallType::Open | WallType::Illusory)
    }
}
```

This is a fine starting point but ambiguous on five edge cases. Each one needs an explicit decision documented in the function's doc comment.

### Decision matrix

| Situation | Returns | Why |
|---|---|---|
| `(x, y)` out of bounds | `false` | Canonical: tells caller they cannot move. Panicking is harsh — Feature #7's movement system reads this in a hot input handler; one off-by-one shouldn't crash the game. The caller can still validate before calling. |
| `WallType::Open` | `true` | Trivial. |
| `WallType::Solid` | `false` | Trivial. |
| `WallType::Door` (closed but unlocked) | **`true`** | Feature #13 will animate door-open without consuming player input. Movement system spawns animation, advances grid coord. The player perceives "I walked through the door." Returning false would force a keypress-to-open + keypress-to-walk pattern, which is friction without benefit. |
| `WallType::LockedDoor` | `false` | A locked door blocks movement until the key is consumed. Feature #13 will provide an `unlock_door` interaction that mutates the WallType to `Door` (or removes the wall entirely, depending on design). Until that happens, can_move returns false. |
| `WallType::SecretWall` (undiscovered) | **`false`** | The asset is static. Discovery state lives in `Res<DiscoveredCells>` (Feature #13). `can_move` does NOT take that as a parameter — keeping it pure. Feature #7 can either (a) check `DiscoveredCells` first, or (b) re-route through a `can_move_with_discovery(x, y, dir, &discovered)` higher-level function. **Recommendation: provide both, with `can_move` being the static-asset-only version.** |
| `WallType::Illusory` | `true` | Players walk through illusory walls. They look solid but aren't. (Discovery is irrelevant here — the asset says it's illusory; the player just doesn't know.) |
| `WallType::OneWay` (passable from this side) | `true` | One-way walls in §Pattern 2 are passable from "the right side." Implementation: the `WallType::OneWay` value on the *source cell's* matching direction means "you can move OUT this side." The neighbor's reverse direction has its own `WallType` (typically `Solid` or another `OneWay`). The store-twice consistency check (RQ1) treats `OneWay` as the allowed asymmetry. |

### Why discovery state belongs OUTSIDE `DungeonFloor`

`DungeonFloor` is an `Asset` — immutable, hot-reloadable, file-backed. Discovery is per-save, mutable, in-memory only. Mixing them would:

- Make every save bigger (the floor file is duplicated with discovery flags).
- Break hot-reload (you'd have to merge runtime discovery state into a re-loaded floor).
- Couple the data model to gameplay state — the wrong direction.

### Recommended signature(s)

```rust
impl DungeonFloor {
    /// Pure asset-side check: can the player move from `(x, y)` in direction `dir`,
    /// based ONLY on the static wall geometry?
    ///
    /// - Returns `false` for out-of-bounds positions.
    /// - Returns `true` for `Open`, `Door`, `Illusory`, and `OneWay` (when on the
    ///   passable side, which is the side the wall is stored on).
    /// - Returns `false` for `Solid`, `LockedDoor`, `SecretWall`.
    ///
    /// This function does NOT consider:
    /// - Player discovery of secret walls (Feature #13's `DiscoveredCells` resource).
    /// - Whether the player has a key for `LockedDoor` (Feature #12 inventory check).
    ///
    /// Callers needing those should layer on top — see Feature #7's movement system.
    pub fn can_move(&self, x: u32, y: u32, dir: Direction) -> bool { /* ... */ }
}
```

The `wall_between(a, b)` helper from the roadmap line 284 is a sibling concern but useful for #13 (door interactions need to know which wall to mutate). Adding it now is in scope:

```rust
impl DungeonFloor {
    /// Returns the WallType between two adjacent cells. Adjacent means
    /// `(b.0, b.1) == (a.0, a.1) + dir.offset()` for some `dir`. Returns
    /// `WallType::Solid` for non-adjacent or out-of-bounds pairs.
    pub fn wall_between(&self, a: (u32, u32), b: (u32, u32)) -> WallType { /* ... */ }
}
```

### Test surface

For `can_move` alone (out-of-scope: discovery, keys, animation), the test matrix is:

- `out_of_bounds_returns_false` (x >= width, y >= height, both)
- `walking_into_solid_blocks` for each direction
- `walking_through_open_succeeds` for each direction
- `door_unlocked_is_passable` (return value; Feature #7 will animate)
- `locked_door_blocks` (must return false — even when key is held; key check lives in #7)
- `secret_wall_blocks` (regardless of "discovery" since this function ignores it)
- `illusory_wall_passes`
- `one_way_passable_from_open_side`
- `one_way_blocks_from_solid_side` (the neighbor cell's wall is `Solid`, so reverse direction returns false)

This is ~10 unit tests, ~80 LOC. The existing roadmap line 286 already lists "out-of-bounds, walking into solid wall, walking through Open, illusory wall, all four direction rotations" — the recommended set above is a strict superset.

### Sources

- `research/20260326-01-bevy-first-person-dungeon-crawler-rpg.md:451-465` — §Pattern 2's reference can_move.
- `project/roadmaps/20260429-01-bevy-dungeon-crawler-roadmap.md:283-286` — required helpers list.

---

## RQ5 — `Direction` semantics: y-axis convention?

### Confidence: HIGH (research §Pattern 2 explicitly chose y-down; conversion to Bevy y-up belongs in renderer)

### The §Pattern 2 reference says

```rust
// research/20260326-01-bevy-first-person-dungeon-crawler-rpg.md:424-431
pub fn offset(self) -> (i32, i32) {
    match self {
        Self::North => (0, -1),
        Self::South => (0, 1),
        Self::East => (1, 0),
        Self::West => (-1, 0),
    }
}
```

— `North = (0, -1)` ⇒ moving north decreases y ⇒ **y-down (screen convention)**.

### Why y-down for grid data

- Top-down auto-map (Feature #10): screens render top-left as origin. North = "up the screen" = y decreases.
- RON file authoring: when you write `walls: [[row 0], [row 1], [row 2], ...]` in RON, row 0 is at the top of the file. Top of file = top of screen = "north" — matches the y-down convention naturally.
- Hand-drawn dungeon maps: every classic Wizardry/Etrian floor map has north-up at the top of the page.

### Why Bevy's y-up doesn't conflict

Bevy's world coordinates use y-up (gravity points in -Y). The first-person renderer (Feature #8) will convert grid coords to world coords with something like:

```rust
fn grid_to_world(x: u32, y: u32, cell_size: f32) -> Vec3 {
    Vec3::new(
        x as f32 * cell_size,
        0.0,
        // Negate y to convert grid-y-down to world-z-forward (north = +Z in world).
        -(y as f32) * cell_size,
    )
}
```

This conversion is one function in Feature #8. It costs nothing at runtime and contains the convention to a single point. The rest of the codebase reads `GridPosition { x, y }` with y-down, and the renderer translates.

### What the lock-in looks like

Once `Direction::North = (0, -1)` ships in this feature, every later feature consumes it:

- **Feature #7 (movement):** `let new = (pos.0 as i32 + dir.offset().0, pos.1 as i32 + dir.offset().1);`
- **Feature #10 (auto-map):** renders `(grid_x * px, grid_y * px)` with north up — y-down matches the canvas.
- **Feature #22 (FOEs):** path-walks in the same grid; consumes `Direction::offset` directly.

Reversing later means: change all four offsets, change the renderer's y conversion, audit every authored floor file (`entry_point: (1, 1, North)` would still mean the same cell, just spawning facing south), revisit every test. This is a 200+ LOC refactor.

**Lock the y-down convention in this feature, document it clearly, never look back.**

### Recommended doc comment

```rust
/// Cardinal direction in grid coordinates.
///
/// **Coordinate convention: y-DOWN (screen convention).**
/// - `North` = (0, -1): moving north decreases y.
/// - `South` = (0,  1): moving south increases y.
/// - `East`  = (1,  0): moving east increases x.
/// - `West`  = (-1, 0): moving west decreases x.
///
/// This matches how `Vec<Vec<WallMask>>` rows are laid out in the RON file
/// (row 0 is at the top, "north"), and how the auto-map (#10) renders
/// (north at top of screen).
///
/// Bevy's world coordinates are y-UP; the 3D renderer (#8) is the single
/// place that converts grid-y-down to world-z (typically negating y).
/// Do NOT add a "world-space Direction" — keep one Direction, one convention.
#[derive(Reflect, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    North,
    South,
    East,
    West,
}
```

### Sources

- `research/20260326-01-bevy-first-person-dungeon-crawler-rpg.md:387-432` — Direction enum + offset().
- `bevy_camera-0.18.1/src/lib.rs` and Bevy's `Transform` docs (well-known) — Bevy world axes are y-up, right-handed.

---

## RQ6 — Spinner metadata for telegraphed UX

### Confidence: MEDIUM (design call; clear winner under §Resolved #4)

### The §Resolved #4 says

> "Modern telegraphed (Etrian style) — Spinner tiles get a visible icon on the auto-map, a sound effect, and a brief screen wobble. Dungeon design culture across all 20 floors leans cerebral, not cruel."

### Options

| Option | What it is | When useful |
|---|---|---|
| **A — `spinner: bool`** (recommended) | One bool per cell. | When all spinners get the same UX, which is exactly the §Resolved #4 design. |
| **B — `spinner: Option<SpinnerConfig>`** | Future-proofed for per-spinner customization. | When some spinners have a different rotation step, sound, or icon — a design we don't have. |
| **C — `spinner: SpinnerStyle` (enum)** | Variants like `Telegraphed`, `Hidden`, `RandomFacing`, etc. | When designing a game with both classic and modern spinners — explicitly rejected by §Resolved #4. |
| **D — `telegraphed: bool` + `spinner: bool`** | Two bools to allow opt-in classic per cell. | Same use-case as C; same rejection. |

### Why A wins

- §Resolved #4 says "all 20 floors" use telegraphed spinners. There is no future plan to mix in classic spinners. Per the lazy-design principle, modeling state for a non-existent feature is YAGNI.
- The auto-map (Feature #10) renders an icon based on the cell having a spinner at all — a `bool` tells it that. Adding `Option<SpinnerConfig>` adds one match-arm in the renderer with no benefit.
- Migrating from `bool` to `Option<SpinnerConfig>` later is a one-time RON migration over ~20 hand-authored floor files (5 minutes of search-replace) and a one-line type change. The cost of waiting is negligible.

### What the planner should NOT do

- Add a `telegraphed: bool` flag inside `CellFeatures`. The flag would always be `true` under §Resolved #4. A field that's always one value is a bug waiting to happen — someone WILL set it false in a hand-edit and never notice.
- Add a `spinner_facing` or `spinner_rotation_step` field "for future flexibility." Telegraphed spinners in Etrian Odyssey rotate the player to a fixed-but-randomly-chosen direction. The randomness lives in #23's RNG; the per-cell field doesn't help.

### Counterargument

> "What if Iron Mode adds classic-style spinners as a hardcore option later?"

That's a global setting (`Res<IronModeSettings>`) — it changes how spinners *behave* (telegraphed UX off), not what's stored in the dungeon file. The dungeon file just says "this cell is a spinner." The behavior layer (#13) decides whether to play the wobble.

### Sources

- `project/roadmaps/20260429-01-bevy-dungeon-crawler-roadmap.md:57` — §Resolved #4 telegraphed-only.

---

## RQ7 — `Reflect` derive on enums and generic types in Bevy 0.18

### Confidence: HIGH (verified directly against extracted bevy_reflect-0.18.1 source)

### The summary

For the type shapes Feature #4 introduces — enum with unit/tuple/struct variants, `Vec<Vec<T>>`, `Option<T>`, primitives, `String`, tuple `(u32, u32, Direction)` — `#[derive(Reflect)]` Just Works. No `#[reflect(...)]` attributes needed. No `TypePath` derive needed (`Reflect` provides it).

### Direct verification

**Mixed-variant enums:** `bevy_reflect-0.18.1/src/enums/mod.rs:11-87` contains a complete test exercising:

```rust
#[derive(Reflect, Debug, PartialEq)]
enum MyEnum {
    A,                       // Unit variant
    B(usize, i32),          // Tuple variant
    C { foo: f32, bar: bool }, // Struct variant
}
```

The test `should_get_enum_type_info` (line 23-87) confirms all three variant types reflect correctly, with `VariantInfo::Unit`, `VariantInfo::Tuple`, and `VariantInfo::Struct` matching the appropriate variants. **Druum's `WallType` (all unit), `TrapType` (mix of unit/struct/tuple-newtype), and `Direction` (all unit) all map to one of these patterns.**

**Vec<T>:** `bevy_reflect-0.18.1/src/impls/alloc/vec.rs:10-20`:

```rust
impl_reflect_for_veclike!(::alloc::vec::Vec<T>, ...);
impl_type_path!(::alloc::vec::Vec<T>);
```

— blanket Reflect impl for `Vec<T>` where `T: Reflect`. **Vec<Vec<WallMask>> reflects because `T = Vec<WallMask>` reflects (recursive blanket impl), and `Vec<WallMask>` reflects because `WallMask` reflects.**

**Option<T>:** `bevy_reflect-0.18.1/src/impls/core/option.rs` — Option impls Reflect when T: Reflect (verified file exists at that path; canonical pattern).

**Tuple `(u32, u32, Direction)`:** `bevy_reflect-0.18.1/src/tuple.rs` provides Tuple subtrait; Reflect derive for tuple-typed fields uses `TupleInfo`. All primitive types Reflect (line 102 of `lib.rs`: "All primitive types implement `FromReflect` by relying on their `Default` implementation").

**Asset trait requirements:** `bevy_asset-0.18.1/src/lib.rs:456`:
```rust
pub trait Asset: VisitAssetDependencies + TypePath + Send + Sync + 'static {}
```
And line 449:
> "[`TypePath`] is largely used for diagnostic purposes, and should almost always be implemented by deriving [`Reflect`] on your type."

— `Reflect` provides `TypePath`; `derive(Asset)` provides `VisitAssetDependencies`. The existing stub already proves this works (it compiles).

### Things that COULD break (and how to verify they don't)

| Concern | Status | Mitigation |
|---|---|---|
| Enum variant reflection at runtime via `Reflect::reflect_ref()` | Works (verified by `enums/mod.rs:23-87` tests) | None needed |
| `FromReflect` derive — auto-generated by `#[derive(Reflect)]` (`lib.rs:267-268`) | Auto-derived | Don't suppress with `#[reflect(from_reflect = false)]` unnecessarily |
| `GetTypeRegistration` for `Vec<Vec<T>>` | Works (line 18 of vec.rs blanket impl) | Don't manually register types unless they need type data |
| Custom `#[reflect(Default)]` for type registry | Optional — saves boilerplate when needed | Add only if a future feature reads back DungeonFloor via reflection (none do today) |

### What about `f32` in `CellFeatures::encounter_rate`?

`f32` is an "opaque" type in Reflect terminology (line 178-181 of `lib.rs`) — it reflects as itself, not breakable into sub-fields. Compatible with all Reflect operations. Confirmed.

### Compile-error hazard: if reflection ever rejects `f32`

It won't — `bevy_reflect-0.18.1/src/impls/core/primitives.rs` provides primitive impls. But if it ever did, the workaround is `#[reflect(opaque)]` on the field. Not needed today.

### Recommended derive list (unchanged from existing stub)

```rust
#[derive(Asset, Reflect, Serialize, Deserialize, Default, Debug, Clone, PartialEq)]
pub struct DungeonFloor { /* ... */ }
```

— same as the current stub PLUS `PartialEq` (added for the App-integration test in RQ2). Each derive is justified:

- `Asset` — required for `Handle<DungeonFloor>`.
- `Reflect` — provides `TypePath` (Asset requires it) + future inspector support + auto-derives `FromReflect` and `GetTypeRegistration`.
- `Serialize` / `Deserialize` — required by `bevy_common_assets::RonAssetPlugin` for parsing.
- `Default` — required by the existing round-trip test; also handy for unit tests that build small grids without setting every field.
- `Debug` — `info!("{:?}", floor)` and friends.
- `Clone` — `bevy_asset_loader::AssetCollection`'s populated-resource pathway needs it.
- `PartialEq` — assertion in App-integration test (RQ2). `Eq` blocked by `f32`. `Hash` not needed.

The full list applies symmetrically to `WallMask` (struct), `WallType` (enum), `Direction` (enum), `CellFeatures` (struct), `TrapType` (enum), `TeleportTarget` (struct). One exception: `Asset` only goes on `DungeonFloor` — the others don't need to be `Handle<...>`-able.

### Sources

- `bevy_asset-0.18.1/src/lib.rs:444-456` — Asset trait definition + Reflect/TypePath guidance.
- `bevy_reflect-0.18.1/src/lib.rs:102-114` — Reflect derive recurses into all fields; FromReflect required for enum derives.
- `bevy_reflect-0.18.1/src/enums/mod.rs:11-87` — verified test for unit/tuple/struct variants under `#[derive(Reflect)]`.
- `bevy_reflect-0.18.1/src/impls/alloc/vec.rs:10-20` — blanket Reflect impl for `Vec<T>`.
- `bevy_reflect-0.18.1/src/impls/core/option.rs` — Option Reflect impl (canonical).
- `bevy_reflect-0.18.1/src/impls/core/primitives.rs` — primitives (incl. f32) Reflect impls.

---

## Architecture Patterns

### Recommended file structure (extends existing layout)

```
src/data/dungeon.rs        # Single file — keeps Feature #4 self-contained
                            # All types: DungeonFloor, WallMask, WallType, Direction,
                            # CellFeatures, TrapType, TeleportTarget. ~250 LOC + tests.

assets/dungeons/
└── floor_01.dungeon.ron    # Replace stub `()` with hand-authored 6x6 test floor

tests/
└── dungeon_floor_loads.rs  # NEW — App-level integration test for RQ2 verification.
                            # Lives at integration-test level (not #[cfg(test)] inside
                            # src/data/dungeon.rs) because integration tests get a fresh
                            # binary, which is closer to "what production does."
                            # Reads: bevy_asset_loader-0.26.0/tests/multiple_asset_collections.rs.
```

### Why a single `src/data/dungeon.rs` file (not a sub-module per type)

The roadmap (line 264) suggested `src/plugins/dungeon/grid.rs` — that's the *plugin* path. Feature #3 already established `src/data/dungeon.rs` as the asset schema home. Feature #4 should stay there and not split into multiple files until LOC > ~600. The current shape — six related types totaling ~150 LOC of struct/enum definitions plus ~100 LOC of impl blocks plus tests — fits comfortably in one file. Splitting prematurely buys nothing and adds cross-file imports.

### Anti-Patterns to Avoid

- **Don't split `Direction` into a separate file just because it has a longer impl block.** Keep all the dungeon grid types in `src/data/dungeon.rs`. They form one cohesive concept.
- **Don't add a `DungeonGrid` newtype around `Vec<Vec<WallMask>>` "for type safety."** It buys nothing serde can't already enforce; it costs every consumer a `.0` access. The roadmap has zero hits for "DungeonGrid" — don't introduce a name not in the design.
- **Don't make `can_move` take `&self` and `Vec3` (world coordinates).** Grid logic should be in grid space. Feature #7 will translate world↔grid; this function lives at grid level only.
- **Don't pull `rand` for `TrapType::Teleport` "to randomize destination."** §Resolved #5 requires deterministic RNG seeded at start, landing in #23 (`RngSeed` resource). The data model has no RNG need; teleport target is fixed in the asset.
- **Don't add a `validate_floor` method on `DungeonFloor` that's MORE than wall consistency.** A "validate everything" method becomes a kitchen-sink. Wall consistency is one thing, in-bounds-ness is another (each `(u32, u32)` should be < `(width, height)` — true for entry_point and teleport targets). Two functions, two test cases.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---|---|---|---|
| Custom serde derive for enums with unit/tuple/struct variants | Manual `Serialize`/`Deserialize` impl | Stock serde derive | Stock serde + ron handles all three variant kinds; verified by ron 0.11/0.12 tests. |
| Custom Reflect impl for `Vec<Vec<WallMask>>` | Manual TypePath/Typed impls | `#[derive(Reflect)]` on each component | Blanket impls + recursive derive cover this. |
| Custom Default for `WallMask` | `impl Default for WallMask` | `#[derive(Default)]` after `#[default] Open` on the enum | `WallType::default() == Open`; struct deriving Default just delegates. |
| RON parsing helper | Wrap `ron::de::from_str` | Use directly in tests; `RonAssetPlugin` handles real loads | Adds nothing. |

---

## Common Pitfalls

### Pitfall 1: Storing walls as a single bitmask byte instead of four `WallType` fields

**What goes wrong:** "Optimize" `WallMask` to `u8` with N/E/S/W as 2-bit fields each. RON serialization becomes opaque (`0x55` vs. `(north: Solid, ...)`); hand-authoring becomes impossible.

**Why it happens:** premature optimization. A 20x20 floor's `Vec<Vec<WallMask>>` is ~6KB serialized — a non-issue.

**How to avoid:** keep the struct of fields. The byte-savings are nonexistent (the WallType enum is already 1 byte each).

### Pitfall 2: Comparing the round-trip test's output of `to_string_pretty` and assuming it matches what `RonAssetPlugin` produces

**What goes wrong:** The pure-stdlib test passes, but loading the file at runtime fails or produces a different value. Cause: one path uses ron 0.11, the other ron 0.12.

**Why it happens:** the version split (RQ2). For empty structs, the format coincidentally matches; for real fields it MIGHT match — but you don't get to verify at the unit-test layer.

**How to avoid:** Add the App-level integration test in `tests/dungeon_floor_loads.rs`. (Detailed in RQ2.)

### Pitfall 3: Using `Vec<Vec<T>>` and indexing `walls[x][y]` instead of `walls[y][x]`

**What goes wrong:** off-by-one bugs and "rotated dungeon" floors.

**Why it happens:** roadmap line 261 acknowledges this: "Forces a (y, x) row-major addressing convention that confuses users used to (x, y)."

**How to avoid:** every place that indexes the grid should go through a method like `wall_at(&self, x: u32, y: u32, side: Direction)` or `cell_at(&self, x: u32, y: u32)`. Doc-comment them and never hand-index from outside the file.

### Pitfall 4: Forgetting `#[serde(default)]` on `CellFeatures`

**What goes wrong:** every cell in the RON file must list every field, or parsing fails.

**Why it happens:** serde's default behavior is "fail on missing field." For `CellFeatures`, where most cells have no features, this would balloon every floor file to 5x its useful size.

**How to avoid:** add `#[serde(default)]` to `CellFeatures` (struct-level). With `Default` derived, missing fields fall back to their type defaults. RON cells can then be written as `()` for plain cells or `(encounter_rate: 0.15)` for cells with one feature.

```rust
#[derive(Serialize, Deserialize, Default, ...)]
#[serde(default)] // <-- THIS LINE matters
pub struct CellFeatures { /* ... */ }
```

### Pitfall 5: Letting `Default` for `DungeonFloor` produce `width: 0, height: 0` and then panicking on `walls[0]` indexing

**What goes wrong:** Default `DungeonFloor` has empty `walls` vec. The round-trip test passes. The first real consumer panics.

**Why it happens:** the round-trip test only checks serde symmetry, not invariants.

**How to avoid:** add a `is_well_formed()` check (or similar) that asserts `walls.len() == height && walls.iter().all(|row| row.len() == width)`. Test it. The Default `DungeonFloor` should not be an "interesting" instance — only used for tests where you build the floor explicitly.

### Pitfall 6: Adding `#[serde(rename = "...")]` for "prettier" RON without thinking about ron 0.11 vs 0.12

**What goes wrong:** `#[serde(rename = "encounter-rate")]` (with a dash) is a non-standard identifier in RON syntax — accepted via raw-identifier (`r#encounter-rate`) but emitted differently than expected.

**Why it happens:** authors think "kebab-case looks nicer in RON."

**How to avoid:** stick with `snake_case` field names. They round-trip cleanly across both ron versions and require no raw-identifier syntax. (Verified by `ron-0.11.0/tests/322_escape_idents.rs` — kebab-case requires `r#` raw-identifier; snake_case never does.)

---

## Security

### Known Vulnerabilities

| Library | Status | Action |
|---|---|---|
| `ron 0.11.0` | No advisories found in extracted CHANGELOG.md or training data. The `307_stack_overflow` test (existing in 0.11) confirms recursion-limit guarding is in place. | Monitor. |
| `ron 0.12.1` | No advisories. | Monitor. |
| `bevy_common_assets 0.16.0` | No advisories; passive transit. | Monitor. |
| `bevy_asset_loader 0.26.0` | No advisories. | Monitor. |
| `serde 1` | Long-standing, widely audited. No advisories that affect us. | Monitor. |

(I cannot run `cargo audit` from this session. Implementer should run after Cargo.lock is regenerated, though no Cargo.lock changes are expected.)

### Architectural Security Risks

| Risk | Affected | Manifestation | Secure pattern | Anti-pattern |
|---|---|---|---|---|
| Parser DoS via deeply nested `Vec<Vec<...>>` | `RonAssetPlugin` parsing untrusted RON | Adversarial RON file with 10,000 nesting levels exhausts stack | Bevy's `AssetPlugin::default()` blocks paths outside `assets/` (`UnapprovedPathMode::Forbid`); ron's stack-recursion limit applies (`tests/307_stack_overflow.rs` proves the guard works) | Loading RON from user paths or net sources without validation |
| Path traversal via `event_id: Option<String>` resolved as filesystem path | Future Feature #13 (event triggers) | A floor file's `event_id: "../../etc/passwd"` reaches a filesystem call | Resolve event_ids ONLY against an allow-list defined at compile time. Don't resolve as paths. | Treating `event_id` as a path or shell command |
| Untrusted save data importing a floor that overwrites runtime grid | Future Feature #23 | Save file contains a serialized `DungeonFloor` that diverges from the loaded asset | Saves should NOT contain DungeonFloor — they reference floors by index/path and store only mutable state | Round-tripping the entire DungeonFloor through saves |

### Trust Boundaries

For Feature #4 specifically (the data model alone), the only trust boundary is:

- **`assets/dungeons/floor_01.dungeon.ron` at startup:** trusted input — shipped with the binary, fixed at build time. Bevy's `UnapprovedPathMode::Forbid` (already enforced in main.rs per Feature #3) blocks any load outside `assets/`. RON parser handles malformed input via `Result`.

Future trust-boundary changes (mod support, dungeon editor) are out of scope here, but `assets/README.md:33-39` already documents the framework. Don't expand the boundary in #4.

---

## Performance

| Metric | Value / Range | Source | Notes |
|---|---|---|---|
| Floor RON file size (20x20) | ~10-30 KB | Estimate from §Pattern 2 example shape | Negligible. |
| Floor parse time (one-shot, ron 0.11) | <5 ms (estimate) | No benchmark in extracted ron-0.11.0; canonical perf is "milliseconds for typical config files" | RON is hand-authored size, not performance-critical. |
| `can_move` runtime | O(1) — single array index + match | Verified by §Pattern 2 reference impl | One call per movement input. |
| Wall-consistency check (20x20) | O(width × height) = ~400 operations | Verified by sketch in RQ1 | Runs once at test time, not at runtime. |
| Memory: `DungeonFloor` size | ~6 KB heap (Vec<Vec<WallMask>> for 20x20 + small fixed fields) | Estimated: 400 cells × ~16 bytes per cell | Per-floor; only 1-2 loaded at a time. |

No performance hot spot. The data model is content-bound, not compute-bound.

---

## Code Examples

Concrete shapes the planner can use to drive file-by-file steps. Not full implementation — just the type definitions with trait derives and key signatures.

### `src/data/dungeon.rs` (final shape — replaces existing stub)

```rust
//! Dungeon floor schema. Razor-wall grid representation.
//!
//! See research/20260326-01-bevy-first-person-dungeon-crawler-rpg.md §Pattern 2
//! for the design rationale and project/research/20260501-220000-feature-4-dungeon-grid-data-model.md
//! for trade-off analysis.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Cardinal direction in grid coordinates. y-DOWN screen convention.
/// (See doc comment in RQ5 for full details — preserve verbatim in the code.)
#[derive(Reflect, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    North,
    South,
    East,
    West,
}

impl Direction {
    pub fn turn_left(self) -> Self { /* per §Pattern 2 */ }
    pub fn turn_right(self) -> Self { /* per §Pattern 2 */ }
    pub fn reverse(self) -> Self { /* per §Pattern 2 */ }
    /// Returns (dx, dy) offset; y-down convention.
    pub fn offset(self) -> (i32, i32) { /* per §Pattern 2 */ }
}

#[derive(Reflect, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WallType {
    #[default]
    Open,
    Solid,
    Door,        // Closed but unlocked — can_move returns true.
    LockedDoor,  // Requires key — can_move returns false until #13 unlocks.
    SecretWall,  // Static asset says solid; #13 layers on discovery.
    OneWay,      // Passable from this side; reverse direction is independently stored.
    Illusory,    // Looks solid; can_move returns true.
}

#[derive(Reflect, Serialize, Deserialize, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct WallMask {
    pub north: WallType,
    pub south: WallType,
    pub east: WallType,
    pub west: WallType,
}

#[derive(Reflect, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum TrapType {
    Pit { damage: u32, target_floor: Option<u32> },
    Poison,
    Alarm,
    Teleport(TeleportTarget),
}

#[derive(Reflect, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct TeleportTarget {
    pub floor: u32,
    pub x: u32,
    pub y: u32,
    pub facing: Option<Direction>,
}

/// Per-cell features. `#[serde(default)]` lets RON omit unused fields.
#[derive(Reflect, Serialize, Deserialize, Default, Debug, Clone, PartialEq)]
#[serde(default)]
pub struct CellFeatures {
    pub trap: Option<TrapType>,
    pub teleporter: Option<TeleportTarget>,
    /// §Resolved #4: telegraphed only — bool is sufficient.
    pub spinner: bool,
    pub dark_zone: bool,
    pub anti_magic_zone: bool,
    pub encounter_rate: f32,
    pub event_id: Option<String>,
}

#[derive(Asset, Reflect, Serialize, Deserialize, Default, Debug, Clone, PartialEq)]
pub struct DungeonFloor {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub floor_number: u32,
    /// Walls stored as [y][x] grid. y-down screen convention.
    pub walls: Vec<Vec<WallMask>>,
    /// Cell features stored as [y][x] grid. Same shape as `walls`.
    pub features: Vec<Vec<CellFeatures>>,
    pub entry_point: (u32, u32, Direction),
    pub encounter_table: String,
}

impl DungeonFloor {
    /// (See RQ4 in research doc for full doc comment — preserve verbatim.)
    pub fn can_move(&self, x: u32, y: u32, dir: Direction) -> bool {
        if x >= self.width || y >= self.height { return false; }
        let cell = &self.walls[y as usize][x as usize];
        let wall = match dir {
            Direction::North => cell.north,
            Direction::South => cell.south,
            Direction::East  => cell.east,
            Direction::West  => cell.west,
        };
        matches!(wall, WallType::Open | WallType::Door | WallType::Illusory | WallType::OneWay)
    }

    /// Returns the WallType between two adjacent cells. Returns Solid for non-adjacent
    /// or out-of-bounds pairs.
    pub fn wall_between(&self, a: (u32, u32), b: (u32, u32)) -> WallType { /* see RQ4 */ }

    /// Asserts wall double-storage symmetry. Used in tests.
    pub fn validate_wall_consistency(&self) -> Result<(), Vec<WallInconsistency>> { /* see RQ1 */ }

    /// Asserts width/height match the actual Vec dimensions.
    pub fn is_well_formed(&self) -> bool {
        self.walls.len() == self.height as usize
            && self.walls.iter().all(|row| row.len() == self.width as usize)
            && self.features.len() == self.height as usize
            && self.features.iter().all(|row| row.len() == self.width as usize)
    }
}

#[derive(Debug, PartialEq)]
pub struct WallInconsistency {
    pub cell_a: (u32, u32),
    pub cell_b: (u32, u32),
    pub direction: Direction,
    pub wall_a: WallType,
    pub wall_b: WallType,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dungeon_floor_round_trips_through_ron() { /* keep existing test */ }

    #[test] fn direction_turn_right_cycles() { /* North -> East -> South -> West -> North */ }
    #[test] fn direction_offset_is_y_down() { /* North.offset() == (0, -1) */ }

    #[test] fn can_move_returns_false_out_of_bounds() { /* x >= width, y >= height */ }
    #[test] fn can_move_blocks_solid() { /* Solid in any direction returns false */ }
    #[test] fn can_move_passes_open() { /* Open in any direction returns true */ }
    #[test] fn can_move_passes_door() { /* Closed unlocked door is passable */ }
    #[test] fn can_move_blocks_locked_door() { /* LockedDoor blocks regardless */ }
    #[test] fn can_move_blocks_secret_wall() { /* Static asset: SecretWall blocks */ }
    #[test] fn can_move_passes_illusory() { /* Illusory passes */ }
    #[test] fn can_move_one_way_asymmetric() { /* OneWay passable; reverse is the neighbor's call */ }

    #[test] fn validate_wall_consistency_detects_mismatch() { /* synthetic floor with mismatched wall */ }
    #[test] fn validate_wall_consistency_allows_one_way_asymmetry() { /* OneWay/Solid pair OK */ }

    #[test] fn floor_01_loads_and_is_consistent() {
        // Read assets/dungeons/floor_01.dungeon.ron via stdlib std::fs (NOT App).
        // Deserialize via ron::de::from_str (ron 0.12 path).
        // Assert is_well_formed AND validate_wall_consistency.
    }
}
```

### `tests/dungeon_floor_loads.rs` (NEW — App-level integration test)

See full body in RQ2 above. Total ~50-70 LOC. Purpose: verify the `RonAssetPlugin` (ron 0.11) path agrees with the stdlib (ron 0.12) path on the same file.

### `assets/dungeons/floor_01.dungeon.ron` (replace stub)

A 6x6 hand-authored test floor. The exact layout is the implementer's call; the planner should require:

- Width 6, height 6.
- Outer perimeter all `Solid` (so the player cannot walk off the edge).
- One `Door` somewhere (so `can_move_passes_door` has real data to point at).
- One `LockedDoor` (for `can_move_blocks_locked_door`).
- One cell with `CellFeatures::trap = Some(Pit { ... })` (exercises Option<TrapType>).
- One cell with `spinner: true` (telegraphed marker).
- `entry_point: (1, 1, North)`.
- Wall consistency must be intact.

This file's existence is verified by Feature #3's loader; Feature #4's job is to make it non-empty.

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|---|---|---|---|
| ron 0.10 base64 byte-strings | ron 0.11+ Rusty byte strings | ron 0.9.0 (2025-03-18) | We have no Vec<u8> bytes today, so no impact. If we add binary blobs later, use Rusty syntax. |
| Hand-rolled `impl AssetLoader for DungeonFloor` | `bevy_common_assets::ron::RonAssetPlugin::<DungeonFloor>` | ~2022 | Done at Feature #3. Don't regress. |
| `EventReader<AssetEvent<T>>` | `MessageReader<AssetEvent<T>>` | Bevy 0.18 | Affects Feature #13 if it watches AssetEvent. Feature #4 doesn't read AssetEvent. |
| Bundle types (`TextBundle`, `Camera3dBundle`) | Component + `#[require(...)]` | Bevy 0.15+ | N/A for Feature #4 (no UI). |

---

## Validation Architecture

### Test Framework

| Property | Value |
|---|---|
| Framework | Cargo's built-in `#[test]` (same as Feature #3) |
| Config file | None — Cargo defaults |
| Quick run command | `cargo test -p druum data::dungeon::tests` |
| Full default suite | `cargo test` |
| Full dev-features suite | `cargo test --features dev` |
| Integration test command | `cargo test --test dungeon_floor_loads` |

### Requirements → Test Map

| Requirement | Behavior | Test Type | Automated Command | File Exists? |
|---|---|---|---|---|
| 4.1 — round-trip `DungeonFloor` through stdlib ron | unit | unit | `cargo test data::dungeon::tests::dungeon_floor_round_trips_through_ron` | ✅ (already in stub — keep, may need to handle non-Default `Vec<Vec<...>>` via `Default::default()` of empty grids) |
| 4.2 — direction rotation/offset | unit | unit | `cargo test data::dungeon::tests::direction_` | ❌ needs creating |
| 4.3 — `can_move` matrix (out-of-bounds + 6 wall types) | unit | unit | `cargo test data::dungeon::tests::can_move_` | ❌ needs creating (~9 tests) |
| 4.4 — wall consistency validator | unit | unit | `cargo test data::dungeon::tests::validate_wall_consistency` | ❌ needs creating |
| 4.5 — `floor_01.dungeon.ron` parses + is consistent (stdlib ron 0.12 path) | unit | unit | `cargo test data::dungeon::tests::floor_01_loads_and_is_consistent` | ❌ needs creating |
| 4.6 — `floor_01.dungeon.ron` parses through `RonAssetPlugin` (ron 0.11 path) | integration | integration | `cargo test --test dungeon_floor_loads floor_01_loads_through_ron_asset_plugin` | ❌ needs creating in `tests/dungeon_floor_loads.rs` |

### Gaps (files to create before implementation)

- [ ] Update `src/data/dungeon.rs` to the final shape (RQ7 code example).
- [ ] Hand-author `assets/dungeons/floor_01.dungeon.ron` (replace stub `()`).
- [ ] Create `tests/dungeon_floor_loads.rs` (App-level integration test).
- [ ] No new `Cargo.toml` changes — confirm by running `cargo tree` after work and diffing.

The 6 verification commands from the user task constraint must each PASS with zero warnings:

| Command | Notes |
|---|---|
| `cargo check` | Zero warnings expected. |
| `cargo check --features dev` | Same. |
| `cargo clippy --all-targets -- -D warnings` | Verify clippy doesn't flag `Vec<Vec<...>>` or large match arms. |
| `cargo clippy --all-targets --features dev -- -D warnings` | Same. |
| `cargo test` | All unit tests + integration test. |
| `cargo test --features dev` | Same plus dev-only systems (none expected for Feature #4). |

---

## Open Questions

### (A) Blockers requiring user decisions

**A1. Should the round-trip test in `src/data/dungeon.rs` test `Default::default()` (empty grids) OR a hand-built non-trivial grid?**

- **What we know:** the existing test uses `DungeonFloor::default()`. With non-trivial fields added, this means `walls: vec![]`, `width: 0`, etc. — the test still passes (empty Vec round-trips), but it's a vacuous test for the real shape.
- **What's unclear:** whether the planner should keep the Default-based test (trivially safe) and add a new test with a hand-built 2x2 floor, OR replace the Default test with the hand-built one.
- **Why this could need user input:** "test the trivial case AND the real case" is two tests; "replace" is one. Style call.
- **Recommendation:** Keep both — they're cheap and orthogonal. The Default case catches symmetric serde derives across empty collections; the hand-built case catches symmetric derives for real data.

### (B) Defaults the planner can pick without escalation

**B1. Layout of the 6x6 `floor_01.dungeon.ron`:** the planner should pick a layout that exercises every WallType and at least one CellFeatures variant. The layout itself is gameplay design, not technical research; the planner can decide or delegate to the implementer. The constraint is "all WallTypes and one trap, one spinner, one teleporter must appear."

**B2. Whether to add a `validate_floor` umbrella method that calls both `is_well_formed` and `validate_wall_consistency`:** both flavors are fine. `validate_floor()` is a tiny convenience; the implementer can add it without escalation.

**B3. Whether to derive `Eq` on `DungeonFloor`:** can't (because of `f32 encounter_rate`). Don't worry about it.

**B4. Test placement (inside `src/data/dungeon.rs` `#[cfg(test)] mod tests` vs separate `tests/` directory):** unit tests stay in-file; the App-level test goes in `tests/dungeon_floor_loads.rs`. This is the established Rust convention; no escalation needed.

**B5. RON file naming `floor_01.dungeon.ron` vs `floor_01.dungeon.ron.gz` (compression):** stay with plain RON. Compression is a build-time concern, not Feature #4.

### (C) Trade-off questions for the user only if user input is needed

**C1. Should `WallType::Door` carry a `key_required: Option<KeyId>` field instead of being split into `Door` and `LockedDoor`?**

- **Tradeoff:** unifying simplifies WallType (one variant for "doors"), but adds a `KeyId` type to the asset (Feature #12 territory) and requires every door — locked or not — to carry that field. Splitting into `Door`/`LockedDoor` keeps the asset slim and defers `KeyId` to #12.
- **Recommendation:** stay with §Pattern 2's split. Feature #4 is not the place to introduce KeyId. If #12 finds the split painful, refactor then.

**C2. Should `CellFeatures::event_id` be `Option<String>` or `Option<EventId>` (a strongly-typed newtype)?**

- **Tradeoff:** newtype is more rigorous, but introduces a type in Feature #4 that no current consumer reads (Feature #13 is months away). String round-trips through serde for free.
- **Recommendation:** `Option<String>` for now. When #13 lands, refactor to `Option<EventId>` with a `From<String>` impl — one-line migration.

---

## Sources

### Primary (HIGH confidence — direct file reads)

- [`bevy_asset-0.18.1/src/lib.rs`](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_asset-0.18.1/src/lib.rs) lines 444-456 — `Asset` trait definition + Reflect/TypePath guidance.
- [`bevy_reflect-0.18.1/src/lib.rs`](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_reflect-0.18.1/src/lib.rs) lines 102-114, 178-181, 267-268 — Reflect derive recursion rules + opaque types + FromReflect auto-derive.
- [`bevy_reflect-0.18.1/src/enums/mod.rs`](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_reflect-0.18.1/src/enums/mod.rs) lines 11-87 — verified test for unit/tuple/struct enum variants.
- [`bevy_reflect-0.18.1/src/impls/alloc/vec.rs`](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_reflect-0.18.1/src/impls/alloc/vec.rs) lines 10-20 — Vec<T> blanket Reflect impl.
- [`bevy_common_assets-0.16.0/Cargo.toml`](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_common_assets-0.16.0/Cargo.toml) lines 169-172 — `ron = "0.11"` aliased as `serde_ron`.
- [`bevy_common_assets-0.16.0/src/ron.rs`](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_common_assets-0.16.0/src/ron.rs) lines 6, 84 — uses `serde_ron::de::from_bytes` for parsing.
- [`bevy_asset_loader-0.26.0/Cargo.toml.orig`](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_asset_loader-0.26.0/Cargo.toml.orig) lines 53, 61-63 — confirms ron 0.11 (via bevy_common_assets) and ron 0.12 coexist in the ecosystem.
- [`bevy_asset_loader-0.26.0/tests/multiple_asset_collections.rs`](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_asset_loader-0.26.0/tests/multiple_asset_collections.rs) lines 8-46 — canonical App-driven loading test pattern.
- [`bevy_asset_loader-0.26.0/tests/finally_init_resource.rs`](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_asset_loader-0.26.0/tests/finally_init_resource.rs) — alternate pattern for asserting in `OnEnter(Loaded)`.
- [`ron-0.11.0/CHANGELOG.md`](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/ron-0.11.0/CHANGELOG.md) — full version history for 0.11.
- [`ron-0.12.1/CHANGELOG.md`](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/ron-0.12.1/CHANGELOG.md) — confirms only API + base64-byte-string format changes between 0.11 and 0.12.
- [`ron-0.11.0/tests/123_enum_representation.rs`](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/ron-0.11.0/tests/123_enum_representation.rs) — verifies external/internal/adjacent/untagged enum representations in ron 0.11.
- [`ron-0.12.1/tests/123_enum_representation.rs`](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/ron-0.12.1/tests/123_enum_representation.rs) — byte-identical to ron 0.11 for first 100 lines.
- [`ron-0.11.0/tests/options.rs`](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/ron-0.11.0/tests/options.rs) and `ron-0.12.1/tests/options.rs` — Option<T> emission verified identical.
- [`ron-0.11.0/tests/floats.rs`](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/ron-0.11.0/tests/floats.rs) and `ron-0.12.1/tests/floats.rs` — float emission verified identical.
- [`bevy-0.18.1/examples/asset/custom_asset.rs`](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy-0.18.1/examples/asset/custom_asset.rs) lines 11-18, 35-54 — canonical Bevy 0.18 derive shape for asset structs.
- `Cargo.lock` lines 4337-4362 — both ron versions co-resident.

### Secondary (project context)

- `research/20260326-01-bevy-first-person-dungeon-crawler-rpg.md` lines 330-466 (Pattern 2), 1335 (dcrawl reference) — original dungeon grid research.
- `project/research/20260501-160000-bevy-0-18-1-asset-pipeline-feature-3.md` — Feature #3 research with ron 0.12 round-trip test rationale.
- `project/roadmaps/20260429-01-bevy-dungeon-crawler-roadmap.md` lines 245-289 (Feature #4 spec), 57 (§Resolved #4 telegraphed-spinner), 260-261 (double-storage acknowledgment).
- `project/reviews/20260501-210000-bevy-0-18-1-asset-pipeline-feature-3.md` lines 75-79 — code reviewer note about ron version split blocking Feature #4.
- `src/data/dungeon.rs` (existing stub) — starting point.
- `src/plugins/loading/mod.rs` — Feature #3's LoadingPlugin that consumes `Handle<DungeonFloor>`.
- `assets/README.md` — asset directory contract.

### Tertiary / institutional memory (LOW confidence — needs validation)

- `.claude/agent-memory/code-reviewer/feedback_ron_version_split.md` — prior reviewer feedback flagging RQ2.
- `.claude/agent-memory/researcher/feedback_bevy_0_18_event_message_split.md` — Bevy 0.18 Event/Message split (relevant to MessageWriter<AppExit> in the integration test).
- Classic Wizardry (1981) used 4-bit-per-cell wall storage — gamedev folklore; no citable academic source available in this session.

---

## Metadata

**Confidence breakdown:**

- RQ1 (wall storage): MEDIUM — design call backed by published §Pattern 2 reference and roadmap acknowledgment. No live ecosystem-survey access this session.
- RQ2 (ron version compatibility): HIGH — both ron versions extracted on disk, test files inspected directly; format-equivalence verified by byte-level diff of test files.
- RQ3 (CellFeatures shape): MEDIUM — design call with clear winner under §Resolved #4 + lazy-design principle; user task description partially conflicts but the conflict (Door variant) is resolvable on technical grounds (doors are a WallType).
- RQ4 (can_move semantics): HIGH — each behavior is locally specified; no external dependencies needed.
- RQ5 (y-axis convention): HIGH — §Pattern 2 already chose y-down; Bevy world-axis convention well-documented; conversion lives in renderer.
- RQ6 (spinner metadata): MEDIUM — design call with clear winner under §Resolved #4.
- RQ7 (Reflect derive): HIGH — verified directly against extracted bevy_reflect-0.18.1 source files.

**Research date:** 2026-05-01

**Pre-submission verification:**

- [x] Pitfalls reviewed: Configuration scope (N/A, single feature), Deprecated features (verified ron CHANGELOG), Negative claims (RQ2's "no format change" backed by direct test-file diff), Single source (multiple Bevy/ron sources cross-referenced)
- [x] Top recommendation red-teamed: counterargument to store-twice (RQ1) addressed; counterargument to struct-of-optionals (RQ3) addressed; counterargument to no-SpinnerStyle (RQ6) addressed
- [x] Must-pass checklist: security checked (no CVEs); architectural risks identified; licenses unchanged (no new deps); compatibility verified (both ron versions coexist already in production Cargo.lock); negative claims verified; all 7 RQs investigated
- [x] Should-pass checklist: multiple sources cross-referenced; confidence honest; library health (already-shipped Feature #3); test infrastructure assessed (extends Feature #3's setup with one new integration-test file); URLs provided as `file:///` paths
