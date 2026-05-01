# Bevy 0.18.1 Asset Pipeline & RON Loading (Feature #3) - Research

**Researched:** 2026-05-01
**Domain:** Bevy 0.18.1 asset pipeline, custom RON loading, declarative asset collections, hot-reload, loading screen
**Confidence:** HIGH on Bevy 0.18.1 first-party APIs (verified directly against the on-disk `bevy_asset-0.18.1`, `bevy_ui-0.18.1`, `bevy_asset_macros-0.18.1`, and `bevy-0.18.1` umbrella source). MEDIUM on the third-party crates `bevy_common_assets` and `bevy_asset_loader` — pinned versions and exact API shapes need a one-time crates.io / GitHub verification before locking `Cargo.toml`. See "Tooling Limitation Disclosure" below.

---

## Recommendation Header (for the planner)

**Top-level recommendation:** Implement `LoadingPlugin` at `src/plugins/loading/mod.rs` (alphabetical sibling of `state/`). Add **two pinned external crates** (`bevy_common_assets`, `bevy_asset_loader`), one new Bevy umbrella feature (`bevy/file_watcher`, gated under the existing `dev` Cargo feature), and **no other changes to `Cargo.toml`**'s feature list. Use `bevy_common_assets::ron::RonAssetPlugin::<T>::new(&["dungeon.ron"])` to register each custom-extension RON type, and `bevy_asset_loader::loading_state::LoadingStateAppExt::add_loading_state` with a `LoadingState::new(GameState::Loading).continue_to_state(GameState::TitleScreen).load_collection::<DungeonAssets>()` to drive the `Loading → TitleScreen` transition. Loading-screen UI is one centered `Text` node spawned on `OnEnter(GameState::Loading)` and despawned on `OnExit`. RON round-trip test goes against `ron::de::from_str` / `ron::ser::to_string_pretty` — no `App` or `AssetServer` needed.

**Five things the planner must NOT skip:**

1. **Verify exact crate versions before editing `Cargo.toml`.** I cannot reach crates.io from this session. Run the verification recipe in §Question 1 once, then pin with `=` (per the project skeleton convention). If either crate has not published a Bevy-0.18-compatible release, **escalate** — do not downgrade Bevy. The `moonshine-save` precedent (Decision §Resolved #3, 2026-04-29) is what to do, not what to avoid.
2. **`bevy_asset_loader::AssetCollection` requires the `Resource` derive too.** The collection struct must derive both `AssetCollection` and `Resource` — easy to miss.
3. **Hot-reload requires both pieces.** Add the `bevy/file_watcher` Cargo feature **and** set `AssetPlugin { watch_for_changes_override: Some(true), ..default() }`. Just one or the other is silently insufficient.
4. **`AssetEvent<T>` is a `Message`, not an `Event`** in Bevy 0.18 — same trap as `StateTransitionEvent` in Feature #2 (research §Pitfall 1 there). If any code in this feature reads asset events directly, it must use `MessageReader<AssetEvent<T>>`. We avoid this entirely by going through `bevy_asset_loader`, but a fallback hand-rolled path would hit it.
5. **Stub asset types use `#[derive(Asset, Reflect, Serialize, Deserialize, Default, Debug, Clone)]`** — the order matters only for readability; the `Asset` trait requires `TypePath + VisitAssetDependencies + Send + Sync + 'static`, and `Reflect` derive supplies `TypePath` (verified at `bevy_asset-0.18.1/src/assets.rs:456` and `bevy_reflect_derive-0.18.1/src/lib.rs:822`). `Asset` derive auto-impls `VisitAssetDependencies` (verified at `bevy_asset_macros-0.18.1/src/lib.rs:46-94`). Bevy's own `LoadedUntypedAsset` uses `#[derive(Asset, TypePath)]` instead — both shapes work.

---

## Tooling Limitation Disclosure (read this first)

This research session ran with only `Read`, `Write`, `Grep`, `Glob`, `Edit`. **No Bash, no MCP servers (despite the `context7` system reminder), no WebFetch, no WebSearch.** This matches the prior Druum research sessions for Features #1 and #2.

**How that was mitigated:** the local Cargo registry at `/Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/` already has the **complete `bevy-0.18.1` crate family** extracted on disk:

- `bevy_asset-0.18.1/` — `AssetPlugin`, `AssetServer`, `AssetEvent`, `Asset` trait, `LoadState`, `is_loaded_with_dependencies`, `AssetPath::get_full_extension`
- `bevy_ui-0.18.1/` — `Text`, `Node`, `JustifyContent`, `AlignItems`, the `#[require(...)]` component-driven 0.18 spawning model
- `bevy_text-0.18.1/`, `bevy_internal-0.18.1/`, `bevy_state-0.18.1/`, `bevy_ecs-0.18.1/`, `bevy_reflect-0.18.1/`, `bevy_asset_macros-0.18.1/`, etc.
- The umbrella `bevy-0.18.1/Cargo.toml` with the full feature-flag map.
- Bevy's own example files at `bevy-0.18.1/examples/asset/{custom_asset.rs, hot_asset_reloading.rs}`.

**Every Bevy 0.18.1 API claim in this document is verified by reading the actual 0.18.1 source files on disk** with file paths and line numbers cited inline. This is HIGH confidence on first-party APIs.

**What I could NOT verify locally** (and where MEDIUM confidence applies):

- The **published version numbers** of `bevy_common_assets` and `bevy_asset_loader` on crates.io — neither was extracted under `~/.cargo/registry`. Inferring from training-data patterns, they typically lag a Bevy point release by 0-2 weeks. Bevy 0.18.0 shipped 2026-03-04 (per Druum research §Standard Stack); both crates have a strong release cadence under maintainer NiklasEi, so 0.18-compatible versions should exist by 2026-05-01. I cannot prove this without crates.io.
- The **exact API shape** for those two crates on the latest 0.18-compatible release. I describe the most likely API based on training data; the implementer should sanity-check by running `cargo add bevy_common_assets bevy_asset_loader` and reading the auto-resolved version, then check that crate's README/docs.rs page once they have a clean repo.

Where this matters concretely: every cell of the "Standard Stack — Core" table that lists a third-party version is followed by a verification recipe, and §Question 1 below has a step-by-step recipe the planner/implementer should run **once** before locking `Cargo.toml`.

---

## Summary

Feature #3 is a +150–250 LOC, +2-deps feature that wires up the data-driven asset loading the rest of the game depends on. Three Bevy 0.18.1 first-party APIs do most of the work — `AssetPlugin`, the `Asset` trait derive, and `AssetServer::is_loaded_with_dependencies` — and two well-known third-party crates by the same maintainer (NiklasEi) reduce boilerplate to near-zero: `bevy_common_assets::ron::RonAssetPlugin` registers a custom-extension serde-RON loader in one line, and `bevy_asset_loader::loading_state::LoadingState` plus `#[derive(AssetCollection)]` give a single `Res<DungeonAssets>` resource at `OnEnter(GameState::TitleScreen)` time.

The single 0.18-specific landmines for this feature are:

1. **`AssetEvent<T>` is a `Message`, not an `Event`.** If any code reads load events manually, it must use `MessageReader<AssetEvent<T>>` (verified at `bevy_asset-0.18.1/src/event.rs:9, 49`). This is the same family-rename trap that bit Feature #2 with `StateTransitionEvent`. Routing through `bevy_asset_loader` avoids it; a hand-rolled fallback would hit it.
2. **Hot-reload needs both the cargo feature `bevy/file_watcher` AND `AssetPlugin { watch_for_changes_override: Some(true), .. }`.** The plain feature flag without the override defaults to off in `AssetPlugin::default()` (verified at `bevy_asset-0.18.1/src/lib.rs:333-345`).
3. **`AssetServer::watch_for_changes()` does NOT exist as a setter in 0.18.1.** Only `AssetServer::watching_for_changes()` (a read-only getter, line 190) and `AssetPlugin::watch_for_changes_override` (the constructor parameter, line 248). The brief's wording "AssetServer::watch_for_changes_override or equivalent" was slightly off — the correct surface is the plugin field, not a server method.
4. **Bevy 0.18 uses `AssetPath::get_full_extension`**, which returns the substring after the **first** `.` (so `floor_01.dungeon.ron` → `"dungeon.ron"`). When registering an extension with `bevy_common_assets::RonAssetPlugin::new(&["dungeon.ron"])`, supply the full multi-dot extension **without** a leading dot. The 0.18 path matcher tries the full extension first, then falls back through `iter_secondary_extensions` (so it would also match if you registered just `"ron"`, but our requirement is per-type extensions, so we register the full thing — verified at `bevy_asset-0.18.1/src/server/loaders.rs:280-289` and `path.rs:464-490`).

**Primary recommendation:** Implement `LoadingPlugin` at `src/plugins/loading/mod.rs`. Stub the five RON asset types (`DungeonFloor`, `ItemDb`, `EnemyDb`, `ClassTable`, `SpellTable`) under `src/data/` (currently empty per `src/data/mod.rs`'s comment) — each derives `Asset, Reflect, Serialize, Deserialize, Default, Debug, Clone`. Register five `RonAssetPlugin` instances in `LoadingPlugin::build`, one per extension. Add a `DungeonAssets` collection covering placeholder fonts/textures via `#[derive(AssetCollection, Resource)]`. Wire `LoadingState::new(GameState::Loading).continue_to_state(GameState::TitleScreen).load_collection::<DungeonAssets>()` through `add_loading_state`. Spawn a centered "Loading..." `Text` UI node on `OnEnter(GameState::Loading)`, despawn on `OnExit`. Add the round-trip serde test inside `#[cfg(test)] mod tests` in the data module — pure stdlib + `ron`, no Bevy app needed. Add `bevy/file_watcher` to the existing `dev` cargo feature so hot-reload only ships under `cargo run --features dev`. Done.

---

## Standard Stack

### Core

| Library | Version | Purpose | License | Maintained? | Why Standard |
| ------- | ------- | ------- | ------- | ----------- | ------------ |
| `bevy_asset` (re-exported via `bevy::asset` and `bevy::prelude`) | 0.18.1 | `Asset` trait, `AssetPlugin`, `AssetServer`, `AssetEvent`, `LoadState`. Already enabled by our `features = ["3d"]` (see §Question 8). | MIT/Apache-2.0 | Yes — co-released with Bevy core | Built-in. The only canonical asset pipeline for Bevy. No third-party alternative. |
| `bevy_common_assets` | **MEDIUM-confidence pin: `=0.14.x` for Bevy 0.18 — verify** (see §Question 1) | Drops in serde-based loaders (`RonAssetPlugin`, `JsonAssetPlugin`, `TomlAssetPlugin`, `MsgPackAssetPlugin`, `YamlAssetPlugin`) for custom-extension typed assets. | MIT/Apache-2.0 (typical for NiklasEi crates; verify when pinning) | Active — release cadence tracks Bevy minors closely | Saves ~30 LOC of `impl AssetLoader for ...` boilerplate per asset type. Listed in research §Don't Hand-Roll. |
| `bevy_asset_loader` | **MEDIUM-confidence pin: `=0.25.x` for Bevy 0.18 — verify** (see §Question 1) | Declarative `#[derive(AssetCollection)]` + `LoadingState` that gates a target `States::Variant` until all assets in the collection report `LoadedWithDependencies`. | MIT/Apache-2.0 (typical) | Active — same maintainer | Replaces hand-rolled "wait for N handles to load, then `next.set(...)`" with one chained builder call. Listed in research §Don't Hand-Roll. |
| `serde` | `1` (transitive — already in `Cargo.lock` line 4326-4329 via Bevy 0.18.1's own dep) | `Serialize` / `Deserialize` derives for asset structs. | MIT/Apache-2.0 | Yes | Stdlib of the Rust serde ecosystem. |
| `ron` | `0.12.1` (transitive — already in `Cargo.lock` line 4192-4195 via `bevy_asset-0.18.1/Cargo.toml:166-168`) | RON parser/serializer for round-trip tests and direct use. | MIT/Apache-2.0 | Yes | Bevy's chosen scene format. Already pulled in. |

**Compatibility check:**
- Bevy 0.18.1's transitive `ron = "0.12"` (verified at `bevy_asset-0.18.1/Cargo.toml:166-168`) is the version we will use. `bevy_common_assets`'s RON loader uses the same crate; if it pulls a different `ron` major, Cargo will resolve to two simultaneous copies — wasteful but not broken. Worth confirming after first compile. Likely fine: `bevy_common_assets` typically tracks the Bevy `ron` version closely.
- Both `bevy_common_assets` and `bevy_asset_loader` re-export their own `Resource`/`Plugin` types around Bevy types. They cannot both use a different `bevy` minor than us — if they target Bevy `^0.17`, the Cargo resolver will refuse the build, and we get the escalation signal the brief describes (Decision §Resolved #3 precedent: `bevy_save 2.0.1` required `bevy ^0.16.1` two minors back, so we picked `moonshine-save 0.6.1` instead — same playbook here).

### Supporting

| Library | Version | Purpose | When to Use |
| ------- | ------- | ------- | ----------- |
| `bevy_ui` (transitive via `features = ["3d"]`, see §Question 6 / §Question 8) | 0.18.1 | `Text`, `Node`, `JustifyContent`, `AlignItems` for the loading-screen fallback. | Spawn one centered `Text` node on `OnEnter(GameState::Loading)`. Already pulled in by `"3d"` (verified at `bevy-0.18.1/Cargo.toml:2322-2330` → `"ui"` umbrella → `"ui_api"` → `"bevy_ui"` at line 2582). |
| `bevy_text` (already in our explicit `features`) | 0.18.1 | Provides `Text` font infrastructure used by `bevy_ui`'s `Text` widget. | Already explicitly listed in `Cargo.toml:15`. |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
| ---------- | --------- | -------- |
| `bevy_common_assets::ron::RonAssetPlugin::<T>::new(&[ext])` | Hand-rolled `impl AssetLoader for T { ... }` | ~30 LOC per type, repeated 5x = ~150 LOC of boilerplate. The `examples/asset/custom_asset.rs` reference at `bevy-0.18.1/examples/asset/custom_asset.rs:35-54` is the canonical hand-rolled shape. Use this only if `bevy_common_assets` has not shipped 0.18 support yet (escalation case). |
| `bevy_asset_loader::LoadingState` | Hand-rolled "track handles + advance state" system reading `AssetServer::is_loaded_with_dependencies(handle)` (verified at `bevy_asset-0.18.1/src/server/mod.rs:1252`) | Doable in ~50 LOC: hold `Handle<T>`s in a `Resource`, poll each in an `Update` system, call `next.set(GameState::TitleScreen)` when all are ready. Acceptable as a fallback if `bevy_asset_loader` lags 0.18, but loses the declarative collection ergonomics. |
| `bevy_asset_ron` (deprecated — see research §State of the Art line 1232) | n/a — use `bevy_common_assets` | Old crate superseded by `bevy_common_assets`. Do not pick it. |

**Installation (post-version-verification):**

```toml
# Cargo.toml additions (versions are placeholders pending §Question 1 verification)
[dependencies]
bevy_common_assets = { version = "=0.14.0", features = ["ron"] }
bevy_asset_loader  = "=0.25.0"
serde              = { version = "1", features = ["derive"] }   # transitive but explicit for clarity
ron                = "0.12"                                       # transitive; explicit for round-trip test ergonomics

[dependencies.bevy]
# Add file_watcher to the existing feature list, gated by our `dev` feature.
# (Existing list stays the same; just add the line below to the [features] block.)

[features]
dev = ["bevy/dynamic_linking", "bevy/file_watcher"]
```

The "add `bevy/file_watcher` to `dev`" is the recommended shape because we only want hot-reload during `cargo run --features dev` — release builds should not watch the filesystem. This matches the existing `dev`-feature philosophy laid down in Feature #1 (`project/resources/20260501-102842-dev-feature-pattern.md` §Effect 2).

---

## Architecture Options

The roadmap's Feature #3 scope is fixed; the decisions to make are about **organisation** of the new code, not the technical approach. Three options surface.

### Option A: `LoadingPlugin` at `src/plugins/loading/mod.rs` with stub asset types in `src/data/` (RECOMMENDED)

| Description | Pros | Cons | Best When |
| ----------- | ---- | ---- | --------- |
| Asset types live in `src/data/` (which today contains only a placeholder `mod.rs` per `src/data/mod.rs:3-4`). The plugin lives in `src/plugins/loading/mod.rs`. `LoadingPlugin` imports the asset types from `crate::data::`. | Matches the existing two-tree split: `src/plugins/` for things that own systems/resources, `src/data/` for static schemas. The `data/mod.rs` placeholder explicitly says "populated by Feature #3" — the slot is reserved. Consistent with Feature #2's "state lives where it does work" pattern but extends with a "schemas live where they don't do work." | Two new module trees touched; minor cognitive cost when the planner first reads the layout. | The codebase has already separated plugins from static data — which druum has. |

### Option B: All Feature-#3 code inside `src/plugins/loading/mod.rs`

| Description | Pros | Cons | Best When |
| ----------- | ---- | ---- | --------- |
| Stub asset types are defined at the top of `loading/mod.rs` next to the plugin. | Single file is easier to read for a small feature. Every consumer can write `use crate::plugins::loading::DungeonFloor`. | Violates the `src/data/mod.rs` placeholder's stated purpose. Asset schemas grow over time (Feature #4 fills `DungeonFloor`'s wall grid; Feature #11 fills `EnemyDb`); placing them inside `LoadingPlugin`'s file means every later feature's data PR touches the loading plugin file. Coupling will hurt by Feature #11. | The schema definitions are guaranteed to stay tiny. They will not. |

### Option C: Asset types in a new `src/assets.rs` (per the original roadmap text, line 218)

| Description | Pros | Cons | Best When |
| ----------- | ---- | ---- | --------- |
| The roadmap entry at line 218 says "New `src/assets.rs` defining typed RON loader registrations." | Matches the literal roadmap text. | Conflicts with the established convention from Feature #1: "everything is a plugin under `src/plugins/`, static data under `src/data/`" (skeleton memory `project_druum_skeleton.md`). Adding a third top-level module type at `src/assets.rs` introduces a special case for no benefit. The roadmap was written before Feature #1 froze the convention. | Choose this only if the planner prefers literal-roadmap fidelity over the established skeleton. |

**Recommended:** Option A — `src/plugins/loading/mod.rs` for the plugin, `src/data/dungeon.rs` / `src/data/items.rs` / `src/data/enemies.rs` / `src/data/classes.rs` / `src/data/spells.rs` for the stub schemas (each one struct, ~5-10 lines for now). Re-export from `src/data/mod.rs` so `crate::data::DungeonFloor` works.

### Counterarguments

Why someone might NOT choose Option A:

- **"Five new files for empty structs is overkill."** — *Response:* The schemas are not empty for long. Feature #4 fills `DungeonFloor` with the razor-wall grid (research §Pattern 2 in 2026-03-26 doc, ~300 LOC including tests). One file per schema is the right granularity for that growth. If the planner really wants one file, `src/data/mod.rs` can hold all five stubs at once — but split before Feature #4 lands.
- **"What if `bevy_asset_loader` doesn't ship 0.18?"** — *Response:* The escalation path is identical to Decision §Resolved #3 (the `moonshine-save` precedent). Halt the feature, write a one-paragraph escalation note, propose the hand-rolled fallback (the "Alternatives Considered" row above) as the alternative, and let the user decide whether to wait or proceed without `bevy_asset_loader`. Do **not** silently fall back; this is a planning-level decision.
- **"The `LoadingState` integration with our `GameState::Loading` enum requires confirmation."** — *Response:* Yes — see §Question 7. The 0.18-compatible API for this is one of the riskier MEDIUM-confidence claims; the verification step is one `cargo doc --open` after `cargo add bevy_asset_loader`.

---

## Critical Questions Answered

### Question 1 — Pinned versions of `bevy_common_assets` and `bevy_asset_loader`

**Confidence: MEDIUM (HIGH that 0.18-compatible releases exist or will soon; MEDIUM on exact versions without crates.io access).**

**What I know:**
- Both crates are maintained by GitHub user `NiklasEi`. Repos: [bevy_common_assets](https://github.com/NiklasEi/bevy_common_assets) and [bevy_asset_loader](https://github.com/NiklasEi/bevy_asset_loader). Both are MIT/Apache-2.0 dual-licensed (typical for NiklasEi).
- Both crates have a strong release cadence — they typically publish a new minor 0-3 weeks after each Bevy minor. Bevy 0.18 shipped 2026-03-04 (per Druum research §Standard Stack). By 2026-05-01, a 0.18-compatible release should exist.
- Historical version mapping (training data, MEDIUM confidence): `bevy_common_assets ~0.13` ↔ Bevy 0.17, `~0.14` ↔ Bevy 0.18. `bevy_asset_loader ~0.24` ↔ Bevy 0.17, `~0.25` ↔ Bevy 0.18. Don't quote me on the exact patch version.
- The brief's lesson learned from Feature #1's `bevy_save` precedent applies here: **if either crate is still on Bevy 0.16/0.17, do not downgrade Bevy — escalate.**

**Verification recipe (run once before locking `Cargo.toml`):**

```bash
# Option 1 (fastest if you already have a Cargo.toml that pulls the crates):
cargo add bevy_common_assets --features ron --dry-run
cargo add bevy_asset_loader --dry-run
# Then read the resolved version printed; check its `bevy = ...` dependency.

# Option 2 (no project state needed):
curl -s https://raw.githubusercontent.com/NiklasEi/bevy_common_assets/main/Cargo.toml | grep -A 3 'bevy ='
curl -s https://raw.githubusercontent.com/NiklasEi/bevy_asset_loader/main/bevy_asset_loader/Cargo.toml | grep -A 3 'bevy ='

# Option 3 (most authoritative — read crates.io API):
curl -s https://crates.io/api/v1/crates/bevy_common_assets | jq -r '.versions[].num' | head -10
curl -s https://crates.io/api/v1/crates/bevy_asset_loader  | jq -r '.versions[].num' | head -10
```

**Decision rule for the planner:**
1. If the latest `bevy_common_assets` and `bevy_asset_loader` both require `bevy = "0.18..."`, pin both with `=` operators (per project convention from Feature #1 — `project_druum_skeleton.md`). Lock to the exact version the verification recipe surfaces.
2. If either lags one Bevy minor (i.e., requires `bevy = "0.17..."`), **stop the feature, escalate to the user with a one-paragraph note**, and propose the hand-rolled fallback (the "Alternatives Considered" row in §Standard Stack) for whichever crate lags. Do not silently downgrade Bevy.
3. If both lag, also escalate. The user may decide to delay Feature #3 until upstream catches up, or to hand-roll a minimal pipeline now.

---

### Question 2 — Custom RON extension registration syntax (Bevy 0.18-compatible)

**Confidence: MEDIUM-HIGH** (the canonical `bevy_common_assets` API has been stable since 0.6+; the form below matches NiklasEi's documented usage pattern, but the **exact spelling** of `RonAssetPlugin::new(&[...])` should be confirmed against the resolved version's docs.rs page).

**The shape:**

```rust
// In LoadingPlugin::build, registering one extension per asset type.
//
// Source pattern: https://github.com/NiklasEi/bevy_common_assets — see README.
// Bevy 0.18 first-party asset path matching: bevy_asset-0.18.1/src/server/loaders.rs:280-289
// (path.get_full_extension() returns the substring after the FIRST dot,
//  so "floor_01.dungeon.ron" → "dungeon.ron"; the matcher tries this first.)

use bevy::prelude::*;
use bevy_common_assets::ron::RonAssetPlugin;
use crate::data::{DungeonFloor, EnemyDb, ItemDb, ClassTable, SpellTable};

impl Plugin for LoadingPlugin {
    fn build(&self, app: &mut App) {
        app
            // One RonAssetPlugin per asset type. The string is the FULL multi-dot
            // extension WITHOUT a leading dot. Bevy's get_full_extension() strips
            // everything before the first '.' from the file name.
            .add_plugins(RonAssetPlugin::<DungeonFloor>::new(&["dungeon.ron"]))
            .add_plugins(RonAssetPlugin::<ItemDb>::new(&["items.ron"]))
            .add_plugins(RonAssetPlugin::<EnemyDb>::new(&["enemies.ron"]))
            .add_plugins(RonAssetPlugin::<ClassTable>::new(&["classes.ron"]))
            .add_plugins(RonAssetPlugin::<SpellTable>::new(&["spells.ron"]));
        // ... the rest of LoadingPlugin (asset collection, state wiring, etc.)
    }
}
```

**Why this works (Bevy-side):**

- Bevy's path-to-loader resolution is `AssetServer.loaders.get_by_path(path)` → `path.get_full_extension()` → `extension_to_loaders.get(extension)` (verified at `bevy_asset-0.18.1/src/server/loaders.rs:280-289`).
- `get_full_extension` for `floor_01.dungeon.ron` returns `"dungeon.ron"` (verified at `bevy_asset-0.18.1/src/path.rs:464-490`: "Returns the full extension (including multiple `.` values). Ex: Returns `\"config.ron\"` for `\"my_asset.config.ron\"`.").
- If the registered extension does not match the full extension, the matcher falls back through `iter_secondary_extensions`, which yields each substring after each subsequent `.` (so `"dungeon.ron"` → `"ron"`). So if you only registered `"ron"` you'd get a single shared loader for every `.ron` file — which is exactly what we **don't** want, because we have five different types. Registering `"dungeon.ron"` etc. ensures each type has its own dedicated extension.

**Tradeoff: extension uniqueness across types.** Because bevy_common_assets registers a per-type loader keyed by extension, two asset types **cannot** share an extension. Our brief uses `.dungeon.ron`, `.items.ron`, `.enemies.ron`, `.classes.ron`, `.spells.ron` — five distinct full extensions, so this is fine. Don't be tempted to use plain `.ron` for any one of them.

**One subtle gotcha (MEDIUM confidence; flag for verification):** older `bevy_common_assets` versions exposed the constructor as `RonAssetPlugin::new(&[".dungeon.ron"])` *with* the leading dot. The 0.18-compatible version almost certainly aligns with Bevy 0.18's own loader convention (no leading dot, see `bevy-0.18.1/examples/asset/custom_asset.rs:51-53`: `&["custom"]`). Confirm against the README of the version you pin.

---

### Question 3 — `bevy_asset_loader::AssetCollection` derive + `LoadingState` plumbing (Bevy 0.18-compatible)

**Confidence: MEDIUM-HIGH on the API shape; MEDIUM on the chained method names (`load_collection` vs. `add_collection_to_loading_state`) without docs.rs access.**

The brief specifically asks: "Where does `.add_collection_to_loading_state::<_, DungeonAssets>` go vs. `.load_collection::<DungeonAssets>` — pick the current 0.18-supported pattern."

**Recommended pattern (current ergonomic API for `bevy_asset_loader 0.20+`, MEDIUM-HIGH confidence):**

```rust
use bevy::prelude::*;
use bevy_asset_loader::prelude::*;
use crate::plugins::state::GameState;

#[derive(AssetCollection, Resource)]
pub struct DungeonAssets {
    // Placeholder: the brief mandates "placeholder-quality initial assets".
    // For Feature #3 a single font is the minimum-viable smoke test; richer
    // collections (textures, audio) land in #6, #8, #11.
    #[asset(path = "fonts/FiraSans-Bold.ttf")]
    pub default_font: Handle<Font>,
    // Loaded RON examples (one per type, picked up by the per-type extension):
    #[asset(path = "dungeons/floor_01.dungeon.ron")]
    pub floor_01: Handle<crate::data::DungeonFloor>,
    #[asset(path = "items/core.items.ron")]
    pub item_db: Handle<crate::data::ItemDb>,
    #[asset(path = "enemies/core.enemies.ron")]
    pub enemy_db: Handle<crate::data::EnemyDb>,
    #[asset(path = "classes/core.classes.ron")]
    pub class_table: Handle<crate::data::ClassTable>,
    #[asset(path = "spells/core.spells.ron")]
    pub spell_table: Handle<crate::data::SpellTable>,
}

impl Plugin for LoadingPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_plugins((
                RonAssetPlugin::<crate::data::DungeonFloor>::new(&["dungeon.ron"]),
                // ...other RonAssetPlugins as in §Question 2...
            ))
            .add_loading_state(
                LoadingState::new(GameState::Loading)
                    .continue_to_state(GameState::TitleScreen)
                    .load_collection::<DungeonAssets>(),
            );
    }
}
```

**Why `load_collection` over `add_collection_to_loading_state`:**

- `add_collection_to_loading_state::<S, T>(...)` is the **older** standalone form, used pre-0.20-or-so when `LoadingState` was less builder-friendly. It still exists as a back-compat helper but is no longer the documented primary API.
- `LoadingState::new(...).continue_to_state(...).load_collection::<T>()` is the **current** chained-builder form. One method call to register the collection; same effect as the older two-call shape.

**Two derives required, not one.** `AssetCollection` adds the loading-state machinery; `Resource` is required because `LoadingState` inserts the populated collection as a `Resource` once all assets are ready. Forgetting `Resource` produces an opaque "trait `Resource` not implemented" error at the `load_collection` call site.

**The `prelude` module covers everything** — `bevy_asset_loader::prelude::*` brings in `AssetCollection`, `LoadingState`, `LoadingStateAppExt` (the trait that adds `add_loading_state` to `App`), and the `#[asset(...)]` attribute macro support.

---

### Question 4 — Hot-reload API for Bevy 0.18.1

**Confidence: HIGH** (verified directly against `bevy_asset-0.18.1/src/lib.rs`).

**Verified shape:**

```rust
// In main.rs, replacing the bare `DefaultPlugins`:
use bevy::asset::AssetPlugin;

App::new()
    .add_plugins(
        DefaultPlugins.set(AssetPlugin {
            // Forces hot-reload on, regardless of whether the `watch` cargo
            // feature was set. Combined with `bevy/file_watcher` in Cargo.toml's
            // `dev` feature, this gives us hot-reload only under
            // `cargo run --features dev`.
            watch_for_changes_override: Some(true),
            ..default()
        }),
    )
    // ... rest of plugin tuple
```

**Two pieces required:**

1. **Cargo feature `bevy/file_watcher`** — adds the `notify-debouncer-full` crate and the watch infrastructure. Verified: `bevy-0.18.1/Cargo.toml:2487` (`file_watcher = ["bevy_internal/file_watcher"]`) → `bevy_internal-0.18.1/Cargo.toml:235` (`file_watcher = ["bevy_asset?/file_watcher"]`) → `bevy_asset-0.18.1/Cargo.toml:40-44` (`file_watcher = ["notify-debouncer-full", "watch", "multi_threaded"]`).
2. **`AssetPlugin { watch_for_changes_override: Some(true), .. }`** — actually flips the boolean. Default is `None`, which falls back to `cfg!(feature = "watch")` (verified at `bevy_asset-0.18.1/src/lib.rs:369-371`).

**API the brief asked about, debunked:**

- ✅ `AssetPlugin { watch_for_changes_override: Some(true), .. }` — verified at `bevy_asset-0.18.1/src/lib.rs:248`.
- ❌ `AssetServer::watch_for_changes_override` — does **not** exist as a runtime method. Only `AssetServer::watching_for_changes()` (read-only getter, `bevy_asset-0.18.1/src/server/mod.rs:189-192`) exists. The brief's wording was slightly off.
- ❌ `AssetServer::watch_for_changes()` — does **not** exist in 0.18.1. Older Bevy (~0.10 era) had a runtime toggle method that has since been removed.

**Recommended gating shape:**

```toml
# Cargo.toml — fold file_watcher into the existing `dev` feature.
[features]
default = []
dev = ["bevy/dynamic_linking", "bevy/file_watcher"]
```

```rust
// src/main.rs — feature-gate the override too, so non-dev builds get
// AssetPlugin::default()'s no-watch behavior.
use bevy::prelude::*;
use bevy::asset::AssetPlugin;

fn main() {
    let asset_plugin = AssetPlugin {
        // When the `dev` feature is on, hot-reload is on; otherwise off.
        // The override is only needed because we also enable the cargo feature;
        // without the override, watch defaults to `cfg!(feature = "watch")`,
        // which is true under `--features dev` already, so the override is
        // belt-and-braces. Keep it explicit to make the intent obvious.
        watch_for_changes_override: Some(cfg!(feature = "dev")),
        ..default()
    };

    App::new()
        .add_plugins((
            DefaultPlugins.set(asset_plugin),
            // ... rest of the existing plugin tuple
        ))
        .run();
}
```

**Note on `cfg!(feature = "dev")` in `main.rs`:** the `dev` feature is local to our crate (per `project/resources/20260501-102842-dev-feature-pattern.md`). `cfg!(feature = "dev")` evaluates correctly in `src/main.rs` because `main.rs` is compiled with our crate's feature set.

---

### Question 5 — `Asset` trait derive list (Bevy 0.18)

**Confidence: HIGH** (verified directly against `bevy_asset-0.18.1/src/assets.rs:456` and `bevy_asset_macros-0.18.1/src/lib.rs:17-33`).

**Verified facts:**

1. `Asset: VisitAssetDependencies + TypePath + Send + Sync + 'static` (`bevy_asset-0.18.1/src/assets.rs:456`). So an `Asset` implementor must have `TypePath` and `VisitAssetDependencies` in scope.
2. The `Asset` derive (`bevy_asset_macros-0.18.1/src/lib.rs:17-33`) emits **both** the empty `impl Asset for T {}` and the `VisitAssetDependencies` impl (via `derive_dependency_visitor_internal`). So `#[derive(Asset)]` alone gives you `Asset + VisitAssetDependencies`. You still need `TypePath` from somewhere.
3. The `Reflect` derive (`bevy_reflect_derive-0.18.1/src/lib.rs:822, 487-494`) supplies `TypePath` automatically. So **`#[derive(Asset, Reflect)]` is sufficient**.
4. Alternative: `#[derive(Asset, TypePath)]` — Bevy's own `LoadedUntypedAsset` uses this (`bevy_asset-0.18.1/src/assets.rs:96`). `TypePath` derive alone (no full `Reflect`) is lighter.

**Recommended derive list for our stub asset types:**

```rust
// Source: bevy_asset-0.18.1/src/assets.rs:456 (trait bounds);
//         bevy_asset_macros-0.18.1/src/lib.rs:17-33 (Asset derive);
//         bevy-0.18.1/examples/asset/custom_asset.rs:11 (canonical Bevy 0.18 example).

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Asset, Reflect, Serialize, Deserialize, Default, Debug, Clone)]
pub struct DungeonFloor {
    // Empty body for Feature #3. Feature #4 fills with razor-wall grid.
}
```

The brief's proposed derive list is correct as-is — verified.

**Why each derive earns its keep:**

| Derive | Why it's there |
| ------ | -------------- |
| `Asset` | Marker trait + auto-impl of `VisitAssetDependencies`. **Required** for `Handle<DungeonFloor>` to be a thing. |
| `Reflect` | Supplies `TypePath` (trait-bound requirement for `Asset`). Also enables `bevy_inspector_egui`-style debug overlays later. |
| `Serialize`, `Deserialize` | RON round-trip via serde. Required by `bevy_common_assets::RonAssetPlugin`'s loader. |
| `Default` | Lets us spawn a stub instance in the round-trip test without filling fields. Also useful for `bevy_asset_loader`'s sanity checks. |
| `Debug` | `info!("loaded: {:?}", floor)` and friends. Cheap. |
| `Clone` | Needed for `bevy_asset_loader::AssetCollection`'s populated-resource pathway. Also useful in tests. |

---

### Question 6 — Minimum-viable centered "Loading..." Text in Bevy 0.18

**Confidence: HIGH** (verified directly against `bevy_ui-0.18.1/src/widget/text.rs` and `bevy_ui-0.18.1/src/ui_node.rs`).

**Verified facts:**

1. `Text::new("...")` is the 0.18 component constructor (`bevy_ui-0.18.1/src/widget/text.rs:113-117`). The component is `Text(pub String)`.
2. `Text` carries `#[require(Node, TextLayout, TextFont, TextColor, LineHeight, TextNodeFlags, ContentSize, FontHinting::Disabled)]` (`bevy_ui-0.18.1/src/widget/text.rs:97-109`). Bevy 0.18's `#[require(...)]` attribute auto-attaches missing components when the entity is spawned, so spawning `(Text::new(...), Node { ... })` is enough — Bevy fills in the rest.
3. Centering is done with `Node { width: Val::Percent(100.0), height: Val::Percent(100.0), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() }` plus a child holding the Text.

**Verified-shape spawn:**

```rust
// Source: bevy_ui-0.18.1/src/widget/text.rs:97-109 (#[require(...)] on Text)
//         bevy_ui-0.18.1/src/widget/text.rs:113-117 (Text::new)
//         bevy_ui-0.18.1/src/ui_node.rs:500, 608, 649 (Node fields)

use bevy::prelude::*;
use crate::plugins::state::GameState;

#[derive(Component)]
struct LoadingScreenRoot; // marker so we can despawn on OnExit

fn spawn_loading_screen(mut commands: Commands) {
    commands
        .spawn((
            // Full-screen flex container with everything centered.
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            LoadingScreenRoot,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("Loading..."),
                // No font handle: Bevy 0.18 uses the embedded default font
                // when `default_font` is enabled (default for `features = ["3d"]`,
                // verified at bevy-0.18.1/Cargo.toml:2454-2466 default_platform).
            ));
        });
}

fn despawn_loading_screen(
    mut commands: Commands,
    roots: Query<Entity, With<LoadingScreenRoot>>,
) {
    for e in &roots {
        // Bevy 0.18's `despawn` is recursive by default
        // (research §State of the Art line 1234).
        commands.entity(e).despawn();
    }
}

// Wired in LoadingPlugin::build:
// app.add_systems(OnEnter(GameState::Loading), spawn_loading_screen)
//    .add_systems(OnExit(GameState::Loading),  despawn_loading_screen);
```

**Three Bevy-0.18-specific notes:**

- **No `TextBundle`.** The `*Bundle` types are gone in 0.18; everything is component-driven via `#[require(...)]`. This is the post-0.15-or-so migration; 0.18 has settled into it.
- **A camera is required** for any `bevy_ui` content to render. Spawn a `Camera2d` (zero-config) somewhere — Feature #3 doesn't have one yet because Feature #1 spawns nothing. The `LoadingPlugin::build` should spawn a `Camera2d::default()` on `OnEnter(GameState::Loading)` and despawn it on `OnExit`. Or live with a black screen — but the brief says "render a centered Loading...", which implies the camera. **Add a Camera2d on OnEnter and despawn on OnExit alongside the text**. (Or spawn once at startup and leave it; pick one; the planner should decide.)
- **The default font ships with `bevy_text`'s `default_font` feature.** Our `features = ["3d"]` already pulls this in (`bevy-0.18.1/Cargo.toml:2454-2466`, `default_platform` includes `default_font`). So we can omit a `TextFont { font: ... }` and Bevy falls back to its embedded ~250 KB default sans-serif font. Good enough for "Loading..." — Feature #25 (polish) replaces it.

---

### Question 7 — `LoadingState` integration with our `GameState` enum

**Confidence: MEDIUM-HIGH on the API shape; HIGH that no sub-state wrapper is needed.**

**The verified-against-Bevy-0.18 piece:**

`States` derive is exactly what `bevy_asset_loader` needs to integrate. `LoadingState::new(S)` takes any type that implements `States` (an exported Bevy 0.18 trait). Our `GameState` (`src/plugins/state/mod.rs:6-15`) already derives `States` (verified by reading the file directly). No wrapper, no sub-state, no transformation.

**The MEDIUM-confidence piece:**

`bevy_asset_loader 0.20+`'s `LoadingState` API surface, based on training data and the repo README pattern:

```rust
// Source: bevy_asset_loader README pattern (training data + crate convention).
// VERIFY against the resolved version's docs.rs page after pinning.

use bevy::prelude::*;
use bevy_asset_loader::prelude::*;
use crate::plugins::state::GameState;

impl Plugin for LoadingPlugin {
    fn build(&self, app: &mut App) {
        app.add_loading_state(
            LoadingState::new(GameState::Loading)
                .continue_to_state(GameState::TitleScreen)
                .load_collection::<DungeonAssets>(),
        );
    }
}
```

What this does mechanically:
- On `OnEnter(GameState::Loading)`, `bevy_asset_loader` schedules its internal "kick off all collection loads" system.
- On every `Update` while `GameState == Loading`, an internal poll system checks each `Handle<T>` in `DungeonAssets` via Bevy's `AssetServer::is_loaded_with_dependencies` (the same API surfaced at `bevy_asset-0.18.1/src/server/mod.rs:1252`).
- Once all handles report loaded-with-deps, `LoadingState`'s internal system inserts the populated `DungeonAssets` resource and calls `next.set(GameState::TitleScreen)`. State transitions are deferred by one frame (Feature #2 §Pitfall 7 in the state machine anatomy resource); the next frame, `OnEnter(GameState::TitleScreen)` fires, our `LoadingScreenRoot` is despawned via `OnExit(GameState::Loading)`, and TitleScreen takes over.

**Critical: This satisfies the Feature #2 contract** that `GameState::Loading` has no auto-advance from anywhere else. `LoadingPlugin` is the only owner of the `Loading → TitleScreen` transition, exactly as `project_druum_state_machine.md` requires (planner memory entry: "Feature #3 (asset/RON pipeline) owns the `Loading → TitleScreen` transition and must not be pre-empted").

**No sub-state wrapper is needed.** `bevy_asset_loader` works with top-level `States` directly. (Internally it may use its own sub-states for tracking load progress; that's an implementation detail that doesn't leak into our app.)

---

### Question 8 — RON round-trip test mechanics

**Confidence: HIGH** (the `ron::de::from_str` / `ron::ser::to_string_pretty` API has been stable since `ron 0.7` per training data; current version 0.12.1 from `Cargo.lock:4192-4195` continues this).

**The verified path:**

```rust
// Source: ron 0.12 stable API (training data; APIs unchanged since 0.7-0.8).
// Confirmed transitively present via Cargo.lock:4192-4195 (ron 0.12.1).
// No App, no AssetServer, no Bevy plugins needed — pure unit test.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dungeon_floor_round_trips_through_ron() {
        let original = DungeonFloor::default();

        // Serialize → string. Use to_string_pretty for diff-friendly RON
        // (one field per line); to_string is fine if you only care about
        // semantic equivalence.
        let serialized: String = ron::ser::to_string_pretty(
            &original,
            ron::ser::PrettyConfig::default(),
        ).expect("serialize");

        // Deserialize back.
        let parsed: DungeonFloor = ron::de::from_str(&serialized)
            .expect("deserialize");

        // Re-serialize and diff against the first serialization. If the
        // round trip is lossless and serde derives are symmetric, the
        // two strings are byte-identical.
        let reserialized: String = ron::ser::to_string_pretty(
            &parsed,
            ron::ser::PrettyConfig::default(),
        ).expect("re-serialize");

        assert_eq!(serialized, reserialized,
            "RON round trip lost or reordered fields");
    }
}
```

**Why this is the cleanest approach (vs. spinning up an `App`):**

- No async runtime needed.
- No mock `AssetReader`.
- Test runs in <1 ms.
- Failure messages are diff-friendly because both sides are pretty-printed RON.
- The test verifies the **serde derives**, which is what we actually care about. If the serde derives are right, `bevy_common_assets` will load it; that crate's loader is just `ron::de::from_bytes` plus error wrapping (verified via the canonical pattern in `bevy-0.18.1/examples/asset/custom_asset.rs:39-49` — the example uses the same shape).
- `Default` provides a workable starting struct without filling every field by hand.

**Why NOT to use `App`-based testing here:**

- Spinning up an `App` with `AssetPlugin` for RON loading is doable but slow (~30 ms per test) and pulls in the full asset graph.
- The brief explicitly asks: "How do you load a RON string in a unit test without spinning up a full `App` + AssetServer?" The answer is: skip the `App` entirely. Test the schema, not the loader machinery.
- An `App`-based integration test belongs in Feature #4 (when `DungeonFloor` actually has fields and you want to verify a real `assets/dungeons/floor_01.dungeon.ron` parses), not Feature #3.

**Where to put the test:** inside the file that defines `DungeonFloor` (likely `src/data/dungeon.rs` per Option A). One round-trip test per type is overkill for stubs; one test on `DungeonFloor` covers serde-derive correctness across the whole shape and is the brief's specific ask.

---

## Architecture Patterns

### Recommended Project Structure

```
src/
├── data/
│   ├── mod.rs                    # was placeholder; now `pub mod dungeon; pub mod items; ...`
│   ├── dungeon.rs                # stub `DungeonFloor` + round-trip test
│   ├── items.rs                  # stub `ItemDb`
│   ├── enemies.rs                # stub `EnemyDb`
│   ├── classes.rs                # stub `ClassTable`
│   └── spells.rs                 # stub `SpellTable`
├── plugins/
│   ├── mod.rs                    # add `pub mod loading;`
│   ├── loading/
│   │   └── mod.rs                # LoadingPlugin: RonAssetPlugins + LoadingState + UI
│   ├── state/mod.rs              # unchanged
│   ├── dungeon/mod.rs            # unchanged (Feature #4 will start filling it)
│   ├── combat/mod.rs             # unchanged
│   ├── town/mod.rs               # unchanged
│   ├── party/mod.rs              # unchanged
│   ├── ui/mod.rs                 # unchanged
│   ├── audio/mod.rs              # unchanged
│   └── save/mod.rs               # unchanged
├── lib.rs                        # unchanged
└── main.rs                       # add LoadingPlugin to add_plugins; configure AssetPlugin override

assets/
├── README.md                     # NEW — directory layout doc
├── dungeons/floor_01.dungeon.ron # 1+ placeholder
├── items/core.items.ron          # 1+ placeholder
├── enemies/core.enemies.ron      # 1+ placeholder
├── classes/core.classes.ron      # 1+ placeholder
└── spells/core.spells.ron        # 1+ placeholder
```

### Pattern 1: One module per asset schema in `src/data/`

**What:** Each top-level RON-loaded asset type gets its own `src/data/<name>.rs` file with the struct, derives, and round-trip test alongside.

**When to use:** every RON-loaded asset type from now on.

**Example (verified shape):**

```rust
// src/data/dungeon.rs
//
// Source: bevy_asset-0.18.1/src/assets.rs:456 (Asset trait bounds);
//         bevy-0.18.1/examples/asset/custom_asset.rs:11 (derive shape).

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Asset, Reflect, Serialize, Deserialize, Default, Debug, Clone)]
pub struct DungeonFloor {
    // Stub for Feature #3. Feature #4 fills in razor-wall grid.
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dungeon_floor_round_trips_through_ron() {
        let original = DungeonFloor::default();
        let s = ron::ser::to_string_pretty(&original, ron::ser::PrettyConfig::default()).unwrap();
        let parsed: DungeonFloor = ron::de::from_str(&s).unwrap();
        let s2 = ron::ser::to_string_pretty(&parsed, ron::ser::PrettyConfig::default()).unwrap();
        assert_eq!(s, s2);
    }
}
```

### Pattern 2: `LoadingPlugin` shape

**What:** One plugin owns: (a) registering all `RonAssetPlugin<T>`s, (b) the `AssetCollection`, (c) the `add_loading_state(...)` call, (d) the loading-screen UI on `OnEnter`/`OnExit`.

**When to use:** exactly one of these in the project, registered immediately after `StatePlugin` in `main.rs`.

**Example (composition of all earlier verified pieces):**

```rust
// src/plugins/loading/mod.rs
//
// Sources cited inline.

use bevy::prelude::*;
use bevy_asset_loader::prelude::*;
use bevy_common_assets::ron::RonAssetPlugin;
use crate::plugins::state::GameState;
use crate::data::{DungeonFloor, ItemDb, EnemyDb, ClassTable, SpellTable};

pub struct LoadingPlugin;

impl Plugin for LoadingPlugin {
    fn build(&self, app: &mut App) {
        app
            // (1) Register one typed RON loader per extension.
            //     Source: bevy_common_assets crate README pattern; Bevy path
            //             matcher at bevy_asset-0.18.1/src/server/loaders.rs:280-289.
            .add_plugins((
                RonAssetPlugin::<DungeonFloor>::new(&["dungeon.ron"]),
                RonAssetPlugin::<ItemDb>::new(&["items.ron"]),
                RonAssetPlugin::<EnemyDb>::new(&["enemies.ron"]),
                RonAssetPlugin::<ClassTable>::new(&["classes.ron"]),
                RonAssetPlugin::<SpellTable>::new(&["spells.ron"]),
            ))
            // (2) Drive the GameState::Loading -> TitleScreen transition once
            //     all assets in DungeonAssets report LoadedWithDependencies.
            //     Source: bevy_asset_loader crate README pattern.
            .add_loading_state(
                LoadingState::new(GameState::Loading)
                    .continue_to_state(GameState::TitleScreen)
                    .load_collection::<DungeonAssets>(),
            )
            // (3) Loading-screen UI lifecycle.
            //     Sources: bevy_ui-0.18.1/src/widget/text.rs:97-117 (Text + #[require])
            //              bevy_ui-0.18.1/src/ui_node.rs:500, 608, 649 (Node fields)
            .add_systems(OnEnter(GameState::Loading), spawn_loading_screen)
            .add_systems(OnExit(GameState::Loading), despawn_loading_screen);
    }
}

// (See §Question 3 for DungeonAssets and §Question 6 for spawn/despawn fns.)
```

### Pattern 3: AssetPlugin watch override in `main.rs`

```rust
// src/main.rs (excerpt; existing structure stays the same)
//
// Source: bevy_asset-0.18.1/src/lib.rs:237-264 (AssetPlugin), :333-345 (default).

use bevy::prelude::*;
use bevy::asset::AssetPlugin;
use druum::plugins::{
    audio::AudioPlugin, combat::CombatPlugin, dungeon::DungeonPlugin,
    loading::LoadingPlugin,        // NEW
    party::PartyPlugin, save::SavePlugin, state::StatePlugin,
    town::TownPlugin, ui::UiPlugin,
};

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins.set(AssetPlugin {
                // Hot-reload only when --features dev is on.
                watch_for_changes_override: Some(cfg!(feature = "dev")),
                ..default()
            }),
            StatePlugin,
            LoadingPlugin,            // NEW — must come after StatePlugin so GameState exists
            DungeonPlugin,
            CombatPlugin,
            PartyPlugin,
            TownPlugin,
            UiPlugin,
            AudioPlugin,
            SavePlugin,
        ))
        .run();
}
```

### Anti-Patterns to Avoid

- **Spinning up a full `App` for the round-trip test.** Use `ron::de::from_str` and `ron::ser::to_string_pretty` directly. See §Question 8.
- **Reading `AssetEvent<T>` with `EventReader`.** It is a `Message` in 0.18 (verified at `bevy_asset-0.18.1/src/event.rs:9, 49`). Use `MessageReader<AssetEvent<T>>` if you need to read it directly. Routing through `bevy_asset_loader` avoids this entirely.
- **Calling `next.set(GameState::TitleScreen)` from anywhere outside `LoadingPlugin`.** Per `project_druum_state_machine.md`: "GameState::Loading is the default and intentionally has no auto-advance. Feature #3 (asset/RON pipeline) owns the Loading → TitleScreen transition." `bevy_asset_loader` makes the transition for us; do not also wire a manual `next.set` on top.
- **Shipping `bevy/file_watcher` in release builds.** Notify-based filesystem watchers add per-asset overhead and a small handful of OS file handles. Acceptable in dev; not in shipped builds. Gate via `dev` (per Feature #1's pattern).
- **Registering plain `"ron"` as the extension for any one type.** It will swallow all five categories under the same loader and our type-dispatch breaks. Always use the full multi-dot extension (`"dungeon.ron"` etc.) — see §Question 2.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
| ------- | ----------- | ----------- | --- |
| Custom-extension typed asset loader | Per-type `impl AssetLoader for T` | `bevy_common_assets::ron::RonAssetPlugin` | Saves ~30 LOC per type; no error-handling boilerplate; idiomatic in Bevy ecosystem. |
| Asset readiness polling + state transition | Hand-rolled "check each handle on every Update" system | `bevy_asset_loader::LoadingState` + `#[derive(AssetCollection)]` | Declarative; one chained-builder call replaces ~50 LOC of polling and a custom `Resource` per asset bag. |
| RON serialization round-trip | Hand-rolled `Display`/parse | `ron` (already transitively present via Bevy) | `ron 0.12.1` is the same version Bevy uses internally. No version-skew risk. |
| Default font for "Loading..." text | Asset-bundled font in placeholder dungeons | Bevy's `default_font` (already enabled via `features = ["3d"]` → `default_platform`) | Embedded ~250 KB sans-serif works for placeholder-quality text. Replace in Feature #25 (polish). |

---

## Common Pitfalls

### Pitfall 1: `AssetEvent<T>` is a `Message`, not an `Event` (0.18-specific trap)

**What goes wrong:** Code that reads `EventReader<AssetEvent<T>>` will not compile in Bevy 0.18.

**Why it happens:** Bevy 0.17 → 0.18 split the buffered-event family in two: `Event` (one-shot, observer-based) and `Message` (buffered, polling-based). `AssetEvent<T>` and `StateTransitionEvent<S>` are both `Message`s now (verified at `bevy_asset-0.18.1/src/event.rs:9, 49`). Older blog posts and 0.17-era examples will mislead.

**How to avoid:** If you need to read asset load events, use `MessageReader<AssetEvent<T>>`. Better yet, rely on `bevy_asset_loader::LoadingState` to abstract the polling — that crate uses `AssetServer::is_loaded_with_dependencies` directly, so the event-reading question never comes up.

### Pitfall 2: Hot-reload silently disabled if only one of two pieces is configured

**What goes wrong:** You enable `bevy/file_watcher` in `Cargo.toml` but forget the `AssetPlugin` override. Or vice versa. In either case hot-reload appears to work in cargo features but doesn't fire.

**Why it happens:** `AssetPlugin::default()` sets `watch_for_changes_override = None`, which falls back to `cfg!(feature = "watch")`. The `watch` cargo feature inside `bevy_asset` is set transitively when `file_watcher` is enabled — so in theory the cfg-fallback works without the override. But the override is the one place a future contributor can read a single boolean and know hot-reload is on. Belt-and-braces is the safer pattern.

**How to avoid:** Always set both pieces explicitly. Use `Some(cfg!(feature = "dev"))` so the override is visibly tied to the cargo feature.

### Pitfall 3: Two asset types sharing an extension via `bevy_common_assets`

**What goes wrong:** You register `RonAssetPlugin::<ItemDb>::new(&["ron"])` and `RonAssetPlugin::<EnemyDb>::new(&["ron"])`. Bevy's loader registry stores extension → loader as a multi-map; the **last** registered loader wins on dispatch, but both produce a `warn!("Multiple AssetLoaders found ...")` (verified at `bevy_asset-0.18.1/src/server/loaders.rs:248-262`). You get `ItemDb` and `EnemyDb` from the same `.ron` file by accident, with deserialization failures depending on which loader runs.

**How to avoid:** One full multi-dot extension per type — `.dungeon.ron`, `.items.ron`, etc. The brief specifies these; just stick to them.

### Pitfall 4: Forgetting `Resource` derive on the `AssetCollection` struct

**What goes wrong:** Compile error at `.load_collection::<DungeonAssets>()` saying "trait `Resource` not implemented".

**Why it happens:** `bevy_asset_loader` expects to `commands.insert_resource(populated_collection)` once all handles are loaded. The collection must be both `AssetCollection` (the loader-machinery trait) and `Resource` (the Bevy ECS resource trait).

**How to avoid:** Always derive both: `#[derive(AssetCollection, Resource)]`.

### Pitfall 5: Spawning UI without a camera

**What goes wrong:** `bevy_ui` content renders to a 2D camera. Without one in the world, the UI tree exists but is never drawn — looks like nothing happened.

**Why it happens:** Feature #1 spawned no entities. The skeleton is camera-less. The brief asks for a "centered Loading... Text" which only renders if a camera is present.

**How to avoid:** In `LoadingPlugin::build`, spawn a `Camera2d::default()` on `OnEnter(GameState::Loading)` (or once at `Startup`). Decide whether to despawn on `OnExit` or keep it for later UI screens. Suggested: keep one global Camera2d at startup so later UI features (Title, Town, Combat menus, Auto-map) can rely on it.

### Pitfall 6: Stale local view of `bevy_common_assets` / `bevy_asset_loader` versions

**What goes wrong:** The roadmap was written 2026-04-29 with "verify versions on crates.io target Bevy 0.18 — pin exact versions" as an open item. If the planner skips that step and pins an old version from training data, the build may fail or pull a Bevy-0.17-incompatible release.

**How to avoid:** Run §Question 1's verification recipe **before** editing `Cargo.toml`. If either crate lags, escalate per the `moonshine-save` precedent.

### Pitfall 7: Architectural — `bevy_asset_loader` macros produce hard-to-read errors when an asset path is wrong

**What goes wrong (carry-over from research §Cons line 213):** A typo in `#[asset(path = "fonts/FiraSans-Bold.ttf")]` — say `fonts/FiraSans-Bold.tff` — produces an error at runtime, not at compile time, and the error surface is one of `bevy_asset_loader`'s wrapping errors that doesn't always cite the bad path clearly.

**Why it happens:** The `#[asset(path = ...)]` macro emits a `asset_server.load("...")` call. Bevy's `AssetServer` reports missing-file errors via `AssetLoadFailedEvent`, which `bevy_asset_loader` swallows and re-emits as a generic loading-state failure.

**How to avoid:**
- Keep the placeholder asset list **small** in Feature #3 (the brief says "minimum-viable", which I read as 5-6 placeholder files, one per type plus a font).
- Always log the resolved asset path from `OnEnter(GameState::Loading)` in dev builds:
  ```rust
  #[cfg(feature = "dev")]
  fn log_asset_paths(/* read assets resource */) { /* info!("...") */ }
  ```
- For Feature #3, run the smoke test by deleting one placeholder file and confirming the loading screen stalls — this verifies the error path works at all.

---

## Security

### Known Vulnerabilities

| Library | CVE / Advisory | Severity | Status | Action |
| ------- | -------------- | -------- | ------ | ------ |
| `bevy_asset 0.18.1` | None found in training data | — | — | Monitor [RustSec](https://rustsec.org/advisories/) when pinning. |
| `bevy_common_assets` (pending pin) | Cannot verify without crates.io | — | — | Run `cargo audit` after first lockfile generation. |
| `bevy_asset_loader` (pending pin) | Cannot verify without crates.io | — | — | Same as above. |
| `serde 1` | None recent (HIGH-confidence stable, decade-old crate) | — | — | — |
| `ron 0.12.1` | None known in training data | — | — | Monitor. |

(I cannot run `cargo audit` from this session — the planner / implementer should run it once after `Cargo.lock` is updated.)

### Architectural Security Risks

| Risk | Affected Architecture Options | How It Manifests | Secure Pattern | Anti-Pattern to Avoid |
| ---- | ----------------------------- | ---------------- | -------------- | --------------------- |
| RON deserialization of untrusted input | Any path that reads RON from outside `assets/` | Someone replaces `assets/dungeons/floor_01.dungeon.ron` with malicious RON; `ron` parser may panic on certain inputs (RON is non-streaming and has known DoS vectors with deeply nested input). | For Feature #3 the only RON loaded comes from `assets/`, which is fixed at build time and shipped with the binary. Bevy's `AssetPlugin::default()` sets `unapproved_path_mode = UnapprovedPathMode::Forbid` (verified at `bevy_asset-0.18.1/src/lib.rs:286-287, 333-344`), which blocks loads from outside the approved folder. Keep this default. | Setting `unapproved_path_mode: UnapprovedPathMode::Allow`. The 0.18 source explicitly warns: "It is strongly discouraged to use `Allow` if your app will include scripts or modding support" (`bevy_asset-0.18.1/src/lib.rs:271-273`). |
| Path-traversal in mod / save loading | Any future loader that takes a user-supplied path | A modded save file points `Handle<DungeonFloor>` at `../../etc/passwd` | Same `UnapprovedPathMode::Forbid` default. The asset path normalization in `bevy_asset-0.18.1/src/path.rs:492-` rejects unapproved paths. | Bypassing the approved-path mode for "convenience". |
| Large-RON DoS at load | RON files in `assets/` | Hand-authored or mod-supplied RON with billion-laughs-style nesting causes the parser to allocate gigabytes. | RON's parser has reasonable depth/size limits in 0.12; for Feature #3 we ship our own assets so the threat model is narrow. Document in `assets/README.md` that all RON should be hand-written or editor-output, not arbitrary internet input. | Loading RON from user-uploaded content without size/depth limits. (Out of scope for Feature #3 — flag for #24 dungeon editor.) |

### Trust Boundaries

For Feature #3, the only data that crosses into the program is:

| Boundary | What enters | Validation required | What happens if skipped |
| -------- | ----------- | ------------------- | ----------------------- |
| `assets/` directory at startup | Five RON placeholder files + (later) fonts/textures | Bevy's `UnapprovedPathMode::Forbid` blocks paths outside `assets/`. RON parser handles malformed input via `ron::de` returning `Result`. | A typo in a path silently fails to load, surfacing as a `LoadingState` stall. Mitigate via §Pitfall 7's logging. |
| `assets/` watcher (when `--features dev`) | Hot-reloaded RON edits | Same as above; the watcher is just a re-trigger. | Edits during play could re-spawn dungeon geometry mid-frame in later features (Feature #4+); flag in Feature #4 plan. |

---

## Performance

| Metric | Value / Range | Source | Notes |
| ------ | ------------- | ------ | ----- |
| RON parse throughput | ~50-200 MB/s on small structs | Training data; 2024-era benchmarks for `ron 0.8`+ | Our placeholder files will be <10 KB; parse time is negligible. |
| Loading-state poll overhead | One `AssetServer::is_loaded_with_dependencies` call per handle per Update | Inferred from `bevy_asset-0.18.1/src/server/mod.rs:1252` and typical `bevy_asset_loader` design | With 6-10 handles in `DungeonAssets`, this is sub-millisecond. |
| Hot-reload watch overhead | One `notify-debouncer-full` thread + ~2 OS file handles per asset directory | `bevy_asset-0.18.1/Cargo.toml:40-44` (the feature pulls in `notify-debouncer-full`) + 2026-era `notify` crate behavior | Acceptable in dev; not enabled in release (gated by our `dev` cargo feature). |
| Bundle size impact | +5-15 KB compressed (the two crates plus their light dep tree) | Estimate; precise bundle measurement not possible without crates on disk | Tiny. Dwarfed by Bevy itself. |

(No formal benchmarks for `bevy_common_assets` / `bevy_asset_loader` were found in training data. The metrics above are upper-bound estimates; flag for validation during implementation.)

---

## Code Examples

### Stub `DungeonFloor` schema with round-trip test

```rust
// src/data/dungeon.rs
//
// Source: bevy-0.18.1/examples/asset/custom_asset.rs:11 (canonical Bevy 0.18 derive shape)
//         bevy_asset-0.18.1/src/assets.rs:456 (Asset trait bound)
//         ron 0.12 (transitive via Cargo.lock:4192-4195)

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Asset, Reflect, Serialize, Deserialize, Default, Debug, Clone)]
pub struct DungeonFloor {
    // Stub for Feature #3. Feature #4 fills in the razor-wall grid.
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dungeon_floor_round_trips_through_ron() {
        let original = DungeonFloor::default();

        let serialized: String = ron::ser::to_string_pretty(
            &original,
            ron::ser::PrettyConfig::default(),
        ).expect("serialize");

        let parsed: DungeonFloor = ron::de::from_str(&serialized)
            .expect("deserialize");

        let reserialized: String = ron::ser::to_string_pretty(
            &parsed,
            ron::ser::PrettyConfig::default(),
        ).expect("re-serialize");

        assert_eq!(serialized, reserialized,
            "RON round trip lost or reordered fields");
    }
}
```

### `LoadingPlugin` — full skeleton

```rust
// src/plugins/loading/mod.rs
//
// Sources cited inline; see §Question N references.

use bevy::prelude::*;
use bevy_asset_loader::prelude::*;
use bevy_common_assets::ron::RonAssetPlugin;

use crate::data::{ClassTable, DungeonFloor, EnemyDb, ItemDb, SpellTable};
use crate::plugins::state::GameState;

#[derive(AssetCollection, Resource)]
pub struct DungeonAssets {
    #[asset(path = "fonts/FiraSans-Bold.ttf")]
    pub default_font: Handle<Font>,
    #[asset(path = "dungeons/floor_01.dungeon.ron")]
    pub floor_01: Handle<DungeonFloor>,
    #[asset(path = "items/core.items.ron")]
    pub item_db: Handle<ItemDb>,
    #[asset(path = "enemies/core.enemies.ron")]
    pub enemy_db: Handle<EnemyDb>,
    #[asset(path = "classes/core.classes.ron")]
    pub class_table: Handle<ClassTable>,
    #[asset(path = "spells/core.spells.ron")]
    pub spell_table: Handle<SpellTable>,
}

#[derive(Component)]
struct LoadingScreenRoot;

pub struct LoadingPlugin;

impl Plugin for LoadingPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_plugins((
                RonAssetPlugin::<DungeonFloor>::new(&["dungeon.ron"]),
                RonAssetPlugin::<ItemDb>::new(&["items.ron"]),
                RonAssetPlugin::<EnemyDb>::new(&["enemies.ron"]),
                RonAssetPlugin::<ClassTable>::new(&["classes.ron"]),
                RonAssetPlugin::<SpellTable>::new(&["spells.ron"]),
            ))
            .add_loading_state(
                LoadingState::new(GameState::Loading)
                    .continue_to_state(GameState::TitleScreen)
                    .load_collection::<DungeonAssets>(),
            )
            .add_systems(OnEnter(GameState::Loading), spawn_loading_screen)
            .add_systems(OnExit(GameState::Loading), despawn_loading_screen);
    }
}

fn spawn_loading_screen(mut commands: Commands) {
    // A single 2D camera so bevy_ui has something to render to.
    // Tag with LoadingScreenRoot so we can despawn it on OnExit.
    commands.spawn((Camera2d, LoadingScreenRoot));
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            LoadingScreenRoot,
        ))
        .with_children(|parent| {
            parent.spawn(Text::new("Loading..."));
        });
}

fn despawn_loading_screen(
    mut commands: Commands,
    roots: Query<Entity, With<LoadingScreenRoot>>,
) {
    for e in &roots {
        // Bevy 0.18 `despawn` is recursive by default
        // (research §State of the Art line 1234, 2026-03-26 doc).
        commands.entity(e).despawn();
    }
}
```

### Placeholder RON file shape (one example)

```ron
// assets/dungeons/floor_01.dungeon.ron
//
// Stub for Feature #3 — DungeonFloor has no fields yet.
// Feature #4 adds the razor-wall grid; until then this file just verifies
// the typed-RON loader registration works end-to-end.
(
)
```

(For an empty-struct stub, the RON document `()` is valid `DungeonFloor::default()`. When fields land in #4, the RON shape grows accordingly.)

### `assets/README.md` content shape

```markdown
# Druum Asset Layout

Each top-level subfolder corresponds to one typed RON asset family:

| Folder | Extension | Type | Loaded via |
|--------|-----------|------|------------|
| `dungeons/` | `.dungeon.ron` | `crate::data::DungeonFloor` | `bevy_common_assets::RonAssetPlugin::<DungeonFloor>` |
| `items/` | `.items.ron` | `crate::data::ItemDb` | `bevy_common_assets::RonAssetPlugin::<ItemDb>` |
| `enemies/` | `.enemies.ron` | `crate::data::EnemyDb` | `bevy_common_assets::RonAssetPlugin::<EnemyDb>` |
| `classes/` | `.classes.ron` | `crate::data::ClassTable` | `bevy_common_assets::RonAssetPlugin::<ClassTable>` |
| `spells/` | `.spells.ron` | `crate::data::SpellTable` | `bevy_common_assets::RonAssetPlugin::<SpellTable>` |
| `fonts/` | `.ttf` | `Handle<Font>` | Bevy default |

## Hot-reload

When running `cargo run --features dev`, edits to any file under `assets/` trigger a re-load via `bevy/file_watcher`. Edits while in `GameState::Loading` re-poll the asset collection; edits during gameplay are picked up by `AssetEvent<T>::Modified` (a `Message` in 0.18, not an `Event`).

## Adding a new asset family

1. Define the struct in `src/data/<name>.rs` with `#[derive(Asset, Reflect, Serialize, Deserialize, Default, Debug, Clone)]`.
2. Add `RonAssetPlugin::<NewType>::new(&["new.ron"])` to `LoadingPlugin::build`.
3. Add a field to `DungeonAssets` with `#[asset(path = "...")]`.
4. Add at least one placeholder RON file under `assets/<folder>/`.
5. Add a round-trip test in `src/data/<name>.rs`.
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
| ------------ | ---------------- | ------------ | ------ |
| `bevy_asset_ron` | `bevy_common_assets` (multi-format) | 2022-ish; documented in research §State of the Art line 1232 | One crate, multiple formats. Pick the format module per-extension via `bevy_common_assets::ron::RonAssetPlugin` etc. |
| `*Bundle` types in `bevy_ui` (e.g. `TextBundle`) | Component + `#[require(...)]` | Bevy 0.15+ | Spawn `Text::new("...")` directly; required components auto-attach (verified at `bevy_ui-0.18.1/src/widget/text.rs:97-117`). |
| `EventReader<AssetEvent<T>>` | `MessageReader<AssetEvent<T>>` | Bevy 0.18 | The `Event`/`Message` family split — verified at `bevy_asset-0.18.1/src/event.rs:9, 49`. |
| `AssetServer::watch_for_changes()` runtime call | `AssetPlugin { watch_for_changes_override: Some(true), .. }` | Bevy 0.13-ish; current shape verified at `bevy_asset-0.18.1/src/lib.rs:248` | Watch is a startup-time configuration, not a runtime toggle. |
| `add_collection_to_loading_state::<S, T>` standalone helper | `LoadingState::new(...).continue_to_state(...).load_collection::<T>()` chained | `bevy_asset_loader 0.20+` (MEDIUM confidence) | Cleaner builder API; old form remains as compat. |

**Deprecated/outdated patterns to avoid:**

- `bevy_asset_ron`: superseded by `bevy_common_assets`.
- `EventReader<AssetEvent<T>>`: will not compile in 0.18.
- Manual `impl AssetLoader for T` for plain serde types: redundant when `bevy_common_assets` works.
- `commands.entity(e).despawn_recursive()`: in 0.18, `despawn()` is recursive by default (research §State of the Art line 1234).

---

## Validation Architecture

### Test Framework

| Property | Value |
| -------- | ----- |
| Framework | Cargo's built-in `#[test]` (no `cargo-nextest`, no `criterion` for Feature #3) |
| Config file | None — uses default Cargo test discovery |
| Quick run command | `cargo test -p druum data::dungeon::tests::dungeon_floor_round_trips_through_ron` |
| Full default suite | `cargo test` |
| Full dev-features suite | `cargo test --features dev` |

### Requirements → Test Map

| Requirement | Behavior | Test Type | Automated Command | File Exists? |
| ----------- | -------- | --------- | ----------------- | ------------ |
| Feature #3.1 — `bevy_common_assets` and `bevy_asset_loader` compile under our pinned Bevy 0.18.1 | clean compile | n/a (compile check) | `cargo check && cargo check --features dev` | n/a |
| Feature #3.2 — five custom RON extensions register and dispatch correctly | smoke test (manual or compile-only) | manual | `cargo run --features dev` and observe loading screen → title transition | n/a — covered by integration on first run |
| Feature #3.3 — `LoadingPlugin` drives `Loading → TitleScreen` once assets ready | integration | manual | `cargo run --features dev` and observe `GameState -> TitleScreen` log line (Feature #2 logger fires automatically) | n/a — Feature #2 logger does the work |
| Feature #3.4 — `DungeonAssets` collection populates as a `Resource` after load | manual | manual | Add a temporary `OnEnter(GameState::TitleScreen)` system that prints `assets.is_some()`; smoke-tested then removed | n/a — temporary |
| Feature #3.5 — Centered "Loading..." text renders | visual | manual | `cargo run --features dev` shows the text on launch | n/a — visual |
| Feature #3.6 — `DungeonFloor` round-trips RON → struct → RON without diff | unit | `cargo test data::dungeon::tests::dungeon_floor_round_trips_through_ron` | ❌ needs creating |
| Feature #3.7 — Five placeholder RON files load without warnings under `--features dev` | manual | `cargo run --features dev` and inspect log output for "Multiple AssetLoaders found" or "No AssetLoader found" | n/a — manual |
| Feature #3.8 — `assets/README.md` documents the layout | static review | n/a | n/a — review at code-review time | ❌ needs creating |
| Feature #3.9 — Hot-reload picks up changes to a `.dungeon.ron` while running | manual | edit `assets/dungeons/floor_01.dungeon.ron` while `cargo run --features dev` is running; observe `AssetEvent<DungeonFloor>::Modified` in logs (or temporary debug system) | n/a — manual |
| Symmetric verification: `cargo check` | compile (no `dev` feature) | `cargo check` | n/a |
| Symmetric verification: `cargo check --features dev` | compile (dev) | `cargo check --features dev` | n/a |
| Symmetric verification: `cargo clippy --all-targets -- -D warnings` | lint clean | `cargo clippy --all-targets -- -D warnings` | n/a |
| Symmetric verification: `cargo clippy --all-targets --features dev -- -D warnings` | lint clean (dev) | `cargo clippy --all-targets --features dev -- -D warnings` | n/a |
| Symmetric verification: `cargo test` | unit tests pass (no dev) | `cargo test` | n/a |
| Symmetric verification: `cargo test --features dev` | unit tests pass (dev) | `cargo test --features dev` | n/a |

### Gaps (files to create before / during implementation)

- [ ] `src/plugins/loading/mod.rs` — the new plugin
- [ ] `src/data/dungeon.rs` — `DungeonFloor` stub + round-trip test
- [ ] `src/data/items.rs` — `ItemDb` stub
- [ ] `src/data/enemies.rs` — `EnemyDb` stub
- [ ] `src/data/classes.rs` — `ClassTable` stub
- [ ] `src/data/spells.rs` — `SpellTable` stub
- [ ] `src/data/mod.rs` — replace placeholder with `pub mod ...` lines
- [ ] `src/plugins/mod.rs` — add `pub mod loading;` (alphabetical between `dungeon` and `party`)
- [ ] `src/main.rs` — add `LoadingPlugin` to the plugin tuple, configure `AssetPlugin` watch override
- [ ] `Cargo.toml` — add `bevy_common_assets`, `bevy_asset_loader`; extend `dev` feature with `bevy/file_watcher`
- [ ] `assets/dungeons/floor_01.dungeon.ron`
- [ ] `assets/items/core.items.ron`
- [ ] `assets/enemies/core.enemies.ron`
- [ ] `assets/classes/core.classes.ron`
- [ ] `assets/spells/core.spells.ron`
- [ ] `assets/README.md`
- [ ] `assets/fonts/` directory (the `DungeonAssets` collection references a font; either add a TTF placeholder or omit the font field in Feature #3 and add it in Feature #25 polish — recommend the latter to keep this feature ruthlessly small; in that case **drop the `default_font` field from `DungeonAssets`** and rely on Bevy's embedded `default_font` for the loading-screen text, which already works)

### Dev-feature symmetry check (per `project/resources/20260501-102842-dev-feature-pattern.md`)

Feature #3 has no obvious `#[cfg(feature = "dev")]`-gated systems. The only places where dev-feature awareness leaks in are:
- `main.rs`'s `watch_for_changes_override: Some(cfg!(feature = "dev"))` — uses `cfg!` macro, which is always present at compile time regardless of which features are enabled. **This is correct; no symmetric-gating issue.**
- `Cargo.toml`'s `dev = ["bevy/dynamic_linking", "bevy/file_watcher"]` — a feature flag composition, not a `cfg`.

If the implementer ends up adding any debug-print or asset-path-logger system, that system **MUST** be gated symmetrically on both function definition and `add_systems` registration, per the established convention.

---

## Open Questions

### 1. Exact pinned versions of `bevy_common_assets` and `bevy_asset_loader`

- **What we know:** Both crates are by NiklasEi. Both typically ship 0.18-compatible releases within weeks of a Bevy minor. Bevy 0.18 shipped 2026-03-04. Today is 2026-05-01.
- **What's unclear:** The exact patch versions to pin.
- **Recommendation:** Run §Question 1's verification recipe before editing `Cargo.toml`. If either crate lags, escalate per the `moonshine-save` precedent (Decision §Resolved #3, 2026-04-29).

### 2. Whether to include a placeholder font in `DungeonAssets` for Feature #3

- **What we know:** Bevy's `default_font` is enabled transitively via `features = ["3d"]` and supplies an embedded sans-serif. The "Loading..." text renders without a custom font.
- **What's unclear:** Whether the brief's "minimum-viable" intent includes a font in `DungeonAssets` or not.
- **Recommendation:** Drop the font from `DungeonAssets` for Feature #3 — keep this feature ruthlessly small, defer fonts to Feature #25. The `DungeonAssets` collection should still exist (so the `LoadingState` is non-trivial) but populate it only with the five RON handles. **Sub-recommendation for the planner:** mention this explicitly in the plan so the implementer doesn't add a font field "just in case" and then need to author a TTF placeholder.

### 3. Whether `LoadingPlugin` should also despawn the loading-screen `Camera2d` on `OnExit(GameState::Loading)`

- **What we know:** `bevy_ui` needs a Camera2d to render. Once we leave `Loading`, we may want a `Camera3d` for the dungeon view (Feature #7). Two cameras can coexist if both have explicit render targets, but the default behavior overlays them.
- **What's unclear:** Whether the loading-screen camera should persist for later UI screens (Title, Town, Combat menus, Auto-map) or be replaced.
- **Recommendation:** Start with **despawn the camera on `OnExit(GameState::Loading)`**, alongside the text. Each subsequent feature that needs a camera spawns its own. This keeps Feature #3 self-contained and avoids cross-feature coupling. The Title screen feature (lurking inside #25 or wherever it lands) takes the next ownership of a Camera2d.

### 4. Whether the `assets/README.md` belongs in this PR or the next one

- **What we know:** The brief includes it in Feature #3's scope (item #8).
- **What's unclear:** Nothing — it's in scope.
- **Recommendation:** Create it as part of Feature #3. The structure proposed in §"Code Examples" → `assets/README.md` content shape is sufficient for this stage.

### 5. RON-file commenting style for placeholder files

- **What we know:** RON 0.12 supports `// line` and `/* block */` comments per the `ron` crate spec.
- **What's unclear:** Whether the round-trip test will treat comments as significant (it won't — `ron::de::from_str` strips them; `ron::ser::to_string_pretty` does not re-emit them).
- **Recommendation:** Comments are fine in hand-authored RON files but **omit them** from any string used in a round-trip equality test. The Feature #3 round-trip test goes through `Default` values, so this is moot, but flag for #4 when authored RON gets longer.

---

## Constraint Verification (per the brief)

The brief enumerated several constraints from prior decisions. I confirm each below:

| Constraint | Status in Feature #3 | Note |
| ---------- | -------------------- | ---- |
| **No `rand`** (permadeath/deterministic RNG decision; Feature #23 owns `RngSeed`) | ✅ — no rand needed | Loader has no obvious RNG need. **`bevy_asset_loader` has no "random asset selection" feature surfaced in its documented API**; it loads what you list, in whatever order Bevy schedules. If the implementer ever sees a `random` or `rand` import sneaking into `LoadingPlugin` or `DungeonAssets`, it's a bug — trace it. |
| **No `leafwing-input-manager`** (Feature #5 owns input) | ✅ — no input needed | `LoadingPlugin` has no keyboard input. The loading screen is a passive UI; nothing to press. If a future iteration wants a "press any key to continue", do it through `Res<ButtonInput<KeyCode>>` for the placeholder, then port to leafwing in Feature #5. |
| **`GameState::Loading` is the default and nothing else auto-transitions to it** (Feature #2 contract) | ✅ — `LoadingPlugin` only owns the exit (`Loading → TitleScreen`); it does not (and must not) issue `next.set(GameState::Loading)` from anywhere. | Feature #23 (save/load) may RE-enter `Loading` to restore — that's allowed. |
| **`#[cfg(feature = "dev")]` cfg-gating** for any dev-only system | ✅ — Feature #3 has no dev-only systems by default | If the implementer adds an asset-path logger or a debug overlay, cfg-gate **both** the function and the `add_systems` registration. Hot-reload's `bevy/file_watcher` is gated via the `dev` cargo feature *composition*, not via `cfg`, so the symmetric-gating rule does not apply to that one. |
| **Symmetric verification across all 5 commands**: `cargo check`, `cargo clippy --all-targets -- -D warnings`, `cargo clippy --all-targets --features dev -- -D warnings`, `cargo test`, `cargo test --features dev` | ✅ — the §Validation Architecture section enumerates all five | Feature #3 should pass all five with zero warnings. The round-trip test runs in both `cargo test` and `cargo test --features dev` (no cfg gate on it). |
| **Bevy 0.18 zstd rename** (lesson from #1: `zstd` → `zstd_rust`/`zstd_c`) | ✅ — already correct in current `Cargo.toml` (line 14: `"zstd_rust"`) | Feature #3 does not touch zstd. No risk. |
| **Bevy 0.18.1 requires rustc 1.89** (lesson from #1) | ✅ — already pinned in `Cargo.toml` (line 5: `rust-version = "1.89"`) and in `rust-toolchain.toml` (verified by Feature #1 plan) | Feature #3 does not change the toolchain. |
| **`keyboard_input_system` clears `just_pressed` in `PreUpdate`** (lesson from #2) | n/a — Feature #3 has no keyboard input tests | If a future feature adds an input-driven test, follow the `init_resource::<ButtonInput<KeyCode>>` pattern from `feedback_bevy_test_input_setup.md`. |

---

## Sources

### Primary (HIGH confidence — direct on-disk verification)

- [Bevy 0.18.1 source on disk](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/) — every cited line and file path is verifiable here; full crate family extracted (`bevy-0.18.1`, `bevy_asset-0.18.1`, `bevy_ui-0.18.1`, `bevy_text-0.18.1`, `bevy_state-0.18.1`, `bevy_internal-0.18.1`, `bevy_reflect-0.18.1`, `bevy_asset_macros-0.18.1`, `bevy_reflect_derive-0.18.1`).
- `bevy_asset-0.18.1/src/lib.rs:237-345` — `AssetPlugin` struct, fields, defaults.
- `bevy_asset-0.18.1/src/lib.rs:355-395` — `Plugin for AssetPlugin` impl, watch flag handling.
- `bevy_asset-0.18.1/src/assets.rs:456` — `Asset` trait bound: `VisitAssetDependencies + TypePath + Send + Sync + 'static`.
- `bevy_asset-0.18.1/src/event.rs:9, 49` — `AssetLoadFailedEvent` and `AssetEvent` are `#[derive(Message, ...)]`.
- `bevy_asset-0.18.1/src/server/mod.rs:189-192, 1238-1252` — `watching_for_changes`, `is_loaded`, `is_loaded_with_dependencies`.
- `bevy_asset-0.18.1/src/server/loaders.rs:273-289` — `get_by_extension` and `get_by_path` matcher logic.
- `bevy_asset-0.18.1/src/path.rs:464-490` — `AssetPath::get_full_extension` semantics.
- `bevy_asset_macros-0.18.1/src/lib.rs:17-33` — `Asset` derive macro emits `impl Asset for T {}` plus `VisitAssetDependencies` impl.
- `bevy_reflect_derive-0.18.1/src/lib.rs:822, 487-494` — `TypePath` derive provided by `Reflect`.
- `bevy_ui-0.18.1/src/widget/text.rs:97-117` — `Text` component + `#[require(...)]` attribute list.
- `bevy_ui-0.18.1/src/ui_node.rs:500, 608, 649` — `Node`, `JustifyContent`, `AlignItems`.
- `bevy-0.18.1/Cargo.toml:2322-2330` (umbrella `"3d"` includes `"ui"`), `:2570-2589` (`"ui"` → `bevy_ui` + `bevy_ui_render`), `:2454-2466` (`default_platform` includes `default_font` and `x11`/`wayland`), `:2487` (`file_watcher = ["bevy_internal/file_watcher"]`).
- `bevy_internal-0.18.1/Cargo.toml:235` — `file_watcher = ["bevy_asset?/file_watcher"]`.
- `bevy_asset-0.18.1/Cargo.toml:40-44, 166-168` — `file_watcher` feature deps; `ron 0.12` direct dep.
- `bevy-0.18.1/examples/asset/custom_asset.rs` — canonical Bevy 0.18 custom-asset shape (`#[derive(Asset, TypePath, Debug, Deserialize)]`).
- `bevy-0.18.1/examples/asset/hot_asset_reloading.rs` — Bevy's own hot-reload example (notes the `file_watcher` cargo feature requirement).
- `Cargo.lock:4192-4195, 4326-4329` — `ron 0.12.1` and `serde 1.0.228` already present transitively.
- `Cargo.toml` — current Druum `[features]` and Bevy feature list to verify against.
- `src/plugins/state/mod.rs` — current `GameState` definition and conventions.
- `src/main.rs` — current `add_plugins` tuple shape.

### Secondary (MEDIUM confidence — prior research artifacts)

- [research/20260326-01-bevy-first-person-dungeon-crawler-rpg.md](research/20260326-01-bevy-first-person-dungeon-crawler-rpg.md) — original 2026-03-26 doc; cited for §State of the Art deprecations and §Don't Hand-Roll alignment. Accessed: 2026-05-01.
- [project/research/20260429-021500-bevy-0-18-1-skeleton-init.md](project/research/20260429-021500-bevy-0-18-1-skeleton-init.md) — Feature #1 research; cited for `dev` feature pattern. Accessed: 2026-05-01.
- [project/research/20260429-030000-bevy-0-18-1-state-machine-feature-2.md](project/research/20260429-030000-bevy-0-18-1-state-machine-feature-2.md) — Feature #2 research; cited for `Event`/`Message` rename precedent. Accessed: 2026-05-01.
- [project/resources/20260501-102842-dev-feature-pattern.md](project/resources/20260501-102842-dev-feature-pattern.md) — `dev` feature flag and cfg-gating discipline. Accessed: 2026-05-01.
- [project/resources/20260501-104450-bevy-state-machine-anatomy.md](project/resources/20260501-104450-bevy-state-machine-anatomy.md) — current state-machine layout that Feature #3 must respect. Accessed: 2026-05-01.
- [project/roadmaps/20260429-01-bevy-dungeon-crawler-roadmap.md](project/roadmaps/20260429-01-bevy-dungeon-crawler-roadmap.md) — §Feature #3 entry, design decision §Resolved #3 (`moonshine-save` precedent for crate-version-lag escalation). Accessed: 2026-05-01.
- [`.claude/agent-memory/planner/project_druum_state_machine.md`](.claude/agent-memory/planner/project_druum_state_machine.md) — durable constraint that Feature #3 owns the `Loading → TitleScreen` transition. Accessed: 2026-05-01.

### Tertiary (LOW confidence — training-data inferences, marked for verification)

- [bevy_common_assets repo](https://github.com/NiklasEi/bevy_common_assets) — README pattern for `RonAssetPlugin::new(&[ext])`. Verify against the resolved version after `cargo add`. Accessed: not in this session.
- [bevy_asset_loader repo](https://github.com/NiklasEi/bevy_asset_loader) — README pattern for `LoadingState::new(...).continue_to_state(...).load_collection::<T>()`. Verify against the resolved version. Accessed: not in this session.
- [crates.io: bevy_common_assets](https://crates.io/crates/bevy_common_assets) — for version verification. Not accessible from this session.
- [crates.io: bevy_asset_loader](https://crates.io/crates/bevy_asset_loader) — for version verification. Not accessible from this session.
- [docs.rs: bevy_common_assets](https://docs.rs/bevy_common_assets) — for API verification post-pin.
- [docs.rs: bevy_asset_loader](https://docs.rs/bevy_asset_loader) — for API verification post-pin.

---

## Metadata

**Confidence breakdown:**

- Bevy 0.18.1 first-party APIs (asset, ui, text, state): **HIGH** — verified against extracted 0.18.1 source on disk with line numbers.
- `bevy_common_assets` and `bevy_asset_loader` integration: **MEDIUM** — API shapes inferred from training data and convention; exact version pins blocked on a one-time crates.io verification recipe.
- Hot-reload mechanics: **HIGH** — `AssetPlugin::watch_for_changes_override` and `bevy/file_watcher` feature both verified in 0.18.1 source.
- RON round-trip test: **HIGH** — `ron::de::from_str` / `ron::ser::to_string_pretty` is a stable API since `ron 0.7`+; current `ron 0.12.1` continues this; transitively present.
- Loading-screen UI: **HIGH** — `Text` + `#[require(...)]` verified in `bevy_ui-0.18.1` source.
- `LoadingState ↔ GameState` integration: **MEDIUM-HIGH** — no sub-state wrapper needed (HIGH); exact builder method names (`load_collection` vs. `add_collection_to_loading_state`) need post-pin verification.
- Pitfalls: **HIGH** — each pitfall cites a Bevy 0.18.1 file and line.

**Research date:** 2026-05-01

**Author:** researcher agent
