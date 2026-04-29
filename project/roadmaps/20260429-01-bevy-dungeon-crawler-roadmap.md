# Druum (Bevy First-Person Dungeon Crawler RPG) — Feature Roadmap

This roadmap turns the 2026-03-26 research document (`research/20260326-01-bevy-first-person-dungeon-crawler-rpg.md`) into a sequenced, feature-by-feature plan for building a Wizardry / Etrian Odyssey-style dungeon crawler in Bevy 0.18.1. The project is greenfield: no source exists yet, and the first features are pure foundation work. The roadmap follows the research's primary recommendation — Option B rendering (3D dungeon geometry + billboard enemy sprites), plugin-per-subsystem architecture, `bevy_egui` for menus, RON-serialized dungeons with razor walls — and orders features so each one stands on a verified base. The genre is content-bound rather than performance-bound, so the roadmap weights difficulty toward UI, data pipelines, and balance work, not toward rendering optimization.

---

## Difficulty Scale

| Rating | Meaning |
|--------|---------|
| 1/5    | Trivial — a few hours, minimal risk |
| 2/5    | Easy — a focused day or two |
| 3/5    | Moderate — multi-day, some coordination between layers/systems |
| 4/5    | Hard — significant effort, architectural decisions required |
| 5/5    | Very Hard — major undertaking, weeks of work |

Half-steps (1.5, 2.5, etc.) are used when a feature sits between two levels.

---

## Current Baseline

This is a greenfield Rust/Cargo project. The repository contains only the research document and `.claude/` config — no `Cargo.toml`, no `src/`, no assets. All numbers below are starting-point reference values; "deltas" in feature impact tables are *additions to this baseline*.

| Metric | Baseline | Source |
|--------|----------|--------|
| Bevy version | 0.18.1 (released 2026-03-04) | Research §Standard Stack |
| Source LOC (Rust) | 0 | greenfield |
| Crate dependency count | 0 | no Cargo.toml |
| Compile time (clean dev) | n/a | will be 30-90s once Bevy is added |
| Compile time (incremental) | n/a | target: 2-8s with `dynamic_linking` + `opt-level=2` for deps |
| Binary size (release) | n/a | target ~15-20 MB executable, 30-60 MB bundle |
| ECS active entities (target gameplay) | n/a | < 1000 typical for this genre |
| Test count | 0 | no tests yet |
| Asset count | 0 | no `assets/` directory |
| UI screens implemented | 0 | DRPGs typically need 15+ |
| Dungeon floors authored | 0 | target ~20 for a full game |
| Classes / races / spells / items | 0 / 0 / 0 / 0 | starter target: 3 classes (Fighter / Mage / Priest) — decision §Resolved #1 |

**Baseline-driven impact metrics used throughout this roadmap:**
- **LOC Δ** — rough Rust source lines added (estimates, ranges)
- **Deps Δ** — new direct crates pulled into `Cargo.toml`
- **Compile Δ** — likely impact on incremental rebuild time
- **Asset Δ** — net new asset files (textures, RON, audio, fonts)

---

## Design Decisions Resolved (2026-04-29)

The following five open questions raised by the roadmapper were closed by the user on 2026-04-29. They are durable design calls; subsequent features must respect them.

| # | Question | Decision | Implications |
|---|----------|----------|--------------|
| 1 | Starter class roster | **3 classes: Fighter / Mage / Priest** | #19 (character creation) ships with 3 classes; the remaining 5 from research are deferred to a post-v1 milestone, not cut. |
| 2 | Encounter philosophy | **Both random encounters AND FOEs** | #16 (encounter system) handles random battles; #22 (FOEs) lands as a separate feature with visible grid-walking enemies. Both must coexist on a floor. |
| 3 | Save/load crate | **`moonshine-save` (v0.6.1, 2026-01-22)** | Chosen because it's the only candidate with native Bevy 0.18 support: `bevy_save 2.0.1` requires `bevy ^0.16.1` (two minors behind); custom would be ~200–400 LOC of yak-shaving. moonshine-save round-trips through `DynamicScene`, giving atomic writes — important now that permadeath is in scope. #23 should pin `=0.6.1` or compatible. |
| 4 | Spinner UX | **Modern telegraphed (Etrian style)** | Spinner tiles get a visible icon on the auto-map, a sound effect, and a brief screen wobble. The auto-map (#10) renders compass-true direction (not party-relative). Dungeon design culture across all 20 floors leans cerebral, not cruel. |
| 5 | Permadeath / Iron Mode | **In scope** | #23 (save/load) must seed RNG deterministically *from the start*, not bolt it on later. #15 (combat) has no in-combat saves; Iron Mode autosaves only on safe checkpoints (floor transitions, town entry). #19 (character creation) supports re-rolls after a TPK. Affects every subsequent gameplay feature's save semantics. |

---

## Table of Contents

1. [Project Skeleton & Plugin Architecture](#1-project-skeleton--plugin-architecture) — 1.5/5
2. [Game State Machine](#2-game-state-machine) — 1.5/5 *(depends on #1)*
3. [Asset Pipeline & RON Loading](#3-asset-pipeline--ron-loading) — 2/5 *(depends on #1)*
4. [Dungeon Grid Data Model](#4-dungeon-grid-data-model) — 2/5 *(depends on #3)*
5. [Input System (leafwing)](#5-input-system-leafwing) — 2/5 *(depends on #1)*
6. [Audio System (BGM + SFX)](#6-audio-system-bgm--sfx) — 2/5 *(depends on #1)*
7. [Grid Movement & First-Person Camera](#7-grid-movement--first-person-camera) — 2.5/5 *(depends on #4, #5)*
8. [3D Dungeon Renderer (Option B)](#8-3d-dungeon-renderer-option-b) — 3/5 *(depends on #4, #7)*
9. [Dungeon Lighting & Atmosphere](#9-dungeon-lighting--atmosphere) — 2/5 *(depends on #8)*
10. [Auto-Map / Minimap](#10-auto-map--minimap) — 2.5/5 *(depends on #4, #7)*
11. [Party & Character ECS Model](#11-party--character-ecs-model) — 3/5 *(depends on #1, #3)*
12. [Inventory & Equipment](#12-inventory--equipment) — 2.5/5 *(depends on #11)*
13. [Cell Features (Doors, Traps, Teleporters, Spinners)](#13-cell-features-doors-traps-teleporters-spinners) — 3/5 *(depends on #4, #7, #8)*
14. [Status Effects System](#14-status-effects-system) — 2.5/5 *(depends on #11)*
15. [Turn-Based Combat Core](#15-turn-based-combat-core) — 4/5 *(depends on #2, #11, #14)*
16. [Encounter System & Random Battles](#16-encounter-system--random-battles) — 2.5/5 *(depends on #4, #15)*
17. [Enemy Billboard Sprite Rendering](#17-enemy-billboard-sprite-rendering) — 3/5 *(depends on #15, #8)*
18. [Town Hub & Services](#18-town-hub--services) — 3/5 *(depends on #2, #11, #12)*
19. [Character Creation & Class Progression](#19-character-creation--class-progression) — 3/5 *(depends on #11, #18)*
20. [Spells & Skill Trees](#20-spells--skill-trees) — 3.5/5 *(depends on #15, #19)*
21. [Loot Tables & Economy](#21-loot-tables--economy) — 3/5 *(depends on #12, #15, #18)*
22. [FOE / Visible Enemies](#22-foe--visible-enemies) — 3.5/5 *(depends on #16)*
23. [Save / Load System](#23-save--load-system) — 3.5/5 *(touches almost everything)*
24. [Dungeon Editor Tool](#24-dungeon-editor-tool) — 4/5 *(depends on #4, #13)*
25. [Title Screen, Settings & End-to-End Polish](#25-title-screen-settings--end-to-end-polish) — 3/5 *(depends on most others)*

---

## 1. Project Skeleton & Plugin Architecture

### Difficulty Rating
**1.5/5** — small in scope but sets architectural conventions that propagate through every later feature.

### Overview
Initialize the Cargo project, pin dependencies, configure dev profiles for fast iteration, and lay down the empty plugin module tree (`dungeon`, `combat`, `party`, `town`, `ui`, `audio`, `save`). Each plugin is a `bevy::Plugin` impl that registers its own systems and resources.

### Pros
- Forces a clean separation of concerns from line one — refactoring later is far more expensive.
- Pinning Bevy `=0.18.1` (research §Pitfall 1) prevents an accidental `cargo update` from triggering a multi-week migration.
- Per-package `opt-level = 2` and `dynamic_linking` feature take the incremental rebuild from ~30s to ~5-8s and pay back hundreds of times over.
- The `app.add_plugins((...))` tuple trivially scales as features land.

### Cons
- The plugin layout in the research is a guess at the right shape. Some splits (e.g. `audio` as its own plugin vs. folding into `combat`/`town`) may turn out to be over-engineered for a solo project.
- Pinning Bevy means you miss bug fixes that ship in 0.18.2/0.18.3 unless you opt in deliberately.
- `dynamic_linking` cannot be used in release builds and complicates CI; you have to remember to disable it for shipping artifacts.

### What This Touches
- Brand-new `Cargo.toml`, `Cargo.lock`, `.gitignore`, `rust-toolchain.toml`.
- `src/main.rs`, `src/lib.rs` and the `src/plugins/{dungeon,combat,party,town,ui,audio,save}/mod.rs` skeleton.
- `[profile.dev.package."*"]` / `[features] dev = ["bevy/dynamic_linking"]` in `Cargo.toml`.
- A bare `assets/` folder and a placeholder `README.md`.

### Impact Analysis

| Dimension | Baseline | After |
|-----------|----------|-------|
| LOC Δ | 0 | +150 to +250 (mostly module declarations and `Plugin` impls) |
| Deps Δ | 0 | +1 (`bevy = "=0.18.1"`) |
| Compile Δ | n/a | First clean build 30-90s; incremental 5-8s with dynamic linking |
| Asset Δ | 0 | 0 |
| Feature flags | 0 | +1 (`dev`) |

Note: This first build is the slowest the project will ever feel. Investing in `opt-level=2` for deps and dynamic linking now is the difference between an enjoyable feedback loop and a frustrating one.

### Broad Todo List

**Cargo / project setup**
- Run `cargo init druum` and add `bevy = { version = "=0.18.1", default-features = false, features = ["3d", "bevy_winit", "x11", "bevy_pbr", "bevy_text", "bevy_asset", "png", "ktx2", "zstd"] }` (cherry-pick features rather than default).
- Add `[profile.dev.package."*"] opt-level = 2` and `[profile.dev] opt-level = 1`.
- Add `[features] dev = ["bevy/dynamic_linking"]` and document `cargo run --features dev`.
- Add `.gitignore` (target/, *.rs.bk, .DS_Store).
- Pin a `rust-toolchain.toml` to a specific stable channel (e.g. 1.85+) to keep CI reproducible.

**Plugin skeleton**
- Create empty `Plugin` impls for `DungeonPlugin`, `CombatPlugin`, `PartyPlugin`, `TownPlugin`, `UiPlugin`, `AudioPlugin`, `SavePlugin`.
- Wire them all into `main.rs` via `App::new().add_plugins((DefaultPlugins, DungeonPlugin, ...))`.
- Add an empty `data/` module for static tables (enemies, items, spells, classes).

**Tooling**
- Add `cargo check` and `cargo clippy -- -D warnings` to a basic `justfile` or `Makefile` target.
- Confirm `cargo run --features dev` opens a black window and exits cleanly on Cmd+Q.

### Additional Notes
Resist the urge to also wire up egui, audio, and input here. Each is a separate feature (#5, #6, plus UI shell as part of #18 and beyond). Keeping this feature ruthlessly small protects the rebuild time bisection — you want the first failing build to fail fast and obvious.

---

## 2. Game State Machine

### Difficulty Rating
**1.5/5** — mostly type-level work, but it shapes every later feature's `run_if` conditions.

### Overview
Define the `GameState` top-level state (`Loading`, `TitleScreen`, `Town`, `Dungeon`, `Combat`, `GameOver`) and the `SubStates` for `DungeonSubState` (`Exploring`, `Inventory`, `Map`, `Paused`, `EventDialog`), `CombatPhase` (`PlayerInput`, `ExecuteActions`, `EnemyTurn`, `TurnResult`), and `TownLocation` (`Square`, `Shop`, `Inn`, `Temple`, `Guild`). Add `OnEnter` / `OnExit` placeholder systems that just log state transitions for now.

### Pros
- Bevy's `States` / `SubStates` API does the run-condition wiring for free; no hand-rolled state machine needed.
- Encodes the genre flow exactly once, in types — every later system just reads `run_if(in_state(...))`.
- `SubStates` automatically deactivate when their parent leaves, so `DungeonSubState::Inventory` cannot persist after entering `GameState::Combat`.

### Cons
- Adds indirection for very early features that don't yet need it (the loading splash could ignore states entirely).
- Bevy's state transitions are deferred by one frame; subtle bugs appear when a system expects `NextState::set` to take effect immediately.
- Refactoring states later (e.g. splitting `Combat` into `RandomBattle` vs `BossBattle`) ripples to every `run_if` clause.

### What This Touches
- New `src/state.rs` (or inside `plugins/mod.rs`) defining the `GameState` and `SubState` enums.
- `App::init_state::<GameState>()` and three `add_sub_state::<...>()` calls in `main.rs` or a `StatePlugin`.
- Each existing plugin gets a stub `OnEnter(GameState::X)` / `OnExit(GameState::X)` system.

### Impact Analysis

| Dimension | Baseline | After |
|-----------|----------|-------|
| LOC Δ | ~200 | +80 to +120 |
| Deps Δ | 1 | 0 (uses Bevy built-ins) |
| Compile Δ | minor | negligible |
| Asset Δ | 0 | 0 |

### Broad Todo List

- Define `enum GameState { Loading, TitleScreen, Town, Dungeon, Combat, GameOver }` with `#[derive(States, Default, ...)]`.
- Define `DungeonSubState`, `CombatPhase`, `TownLocation` with `#[derive(SubStates, ...)]` and `#[source(GameState = ...)]`.
- Register all four with `App::init_state` / `add_sub_state` in a `StatePlugin`.
- Add a debug system that logs `Changed<State<GameState>>` transitions to the console.
- Add `OnEnter` / `OnExit` placeholders in `DungeonPlugin`, `CombatPlugin`, `TownPlugin` that just print their entry.
- Add a hotkey (e.g. F9) bound to `NextState<GameState>` cycling for early manual testing.

### Additional Notes
Keep `Loading` as the default and don't overload it — its only job is to gate gameplay until assets + saves are ready. Later, #3 (asset pipeline) and #23 (save/load) will hook into it.

---

## 3. Asset Pipeline & RON Loading

### Difficulty Rating
**2/5** — a half-day to wire up `bevy_common_assets` + `bevy_asset_loader`, plus learning their conventions.

### Overview
Add `bevy_common_assets` (custom file extensions → typed `Asset`) and `bevy_asset_loader` (declarative collection loading + loading state). Define the asset types used everywhere downstream: dungeon RON, item DB RON, enemy DB RON, class table RON, spell table RON. Show a placeholder loading screen during the `GameState::Loading` state, advance to `TitleScreen` on completion.

### Pros
- One dependency adds RON/JSON/TOML asset loaders for every later data file — no per-format boilerplate.
- `bevy_asset_loader` collections give a single `Res<DungeonAssets>` type instead of dozens of loose `Handle<...>` resources.
- Hot-reloading dungeon RON during development is a 1-line opt-in (`AssetServer::watch_for_changes`) — research §Pitfall 3 calls this out as essential.

### Cons
- Adds two crates whose Bevy 0.18 compat needs verifying on `crates.io` before pinning (research lists "latest" rather than concrete versions).
- `bevy_asset_loader` macros produce hard-to-read errors when an asset path is wrong; new contributors hit the wall.
- Loading state UI is "free" but ugly; you will replace it during #25.

### What This Touches
- `Cargo.toml` (add `bevy_common_assets`, `bevy_asset_loader`).
- New `src/assets.rs` defining typed RON loader registrations.
- New `src/plugins/loading.rs` with `LoadingPlugin` driving the `GameState::Loading → TitleScreen` transition.
- `assets/` subfolders: `dungeons/`, `enemies/`, `items/`, `spells/`, `classes/`, `textures/`, `fonts/`.

### Impact Analysis

| Dimension | Baseline | After |
|-----------|----------|-------|
| LOC Δ | ~280 | +150 to +250 |
| Deps Δ | 1 | +2 (`bevy_common_assets`, `bevy_asset_loader`) |
| Compile Δ | 5-8s incremental | +1-2s clean |
| Asset Δ | 0 | +5 placeholder RON files for smoke testing |

### Broad Todo List

- Verify `bevy_common_assets` and `bevy_asset_loader` versions on crates.io target Bevy 0.18 — pin exact versions.
- Add `.dungeon.ron`, `.items.ron`, `.enemies.ron` etc. as registered RON asset extensions.
- Define `LoadingPlugin` with an `AssetCollection` covering all initial assets.
- Implement loading-state UI fallback: a centered "Loading..." `Text` (bevy_ui) is fine; egui comes later.
- Add `cargo test` for at least one round-trip (`DungeonFloor` → RON → `DungeonFloor`) using `ron::from_str`.
- Document the `assets/` directory layout in a top-level `assets/README.md` so contributors don't sprinkle files.

### Additional Notes
Hot-reload caused issues in older Bevy — confirm in 0.18 that mutating an in-flight `.dungeon.ron` while exploring re-spawns geometry without crashing. The team's iteration speed depends on this working.

---

## 4. Dungeon Grid Data Model

### Difficulty Rating
**2/5** — pure data + unit-testable logic, no Bevy ECS surface area.

### Overview
Implement the razor-wall grid types from research Pattern 2: `WallType`, `WallMask`, `CellFeatures`, `TrapType`, `TeleportTarget`, `Direction`, `DungeonFloor`. Add `DungeonFloor::can_move()` and `Direction` rotation/offset helpers. Author one hand-built test floor (`floor_01.dungeon.ron`) and verify it round-trips through serde.

### Pros
- Zero Bevy or rendering surface area — the entire model can be unit-tested with stdlib Rust.
- Razor walls (research §Wall Representation) are the canonical Wizardry-style format and the right call for the genre.
- A small, well-tested grid library de-risks every later feature that consumes it (#7, #8, #10, #13, #16, #22, #24).

### Cons
- Razor walls don't naturally extend to multi-cell features (e.g. a 2x2 boss arena); modeling these later requires an additional layer.
- Bitmask-on-each-side means a wall between cells (4,5) and (5,5) is stored twice (east of (4,5) and west of (5,5)) — they must be kept in sync, easy to corrupt by hand.
- Forces a (y,x) row-major addressing convention that confuses users used to (x,y).

### What This Touches
- New `src/plugins/dungeon/grid.rs` with all data types + `#[cfg(test)] mod tests`.
- `assets/dungeons/floor_01.dungeon.ron` as a working sample.
- Tests in `cargo test plugins::dungeon::grid::tests`.

### Impact Analysis

| Dimension | Baseline | After |
|-----------|----------|-------|
| LOC Δ | ~430 | +250 to +400 (incl. tests) |
| Deps Δ | 3 | 0 |
| Compile Δ | 5-8s | +0.2s |
| Asset Δ | +5 | +1 (`floor_01.dungeon.ron`) |
| Test count | 0 | +6-10 |

### Broad Todo List

- Implement `Direction::{turn_left, turn_right, reverse, offset}` per research Pattern 2.
- Implement `WallType`, `WallMask`, `CellFeatures`, `TrapType`, `TeleportTarget`.
- Implement `DungeonFloor` with `Asset` + `TypePath` derives so it loads via `bevy_common_assets`.
- Implement `DungeonFloor::can_move(x, y, dir) -> bool` honoring `Open` / `Illusory` as walkable, all others blocked.
- Add a helper `DungeonFloor::wall_between(a, b) -> WallType` that resolves the cell-pair contradiction (pick north/west cell as canonical).
- Author `floor_01.dungeon.ron` (a small 6x6 test floor with a couple of doors and a trap).
- Unit tests: out-of-bounds, walking into solid wall, walking through Open, illusory wall, all four direction rotations.

### Additional Notes
Research §Pitfall 3 (no editor) bites here: every floor authored before #24 is hand-written RON. Keeping the cell schema as flat as possible reduces hand-editing pain.

---

## 5. Input System (leafwing)

### Difficulty Rating
**2/5** — adopting `leafwing-input-manager` is straightforward but its action enum + binding maps are upfront design work.

### Overview
Add `leafwing-input-manager 0.18.x` and define `enum DungeonAction { Forward, Backward, StrafeLeft, StrafeRight, TurnLeft, TurnRight, Interact, OpenMap, OpenInventory, Pause }` and `enum CombatAction { Confirm, Cancel, Up, Down, Left, Right }`. Provide default keyboard + gamepad bindings. Wire input plugins into `DungeonPlugin` and `CombatPlugin` only when their respective state is active.

### Pros
- Many-to-many binding (research §Don't Hand-Roll) means a single action can fire from keyboard, gamepad, or mouse without per-input branching.
- Bindings are serializable, so a "rebind keys" settings screen later is mostly UI work, not engine work.
- State-scoped input plugins prevent dungeon hotkeys from firing during combat menus.

### Cons
- One more crate that lags behind Bevy upgrades (research §Pitfall 1) — pin precisely.
- Default bindings are a design decision masquerading as defaults; getting them wrong wastes playtest time.
- Gamepad support is real but inconsistent across platforms; expect platform-specific edge cases.

### What This Touches
- `Cargo.toml` (`leafwing-input-manager = "0.18"`).
- New `src/input.rs` defining all action enums and default bindings.
- `App::add_plugins(InputManagerPlugin::<DungeonAction>::default().run_if(in_state(GameState::Dungeon)))` and the equivalent for combat.

### Impact Analysis

| Dimension | Baseline | After |
|-----------|----------|-------|
| LOC Δ | ~750 | +120 to +200 |
| Deps Δ | 3 | +1 |
| Compile Δ | small | +0.5-1s clean |
| Asset Δ | +6 | 0 (bindings can stay in code until settings screen ships) |

### Broad Todo List

- Add `leafwing-input-manager` to `Cargo.toml` pinned to a 0.18-compatible release.
- Define `DungeonAction`, `CombatAction`, and a top-level `MetaAction` (escape, screenshot, etc.).
- Author default keyboard bindings (WASD + QE for turning, M for map, Tab for inventory, Esc for pause).
- Author default gamepad bindings (D-pad + face buttons).
- Wire one smoke test: pressing W in dungeon state logs "forward".
- Document the binding map and reserved keys in `assets/README.md`.

### Additional Notes
Defer the actual rebinding UI to #25. Encoding bindings as data (RON) from day one means later UI work is just editing a `Res<InputMap<DungeonAction>>`.

---

## 6. Audio System (BGM + SFX)

### Difficulty Rating
**2/5** — `bevy_kira_audio` is well-documented; the work is in channel design and asset acquisition, not Rust.

### Overview
Add `bevy_kira_audio 0.25.x` and define audio channels: `Bgm`, `Sfx`, `Ui`, `Ambient`. Implement helper systems for BGM crossfades on state transitions (e.g. `OnEnter(GameState::Town)` plays town BGM, fading out the previous track). Provide a placeholder royalty-free track + 4-5 SFX (footstep, door, encounter sting, menu click, attack hit).

### Pros
- `bevy_kira_audio` (research §Standard Stack) gives crossfading and per-channel volume out of the box; built-in `bevy_audio` would force a custom audio state machine.
- Channel design front-loads decisions about UI vs. ambient vs. music so individual systems just push to the right channel.
- Audio is one of the cheapest features to add atmosphere — a single ambient drone makes the dungeon feel "real".

### Cons
- Royalty-free audio is rarely a perfect fit; expect to swap placeholder tracks more than once.
- Crossfade timing is a content tuning problem (too fast feels jarring, too slow feels mushy) — every tester will have an opinion.
- `kira` and `bevy_kira_audio` releases lag Bevy slightly; verify exact version compat.

### What This Touches
- `Cargo.toml` (`bevy_kira_audio = "0.25"`).
- New `src/plugins/audio/{mod.rs, bgm.rs, sfx.rs}`.
- `assets/audio/{bgm,sfx,ambient,ui}/` directories.

### Impact Analysis

| Dimension | Baseline | After |
|-----------|----------|-------|
| LOC Δ | ~870 | +200 to +350 |
| Deps Δ | 4 | +1 |
| Compile Δ | small | +1-2s clean |
| Asset Δ | +6 | +5-8 placeholder audio files (~5-15 MB) |
| Bundle size | n/a | +3-10 MB |

Note: Asset size is a real bundle cost; plan to compress to OGG/Opus rather than ship raw WAV.

### Broad Todo List

- Add `bevy_kira_audio` and pin to a 0.18-compatible release.
- Define `AudioChannel` markers (`Bgm`, `Sfx`, `Ui`, `Ambient`).
- Implement `play_bgm(track_handle)` with a configurable crossfade duration.
- Hook `OnEnter(GameState::Town)` and `OnEnter(GameState::Dungeon)` to play their default BGM.
- Implement an `Sfx` event (`SfxEvent::PlayOneShot(handle)`) that any system can fire.
- Acquire and check in 5 placeholder royalty-free tracks (CC0 from FreePD/Pixabay) — never check in licensed audio.
- Add a master-volume + per-channel-volume `Resource` ahead of the settings UI in #25.

### Additional Notes
Pre-allocate channel handles at startup in a `Resource` so no system has to look them up. SFX latency is sensitive to dropped audio frames during a heavy ECS update; if you see audio glitches, profile the audio sub-app first.

---

## 7. Grid Movement & First-Person Camera

### Difficulty Rating
**2.5/5** — straightforward state + interpolation, but the interaction with input queueing and animation timing is where bugs live.

### Overview
Implement `PlayerParty`, `GridPosition`, `Facing` components plus `MovementAnimation` and the smooth-step interpolation system from research Pattern 4. Tie input from #5 to grid moves: on `DungeonAction::Forward`, check `DungeonFloor::can_move`, spawn an animation, advance grid coordinate. Camera is a single child entity at eye height (~1.6m) on the party transform. No input is consumed while `MovementAnimation` is active.

### Pros
- Smooth interpolation is the difference between "feels professional" and "instant teleport feels broken" (research §Anti-Patterns).
- Clean separation: `GridPosition` is the source of truth, `Transform` is the visual — every later system reads `GridPosition`, not `Transform`.
- Strafing, turning, and moving all land back on the canonical grid; rotation drift is impossible.

### Cons
- 250ms move duration is a player-feel tradeoff: too fast feels twitchy, too slow feels sluggish; you will retune.
- Input queueing (do you accept the next move while still animating?) is an ongoing UX decision — the research doesn't pick one.
- Frame-rate-dependent animation can drift slightly across machines unless you use the `Time` resource consistently (which Bevy does, but new systems forget).

### What This Touches
- `src/plugins/dungeon/movement.rs` with all movement systems.
- `Camera3dBundle` spawned as a child of `PlayerParty` in `OnEnter(GameState::Dungeon)`.
- `run_if(in_state(DungeonSubState::Exploring))` clauses on input handlers.
- `assets/dungeons/floor_01.dungeon.ron` consumed for `can_move` checks.

### Impact Analysis

| Dimension | Baseline | After |
|-----------|----------|-------|
| LOC Δ | ~1170 | +250 to +400 |
| Deps Δ | 5 | 0 |
| Compile Δ | minor | +0.2s |
| Asset Δ | +14 | 0 |
| Test count | ~8 | +4-6 (movement integration tests) |

### Broad Todo List

- Implement `GridPosition`, `Facing`, `PlayerParty`, `MovementAnimation` per research Pattern 4.
- Implement `handle_movement_input` system gated on `Without<MovementAnimation>` (input consumed while animating).
- Implement `animate_movement` system using `smoothstep` interpolation.
- Spawn the player entity + child `Camera3d` on `OnEnter(GameState::Dungeon)` at the floor's `entry_point`.
- Despawn dungeon-tagged entities (`DungeonGeometry`) on `OnExit(GameState::Dungeon)`.
- Integration test (Bevy `App::update`-based) that simulates 4 forward presses on an open corridor and asserts grid position advances.
- Decide and document the input-queueing policy (current default: drop input while animating).

### Additional Notes
Resist hooking encounter checks here — that belongs in #16. Movement should emit a `MovedEvent { from, to, facing }` event; any subscriber (encounter, traps, footstep SFX) handles it.

---

## 8. 3D Dungeon Renderer (Option B)

### Difficulty Rating
**3/5** — straightforward Bevy 3D work, but tile-streaming and material reuse have real performance and content-pipeline implications.

### Overview
Implement `generate_dungeon_geometry` from research Pattern 6: walk the loaded `DungeonFloor`, spawn floor + ceiling quads per cell, plus wall segments for solid/secret/door walls. Use a small set of shared `StandardMaterial`s (one per wall texture) rather than per-cell materials. Tag every spawned mesh entity with `DungeonGeometry` for cleanup.

### Pros
- Bevy 0.18's clustered forward renderer (research §Performance) handles thousands of textured quads with no concern.
- Shared materials and meshes mean a 20x20 floor is ~1600 mesh instances but only 3-5 distinct materials — GPU-friendly.
- Option B is the genre-correct visual target (Etrian Odyssey, Wizardry remake) and uses 2D enemy art that a small team can produce.

### Cons
- For a 20-floor dungeon, naively spawning every cell at every floor change is wasteful (most cells are off-camera). A streaming/visibility pass becomes an issue at scale (deferred to a later optimization).
- Wall textures must tile cleanly — sourcing or producing tileable PBR textures takes time.
- Doors, secret walls, illusory walls all need slightly different mesh + material treatments; the if-chain in `generate_dungeon_geometry` grows.

### What This Touches
- `src/plugins/dungeon/renderer.rs` containing geometry generation.
- `assets/textures/{walls, floors, ceilings, doors}/` populated with placeholder PBR maps.
- `OnEnter(GameState::Dungeon)` triggers regeneration; `OnExit` cleans up.
- A `WallTextures` resource storing pre-loaded handles to shared materials.

### Impact Analysis

| Dimension | Baseline | After |
|-----------|----------|-------|
| LOC Δ | ~1500 | +300 to +500 |
| Deps Δ | 5 | 0 |
| Compile Δ | minor | +0.5s |
| Asset Δ | +14 | +6-12 textures (8-30 MB depending on resolution) |
| GPU draw calls (test floor) | 0 | ~500-2000 per floor |

Note: Mesh + material handles must be cached in resources, not re-created per cell. Spawning 1000 unique `StandardMaterial`s collapses the renderer.

### Broad Todo List

- Pre-load floor/ceiling/wall/door materials in `OnEnter` and stash in a `WallTextures` resource.
- Implement `generate_dungeon_geometry` reading `Res<CurrentDungeonFloor>`.
- Implement `spawn_wall_segment` helper for each cardinal direction (rotate by 0/90/180/270).
- Add door variants with a distinct mesh/material; add an `Interactable` marker component for #13.
- Add `DungeonGeometry` cleanup in `OnExit(GameState::Dungeon)`.
- Source/produce 4-6 placeholder PBR textures (CC0 ambient.com or Polyhaven) for stone wall, dirt floor, ceiling.
- Smoke-test rendering by running `cargo run --features dev` and walking around `floor_01.dungeon.ron`.

### Additional Notes
A future optimization is "just spawn the cells within N tiles of the player and stream as you move" — but for a 20x20 floor at <2000 quads, the naive whole-floor spawn is fine. Defer streaming until a real perf problem appears.

---

## 9. Dungeon Lighting & Atmosphere

### Difficulty Rating
**2/5** — Bevy's `DistanceFog` and `PointLight` are well-documented; the work is in tuning, not coding.

### Overview
Add `DistanceFog` to the dungeon camera with exponential falloff for depth, low ambient light (research §Code Examples), and per-cell `PointLight` torches placed via a `light_positions: Vec<(u32, u32)>` field on `DungeonFloor`. Keep `VolumetricFog` reserved for special atmospheric zones (research notes its GPU cost).

### Pros
- Fog + flickering torch light transforms a flat-textured corridor into a "dungeon" — single highest visual ROI per hour spent.
- Cheap on the GPU (`DistanceFog` is essentially free; `PointLight` count stays small).
- Per-cell light data lives in the RON file, so designers control mood without code changes.

### Cons
- Shadows from many `PointLight`s with `shadows_enabled: true` add up — typical limit is ~4-8 shadow-casting lights per frame.
- Tuning fog density vs. light intensity is iterative; expect to revisit when textures and enemies arrive.
- Dark zones (cells with `dark_zone: true`) and anti-magic zones aren't lighting, but they should *feel* visually distinct — a design call to make later.

### What This Touches
- `src/plugins/dungeon/renderer.rs` (extend with lighting setup).
- `DungeonFloor` schema in `grid.rs` gains a `light_positions` field (or an `Option<TorchData>` per cell).
- Camera spawned in #7 gets a `DistanceFog` component.
- `assets/dungeons/floor_01.dungeon.ron` updated with sample torches.

### Impact Analysis

| Dimension | Baseline | After |
|-----------|----------|-------|
| LOC Δ | ~1850 | +120 to +200 |
| Deps Δ | 5 | 0 |
| Compile Δ | minor | negligible |
| Asset Δ | +20 | 0 (fog parameters live in code; torches in RON) |
| Frame time (test floor) | unmeasured | +0.5-2ms per shadow-casting torch |

### Broad Todo List

- Add `DistanceFog { color, falloff: Exponential { density: 0.15 }, ... }` to the dungeon camera.
- Set `AmbientLight` resource to a low warm value on `OnEnter(GameState::Dungeon)`.
- Add `light_positions: Vec<TorchData>` (or per-cell option) to `DungeonFloor` schema; rerun #4 round-trip tests.
- Implement `spawn_torch` per research §Code Examples.
- Cap shadow-casting torches at 4 per visible region; the rest are non-shadow lights.
- Add a flicker shader-free option: a system that animates `PointLight::intensity` with a sine + noise per frame.
- Update `floor_01.dungeon.ron` with 3-4 torch positions and verify atmosphere visually.

### Additional Notes
Fog color should match the dungeon's wall palette — research's example uses very dark blue; for stone dungeons, a warm dark gray often reads better. Make this a per-floor RON parameter so each level can have its own mood.

---

## 10. Auto-Map / Minimap

### Difficulty Rating
**2.5/5** — egui canvas drawing is straightforward; the data side (per-cell explored bit) is trivial.

### Overview
Add an `ExploredCells` resource: `HashMap<(floor: u32, x: u32, y: u32), ExploredState>` where `ExploredState` is `Unseen`, `Visited`, or `KnownByOther` (e.g. revealed by an item). Update on every `MovedEvent`. Render the map as an egui canvas in `DungeonSubState::Map` showing walls, doors, the player position + facing, and stairs/teleporters as icons. A small overlay version (top-right corner) can be toggled during exploration.

### Pros
- Auto-mapping is non-negotiable for the genre (research §Modern Takes); many players quit a DRPG without one.
- Per-floor `HashMap` is tiny (a 50x50 floor is 2500 entries); not a performance or save-size concern.
- egui canvas painters give full control over colors, icons, transparency — much easier than render-to-texture.

### Cons
- Drawing thousands of wall segments in egui per frame is slower than render-to-texture; large floors (>50x50) may need an upgrade (research §Open Question 5).
- "Draw your own map" mode (Etrian Odyssey style) is appealing but a much larger feature — defer.
- Overlay minimap during exploration competes with HUD real estate; placement decision impacts UI design.

### What This Touches
- `src/plugins/dungeon/minimap.rs` for the explored-cell tracking + egui render.
- `MovedEvent` subscriber in this plugin.
- `bevy_egui` dependency added (also used by every later UI feature, so this is the natural place to add it).
- `DungeonSubState::Map` for the full-screen view.
- `ExploredCells` becomes part of the save data later (#23).

### Impact Analysis

| Dimension | Baseline | After |
|-----------|----------|-------|
| LOC Δ | ~2050 | +350 to +500 |
| Deps Δ | 5 | +1 (`bevy_egui = "0.39"`) |
| Compile Δ | small | +2-4s clean (egui is non-trivial) |
| Asset Δ | +20 | 0-3 (icons for stairs / teleporter / door) |

Note: Adding `bevy_egui` here means every later UI feature builds on a verified base. If you defer it to #15 (combat), you'll wish you had hadn't.

### Broad Todo List

- Add `bevy_egui = "0.39"` and pin a 0.18-compatible release.
- Define `ExploredCells` resource and the `MovedEvent` listener that flips cells to `Visited`.
- Build an egui painter that draws the floor's grid lines, walls (using `WallType` for color), and per-cell explored shading.
- Render the player as an arrow (rotated by `Facing`) on top of the painter output.
- Implement full-screen map view in `DungeonSubState::Map`; toggle with `DungeonAction::OpenMap`.
- Implement minimap overlay (top-right, scaled-down version) during `DungeonSubState::Exploring`.
- Disable map updates inside cells where `dark_zone: true`; let stale data persist but show a "?" indicator.
- Add a "show full map" debug command (toggle reveals everything for testing).

### Additional Notes
This is the first feature that actually uses `bevy_egui` for a real screen — you'll discover its quirks here. Don't try to make it pretty yet; just functional. Visual polish lands in #25.

---

## 11. Party & Character ECS Model

### Difficulty Rating
**3/5** — many components and a lot of small decisions, but each piece is straightforward.

### Overview
Implement the full character model from research Pattern 3: `CharacterName`, `Race`, `Class`, `BaseStats`, `DerivedStats`, `Experience`, `PartyRow`, `PartySlot`, `Equipment`, `StatusEffects`, `ActiveEffect`, `StatusEffectType`, plus the `PartyMemberBundle`. Build `derive_stats(base_stats, equipment, status, level) -> DerivedStats` as the pure function that recomputes derived stats whenever inputs change. Spawn a default 4-character debug party on `OnEnter(GameState::Loading)` for testing.

### Pros
- Components-as-data (research §Anti-Patterns) means save/load (#23), UI (#19, #25), and combat (#15) all read the same source.
- The pure `derive_stats` function is unit-testable without any Bevy plumbing.
- Every component has `#[derive(Serialize, Deserialize)]` from the start (research §Pitfall 5) — saves are trivial later.

### Cons
- A debug party hard-coded in source is convenient but easy to forget about; it must be replaced by character creation (#19) before shipping.
- `Equipment` referencing `Entity` (per research) means equipped items are entities too — clean ECS, but creates lifetime-management issues across save/load if not handled carefully.
- 8 classes is the research's example; the actual class roster is a design call that affects every later balance + content decision.

### What This Touches
- `src/plugins/party/{character.rs, inventory.rs, progression.rs}` (with `inventory` and `progression` mostly stubs at this stage).
- `src/data/classes.rs` containing class growth tables.
- Tests in `cargo test plugins::party::character::tests`.

### Impact Analysis

| Dimension | Baseline | After |
|-----------|----------|-------|
| LOC Δ | ~2400 | +500 to +800 |
| Deps Δ | 6 | 0 (uses serde already in graph) |
| Compile Δ | small | +0.5s |
| Asset Δ | +20-23 | +1 (`classes.ron` placeholder) |
| Test count | ~14 | +6-10 |

### Broad Todo List

- Implement all components from research Pattern 3 with serde derives.
- Implement `PartyMemberBundle`.
- Implement `derive_stats(base, equipment, status, level)` and unit test against known inputs.
- Define a `PartySize: Resource` capping at 4-6 members (genre-typical).
- Add a `spawn_default_debug_party` system gated on `cfg(debug_assertions)` running on app startup.
- Author `classes.ron` with stat growth ranges for an initial 3-class set (Fighter, Mage, Priest) — defer the full 8-class table.
- Smoke-test by querying `(With<PartyMember>, &CharacterName)` and printing the party.

### Additional Notes
Resist starting with all 8 classes (research §Pitfall 6). Three classes is enough to validate the system; expand once the core combat loop is enjoyable. Race effects are a similar trap — pick one race for now (`Human`) and add the rest in #19.

---

## 12. Inventory & Equipment

### Difficulty Rating
**2.5/5** — the data model is light; the slot-by-slot UI is where time goes (deferred to a later UI pass).

### Overview
Define `Item` as a component on item entities (with `ItemKind`, `EquipSlot`, stat modifiers, weight, value, stackable flag). Define a `PartyInventory` resource (or a per-character `Inventory` component) holding a `Vec<Entity>`. Implement `equip(actor, item)`, `unequip(actor, slot)`, and `give_item(actor, item)` systems that maintain integrity (cannot equip a non-weapon in the weapon slot). Author `items.ron` with 8-12 starter items (rusty sword, leather armor, healing potion, lockpick).

### Pros
- Item-as-entity (research §Pattern 3 references `Equipment` fields holding `Entity`) makes "this specific sword has +2 from a temple blessing" trivial — each instance is unique.
- Stat re-derivation triggered on equip/unequip flows through #11's `derive_stats` for free.
- RON-driven items (research §Don't Hand-Roll) means designers add items without recompiling.

### Cons
- Item-as-entity complicates save/load — each item must be saved with its entity ID and remapped on load.
- Stackable items (potions × 5) clash with the "every item is a unique entity" model; you need a separate `StackableItem` component or a parallel inventory model.
- The first inventory UI is a heavy egui screen (drag/drop, tooltips, sort/filter) — defer the polish to #25.

### What This Touches
- `src/plugins/party/inventory.rs` for components, resources, and equip systems.
- `src/data/items.rs` for item definitions.
- `assets/items/items.ron` with starter items.
- An `EquipmentChangedEvent` that triggers stat re-derivation.

### Impact Analysis

| Dimension | Baseline | After |
|-----------|----------|-------|
| LOC Δ | ~3000 | +400 to +600 |
| Deps Δ | 6 | 0 |
| Compile Δ | small | +0.3s |
| Asset Δ | +21-24 | +1 (`items.ron`), +5-10 item icons |
| Test count | ~24 | +6-10 |

### Broad Todo List

- Define `Item`, `ItemKind { Weapon, Shield, Armor(Slot), Accessory, Consumable, KeyItem }`, `EquipSlot` enum.
- Define `Inventory(Vec<Entity>)` component or `PartyInventory` resource (decision: per-character, not pooled — Wizardry-style).
- Implement `equip(actor, item)` system with slot validation.
- Implement `EquipmentChangedEvent` and a system that re-runs `derive_stats` on the actor.
- Author `items.ron` with 8-12 starter items.
- Source/produce 5-10 placeholder 32x32 item icons (PNG) under `assets/ui/icons/items/`.
- Unit test: equipping a weapon updates `DerivedStats::attack`; unequipping reverses it.
- Stackable items: punt for now, document that potions are unique entities (suboptimal but simple).

### Additional Notes
Stackable items are surprisingly tricky in an entity-per-item world. Defer the "5x healing potion" UI until you've shipped the core combat loop and seen whether players actually accumulate enough items for it to matter.

---

## 13. Cell Features (Doors, Traps, Teleporters, Spinners)

### Difficulty Rating
**3/5** — each feature is small but they all interact with movement, encounter, and audio.

### Overview
Implement the `CellFeatures` reactions: doors (locked/unlocked, key-required), traps (pit, poison, alarm, teleport), teleporters (target floor + cell + facing), spinners (random facing rotation), dark zones (disable map updates), anti-magic zones (disable spell-casting flag in `Combat`). Each feature subscribes to `MovedEvent` (or runs `OnEnter` of the cell) and triggers its effect.

### Pros
- The data-driven approach (everything reads from the loaded `DungeonFloor`) means designers add tricks without code changes.
- Each feature is a single small system (`apply_pit_trap`, `apply_teleporter`, etc.) — easy to test in isolation.
- Cumulatively, these features are what make a dungeon crawl feel like a dungeon crawl (research §Essential Mechanical Systems).

### Cons
- Spinners are now telegraphed (decision §Resolved #4): visible icon, sound effect, brief screen wobble. The auto-map *must* update the displayed facing post-rotation — anything else breaks player trust. This means more art (spinner tile texture) and more SFX (rotation whoosh) than the classic-hidden alternative.
- Teleporters across floors trigger heavy state changes (despawn old geometry, load new floor RON, spawn new geometry) — a brief loading flicker is unavoidable without preloading.
- Locked doors require a key-item lookup — depends on inventory (#12) being live before this feature can be fully tested.

### What This Touches
- `src/plugins/dungeon/features.rs` containing all cell-feature systems.
- New `KeyItem` flag on `Item` (or a tag component) consumed by locked doors.
- `MovedEvent` subscribers across multiple feature systems.
- `floor_01.dungeon.ron` augmented with one of each feature for testing.
- A `TeleportRequested` event that triggers a state-managed floor transition.

### Impact Analysis

| Dimension | Baseline | After |
|-----------|----------|-------|
| LOC Δ | ~3450 | +400 to +700 |
| Deps Δ | 6 | 0 |
| Compile Δ | small | +0.3s |
| Asset Δ | +27-34 | +2-4 (door textures, trap SFX) |
| Test count | ~32 | +6-8 |

### Broad Todo List

- Implement `apply_pit_trap` (apply damage, optionally drop to next floor).
- Implement `apply_poison_trap` (apply `StatusEffect::Poison` to the party).
- Implement `apply_alarm_trap` (force an encounter via #16).
- Implement `apply_teleporter` (state transition: despawn current floor, load new floor, set new `GridPosition`).
- Implement `apply_spinner` (randomize `Facing` of `PlayerParty`; play whoosh SFX, trigger 200ms screen wobble, update auto-map indicator — telegraphed UX per §Resolved #4).
- Implement door interaction: pressing `DungeonAction::Interact` against a `WallType::Door` cell pair toggles open/closed.
- Implement locked-door check (consumes / requires a `KeyItem` from `Inventory`).
- Add corresponding SFX (door creak, trap snap) routed through #6.
- Update `floor_01.dungeon.ron` to include one of each.
- Add the spinner asset trio: tile texture, whoosh SFX, screen-wobble shader / camera shake.

### Additional Notes
Spinner UX is locked to telegraphed (§Resolved #4). Dungeon authors across all 20 floors should treat them as a player-facing puzzle ("you've been spun, where are you facing now?") rather than as a hidden trust-breaker.

---

## 14. Status Effects System

### Difficulty Rating
**2.5/5** — components + duration tracking are simple; the interaction tree (poison + regen + paralysis) is where bugs hide.

### Overview
Implement the duration-tracked status effects from research Pattern 3 (`StatusEffects`, `ActiveEffect`, `StatusEffectType`). Build per-status systems: `tick_status_durations` (every combat round and/or every dungeon step), `apply_poison_damage`, `block_action_if_paralyzed`, `block_spells_if_silenced`, `block_action_if_asleep`, etc. Add an `ApplyStatusEvent { target, effect, potency, duration }` that any system can fire.

### Pros
- One canonical event (`ApplyStatusEvent`) means every source — traps, enemy spells, items — uses the same path.
- Per-effect functions are small and isolated; easy to unit-test (apply poison, tick 3 rounds, verify HP reduction).
- Buffs (`AttackUp`, `DefenseUp`) and debuffs share infrastructure, halving the work.

### Cons
- Stacking rules are ambiguous: does a second `Poison` reset the duration, extend it, or stack potency? You must decide and document.
- "Permanent until cured" effects (`Stone`, `Dead`) require a different cure path (temple at the town hub, #18); status code becomes town-aware.
- A status effect's UI representation (icon over the portrait) is content work that piles up in #25.

### What This Touches
- `src/plugins/combat/status_effects.rs` (the central status registry).
- Cross-cutting: `combat/turn_manager.rs` (skip turn if paralyzed/asleep), `dungeon/movement.rs` (poison damage over steps).
- Tests in `cargo test plugins::combat::status_effects::tests`.

### Impact Analysis

| Dimension | Baseline | After |
|-----------|----------|-------|
| LOC Δ | ~3950 | +350 to +500 |
| Deps Δ | 6 | 0 |
| Compile Δ | small | +0.2s |
| Asset Δ | +29-38 | 0-2 (status icons later) |
| Test count | ~38 | +8-12 |

### Broad Todo List

- Implement `ApplyStatusEvent` and the central handler that adds/refreshes effects on the target's `StatusEffects`.
- Implement `tick_status_durations` system (decrement `remaining_turns`, remove on 0).
- Implement per-effect resolvers: poison damage tick, paralysis (skip turn), sleep (skip turn, breaks on damage), silence (block spells), blind (reduce accuracy), confused (random target), stone/dead (no actions until cured).
- Implement the buff variants: `AttackUp`, `DefenseUp`, `SpeedUp`, `Regen` mutate `derive_stats` output.
- Document stacking rules: same effect refreshes duration; potency takes the higher value.
- Unit tests: apply poison, tick 5 rounds, verify HP and removal.
- Hook poison/regen ticks to *both* combat rounds (in combat) and dungeon steps (out of combat) — same event, different triggers.

### Additional Notes
Status effects are a sneaky source of save-file complexity. Treat the `Vec<ActiveEffect>` as opaque for serialization (just persist it as-is) and tick durations only via the canonical system, never inline.

---

## 15. Turn-Based Combat Core

### Difficulty Rating
**4/5** — the most architecturally significant feature; ties together party, status, encounter, AI, UI, and animation.

### Overview
Implement the action-queue combat loop from research Pattern 5: collect each party member's chosen action through `CombatPhase::PlayerInput`, append enemy AI actions, sort by `speed`, then `CombatPhase::ExecuteActions` resolves them one by one with damage, healing, status, and item effects, finally `CombatPhase::TurnResult` checks for victory/defeat/flee. Combat UI is a heavy egui screen showing party HP/MP bars, enemy display, and action menus (Attack / Defend / Spell / Item / Flee → target selection).

### Pros
- The action-queue model (research Pattern 5) is the proven genre standard — copies what Wizardry, Etrian Odyssey, and Undernauts already validate.
- Combat is the longest single gameplay loop in the game; getting it right is the difference between "I'll play 60 hours" and "I refunded after the tutorial".
- ECS-driven combat (research §Anti-Patterns) means damage, status, and accuracy are independent systems that compose.

### Cons
- This feature is a thicket: turn ordering, action targeting, target validation (target died this turn?), animation timing, UI state, AI scripting all intersect. Expect 1-2 weeks of focused work plus follow-up bug-fixing.
- "Front row vs. back row" damage modifiers and "back-row weapons can't hit back row" rules need to be encoded somewhere — whichever module owns it becomes a hub of conditional logic.
- AI is a tar pit: even simple "pick a random target" is fine for fodder, but bosses need scripted AI patterns. You either hard-code or build a mini-DSL — both are iceberg-shaped.

### What This Touches
- `src/plugins/combat/{turn_manager.rs, actions.rs, damage.rs, ai.rs, ui_combat.rs}` — the largest single feature footprint.
- `Encounter` resource (depends on #16).
- Heavy egui usage (combat menu, target selection, damage popups).
- New `CombatLog: Resource` capturing recent events for the player (e.g. "Goblin attacks Aldric for 7 damage").
- Tests across damage, ordering, and target resolution.

### Impact Analysis

| Dimension | Baseline | After |
|-----------|----------|-------|
| LOC Δ | ~4350 | +1000 to +1800 |
| Deps Δ | 6 | 0 (uses already-included `rand`, `bevy_egui`) |
| Compile Δ | medium | +1-2s |
| Asset Δ | +29-40 | 0-3 (placeholder enemy sprites — full work in #17) |
| Test count | ~50 | +20-30 |

Note: This is the single largest feature in the roadmap. Plan it as multiple sub-PRs (turn manager → damage → AI → UI) rather than one monolithic change.

### Broad Todo List

**Turn manager**
- Implement `TurnActionQueue: Resource` and `PlayerInputState: Resource`.
- Implement `player_combat_input` system collecting actions slot-by-slot.
- Implement `enemy_ai_action_select` filling enemy actions.
- Implement `sort_by_speed` and `execute_combat_actions` per research Pattern 5.

**Damage / actions**
- Implement `damage_calc(attacker, defender, action)` with research-recommended formulas (Wizardry/Etrian-style).
- Implement `Attack`, `Defend` (sets a 1-turn `DefenseUp`-equivalent), `UseItem`, `Flee` action handlers.
- Stub `CastSpell` — full implementation deferred to #20.

**Combat UI**
- Implement an egui combat screen with party HP/MP bars and a left-aligned enemy column.
- Implement action menu (Attack/Defend/Spell/Item/Flee) and target selection sub-menu.
- Implement a scrolling combat log resource + UI panel.

**Targeting**
- Implement `TargetSelection` resolution (single enemy, all enemies, single ally, all allies, self).
- Implement target validation (re-target if the original died, fall through gracefully).

**AI**
- Implement a baseline `random_target_attack` AI for fodder enemies.
- Stub a `BossAI` enum so boss scripts can be added later.

**Tests**
- Damage formula edge cases (defense > attack, criticals, 0-HP).
- Turn ordering with ties.
- Action queue execution with mid-turn deaths.

### Additional Notes
Implement damage as a *pure* function `(attacker_stats, defender_stats, weapon, action) -> DamageResult`. Make AI just emit `CombatAction`s into the queue. Make UI just call out into target-selection state. The temptation to mix concerns is enormous; resist it. Every combat bug you ever ship will live at the seams.

---

## 16. Encounter System & Random Battles

### Difficulty Rating
**2.5/5** — straightforward probabilistic logic, but the tuning loop is real work (research §Pitfall 4).

### Overview
Implement the encounter check from research §Code Examples: per-cell `encounter_rate`, per-step accumulator boosting probability over time, per-floor encounter table mapping rolls to enemy groups. On encounter, transition `GameState::Dungeon → GameState::Combat`, populate a `CurrentEncounter` resource with the spawned enemies, and let #15 take over. On combat end, return to dungeon at the same grid position.

### Pros
- Per-cell encounter rate is data-driven (research §Pitfall 4), letting designers vary tension without code changes.
- Step-counter accumulation gives "no encounter for 30 steps" a soft pity timer, which is much fairer than pure RNG.
- Separating encounter detection (#16) from combat resolution (#15) means each can change without the other — a frequent ask during balance tuning.

### Cons
- Random encounters are universally divisive, but the project ships both random encounters AND FOEs (decision §Resolved #2). The two systems must coexist gracefully on the same floor: an FOE in line-of-sight should suppress random rolls (otherwise the player gets "surprise" battles while staring at a known enemy on the map). Plan for #22 to publish a `FoeProximity` resource that #16 reads.
- Encounter tables are content (research §Pitfall 6); a 20-floor dungeon needs 20-40 unique tables, plus rare/uncommon/common groupings.
- Re-entering the dungeon after combat must restore exact pre-combat state (cell, facing, animation finished) — easy bug source.

### What This Touches
- `src/plugins/combat/encounter.rs` for probability checks + table loading.
- `assets/encounters/*.encounters.ron` per floor.
- `src/plugins/combat/mod.rs` for the `GameState::Dungeon → Combat → Dungeon` transition flow.
- A `MovedEvent` subscriber for the step counter.
- `CurrentEncounter: Resource` populated on transition.

### Impact Analysis

| Dimension | Baseline | After |
|-----------|----------|-------|
| LOC Δ | ~5500 | +400 to +600 |
| Deps Δ | 6 | +1 (`rand = "0.8"`) |
| Compile Δ | small | +0.5s |
| Asset Δ | +29-43 | +3-6 (encounter table RON files) |
| Test count | ~75 | +6-10 |

### Broad Todo List

- Define `EncounterTable { entries: Vec<(weight, EnemyGroup)> }` and `EnemyGroup { enemies: Vec<EnemySpec> }`.
- Implement `EncounterState: Resource` (steps_since_last, base_rate).
- Implement `check_random_encounter` system per research §Code Examples.
- Implement `start_combat` system: spawn enemies based on rolled `EnemyGroup`, transition state.
- Implement post-combat cleanup: despawn enemy entities, return to `GameState::Dungeon`, restore prior `DungeonSubState`.
- Author `b1f_encounters.ron` with 3-5 enemy groups.
- Add a `?force_encounter` debug command for testing.
- Tests: probability over many steps converges to expected rate; force-encounter triggers correctly.

### Additional Notes
Defer the FOE/visible-enemy system to #22. They share the encounter end-game (transition to combat, spawn enemies) so #16 should expose `start_combat(enemy_group)` as the single entry point — both random and FOE encounters call it.

---

## 17. Enemy Billboard Sprite Rendering

### Difficulty Rating
**3/5** — `bevy_sprite3d` does most of the heavy lifting, but verifying 0.18 compat (research §Open Question 2) and the art pipeline are the real cost.

### Overview
Add `bevy_sprite3d 7.x` (verifying 0.18 compatibility — fall back to a custom textured-quad-faces-camera system if incompatible). On combat entry, spawn enemy entities as billboards in 3D space arranged in a row in front of the camera. Sprites support idle / attack / damage / dying frames driven by an animation state machine. Out of combat, billboards are also used for FOEs (#22) walking on the dungeon grid.

### Pros
- Billboarded 2D sprites are the genre-correct visual (research §Architecture Options) — Etrian Odyssey, Undernauts, Wizardry remake all use this approach.
- Art pipeline cost is dramatically lower than full 3D enemy models (no rigging, no animation export).
- Sprite swaps for different enemy types are O(1) — change the texture handle.

### Cons
- `bevy_sprite3d` v7.0.0's exact 0.18.1 compat needs verification; if it lags, you write the billboard math (research §Open Question 2 explicitly flags this).
- Single-facing sprites (always face camera) feel cheap if the player walks around an FOE — a design decision: 4-direction sprites cost 4x art per enemy.
- Animation transitions (idle → attack → return to idle) are state-machine work that compounds for a roster of 30+ enemies.

### What This Touches
- `src/plugins/combat/enemy_render.rs`.
- Reused by `src/plugins/dungeon/foe.rs` (#22).
- `Cargo.toml` (`bevy_sprite3d = "7"`, with the fallback documented).
- `assets/enemies/<enemy_id>/*.png` sprite sheets.

### Impact Analysis

| Dimension | Baseline | After |
|-----------|----------|-------|
| LOC Δ | ~5950 | +400 to +600 |
| Deps Δ | 7 | +1 (`bevy_sprite3d`) |
| Compile Δ | small | +1-2s clean |
| Asset Δ | +32-49 | +5-15 placeholder sprite sheets (~5-30 MB) |
| Bundle size | ~baseline | +5-30 MB depending on sprite resolution |

Note: 256-512px enemy sprites with 4-8 frames each grow fast. Use a sprite atlas + texture compression (KTX2 + zstd, already enabled in #1's feature set) to keep size bounded.

### Broad Todo List

- Verify `bevy_sprite3d 7.x` Bevy 0.18 compat on crates.io and Cargo.lock; have the manual fallback ready.
- Define `EnemyVisual` component: sprite handle, frame count, current animation state.
- Implement `spawn_enemies_for_encounter` system: place 1-4 enemies in a line in front of camera at fixed offsets.
- Implement an `EnemyAnimation` state machine (`Idle`, `Attacking`, `TakingDamage`, `Dying`).
- Add a "damage shake" tween effect on damage taken.
- Source/produce 5-10 placeholder enemy sprite sheets (CC0 itch.io packs are a good starting point).
- Author `enemies.ron` mapping enemy IDs to sprite paths + stats.
- Smoke-test by forcing an encounter and confirming enemies render facing the camera.

### Additional Notes
For FOEs (#22) the same visual pipeline is reused with a different transform parent (the FOE's grid position rather than the combat scene). Plan the API for that reuse now: `spawn_enemy_visual(commands, enemy_spec, position)` should be agnostic to whether it's combat or overworld.

---

## 18. Town Hub & Services

### Difficulty Rating
**3/5** — five interlocking egui screens; mostly UI work but each interacts with party, inventory, and economy.

### Overview
Implement `GameState::Town` with sub-states `Square`, `Shop`, `Inn`, `Temple`, `Guild`. The town square is a static backdrop with menu options. Shop buys/sells items against a gold currency. Inn rests the party (full HP/MP heal, time advances). Temple revives dead, cures stone/poison/etc for a price. Guild manages party composition (recruit/dismiss; full character creation lives in #19).

### Pros
- A clear "safe haven" loop is core to the genre's tension — without a town, there's no satisfaction in returning.
- Each service is a self-contained egui screen; they can be built incrementally.
- Everything is a content extension of features already built (party, inventory, status, gold).

### Cons
- Five distinct UI screens is a lot for one feature. Consider shipping town in two passes: shop + inn first, temple + guild later.
- Gold is an economy decision (research §Pitfall 6); pricing items requires balance work that lives in #21.
- A truly atmospheric town (NPCs, music, voice, art) is a giant content sink. Keep this feature scoped to the *services*, not the *flavor*.

### What This Touches
- `src/plugins/town/{shop.rs, inn.rs, temple.rs, guild.rs, square.rs}`.
- Heavy egui screens for each.
- `src/data/items.rs` extended with `buy_price`, `sell_price`, `available_in_town`.
- A `Gold` resource (or per-party `Gold` component) tracking the player's coin.
- BGM track switch on entering town.

### Impact Analysis

| Dimension | Baseline | After |
|-----------|----------|-------|
| LOC Δ | ~6450 | +800 to +1300 |
| Deps Δ | 8 | 0 |
| Compile Δ | small | +0.5s |
| Asset Δ | +37-64 | +3-6 (town backdrop, town BGM, NPC portraits) |
| UI screens | ~3-4 | +5 |

### Broad Todo List

**Square**
- Implement `TownLocation::Square` egui screen with a list of services and "leave town" option.

**Shop**
- Implement buy mode: list available items, deduct gold, add to party inventory.
- Implement sell mode: list party items, return gold, remove item.
- Bound stock by floor progression (data-driven).

**Inn**
- Implement rest: full HP/MP heal, advance an in-game clock, charge gold.
- Optionally apply rest-cure to mild status effects (poison) but not severe (stone, dead).

**Temple**
- Implement revive (dead → 1 HP, charge gold proportional to level).
- Implement cure-status (stone, poison) for gold.

**Guild**
- Implement party roster view (your active 4-6 + dismissed pool).
- Implement recruit: pick from pre-made character pool (full creation in #19).
- Implement dismiss / reorder party slots.
- Implement front/back row swap.

### Additional Notes
A single hand-painted "town square" backdrop (one PNG) is enough atmosphere for now. Defer 3D town rendering indefinitely — it's not what the genre is about. The square is a menu, not a level.

---

## 19. Character Creation & Class Progression

### Difficulty Rating
**3/5** — the leveling math is small but the creation UI is the most data-entry-heavy screen in the game.

### Overview
Implement the full Guild creation flow: pick race, pick class, allocate or roll stats, choose a name, accept. On creation, a new character entity is spawned with the right components. Implement leveling: `Experience` accumulates, on threshold the character levels up with class-driven stat gains. Implement class change (Bishop, Lord, Samurai, Ninja in research Pattern 3) with stat penalties + skill retention.

### Pros
- Character creation is many players' favorite part of a DRPG; investing here pays back hundreds of hours of player time.
- Leveling is a small, testable function (`level_up(character, class) -> StatGains`).
- Class change adds depth at low marginal code cost — most logic is data tables.

### Cons
- Stat allocation systems (point-buy vs. rolled vs. hybrid) are a design decision that you can't easily reverse — affects every later balance call.
- Class advancement requirements (`Bishop requires Mage L5 + Priest L5`) is a graph of prerequisites; getting it wrong creates locked-out characters.
- The creation screen has the highest egui complexity of any UI: scrollable race/class lists, dynamic stat displays, modal confirmations. Plan two passes (functional, then polished).

### What This Touches
- `src/plugins/town/guild.rs` (extended substantially) and `src/plugins/party/progression.rs`.
- New `assets/classes/classes.ron` with class definitions, growth tables, and advancement requirements.
- `assets/races/races.ron` with race modifiers.
- Character creation egui screen.

### Impact Analysis

| Dimension | Baseline | After |
|-----------|----------|-------|
| LOC Δ | ~7550 | +500 to +900 |
| Deps Δ | 8 | 0 |
| Compile Δ | small | +0.3s |
| Asset Δ | +40-70 | +2-4 (class portraits, races/classes RON) |
| Test count | ~85 | +8-12 |

### Broad Todo List

- Define `RaceData` and `ClassData` structs with stat modifiers, growth ranges, restrictions.
- Author `races.ron` (Human, Elf, Dwarf, Gnome, Hobbit) and `classes.ron` (Fighter, Mage, Priest, Thief, Bishop, Samurai, Lord, Ninja).
- Implement stat allocation: rolled bonus points + minimum thresholds for each class (research Pattern 3 implies a Wizardry-style "bonus pool").
- Implement `level_up(character, class) -> StatGains` consulting `ClassData` growth table.
- Implement class change: requirements check, stat penalties, retain skills.
- Implement the egui creation screen.
- Hook XP gain on combat victory (drives `Experience::current_xp`).
- Unit tests: leveling produces expected ranges; class change requirements correctly reject ineligible characters.

### Additional Notes
Resist shipping all 8 classes day one (research §Pitfall 6). Three classes is enough to validate the leveling system; expand once #15 combat is fun and balanced.

---

## 20. Spells & Skill Trees

### Difficulty Rating
**3.5/5** — spell registry is moderate; balance tuning across 30-50 spells is the hidden work.

### Overview
Implement a spell registry: each spell has an ID, MP cost, target type, level, school (mage/priest), effects. Wire `ActionType::CastSpell` (stub from #15) to actually resolve effects (damage, heal, status, buff). Implement skill trees: per-class trees of spells and passive abilities, learned by spending skill points on level up.

### Pros
- Spell variety is core to the genre's depth and replayability.
- Data-driven spells (research §Don't Hand-Roll) — adding a spell is a RON entry plus optional special-case Rust.
- Skill trees give classes long-term progression beyond raw stat gains.

### Cons
- Spell balance is a combinatorial nightmare (research §Pitfall 6) — a 100-damage spell is fine alone but broken with a 3x AttackUp buff and a Silence resistance bypass.
- Skill trees create class identity but increase build complexity; a 50-node tree per class is many weeks of design.
- "Stop spell" / silence interactions need extra plumbing beyond #14's silence.

### What This Touches
- `src/plugins/combat/{spell.rs, skills.rs}`.
- `src/plugins/party/skills.rs` for the per-character known-spells / skill-tree component.
- `assets/spells/spells.ron` and `assets/skills/<class>.skills.ron`.
- Combat UI extended with spell-list submenu.

### Impact Analysis

| Dimension | Baseline | After |
|-----------|----------|-------|
| LOC Δ | ~8550 | +700 to +1200 |
| Deps Δ | 8 | 0 |
| Compile Δ | small | +0.5s |
| Asset Δ | +43-74 | +5-10 (spell/skill RON files, spell icons) |
| Test count | ~95 | +10-15 |

### Broad Todo List

- Define `Spell { id, name, mp_cost, target, level, effect: SpellEffect }`.
- Define `SpellEffect` enum (Damage, Heal, ApplyStatus, Buff, RaiseDead, etc.).
- Implement `cast_spell` system in combat that resolves the effect.
- Define `SkillTree`: a per-class graph of nodes, each unlocking a spell or a passive.
- Implement skill-point allocation (1-2 per level up, configurable).
- Author `spells.ron` with 15-25 starter spells (3 levels × 2 classes initially).
- Author `fighter.skills.ron`, `mage.skills.ron`, `priest.skills.ron` initial trees.
- Combat UI: spell submenu (target → spell → cast).
- Unit tests: damage spell, heal spell, status spell each resolve correctly.
- Add a "spell sim" debug command that runs N battles with random spell choices to surface broken combinations.

### Additional Notes
Defer the full Wizardry-style mage/priest spell count (50+ each) until the engine is shipped. Even 10 well-tuned spells are more fun than 50 imbalanced ones.

---

## 21. Loot Tables & Economy

### Difficulty Rating
**3/5** — straightforward systems; tuning the loot/gold curve is iterative content work.

### Overview
Implement loot drops: each `EnemyGroup` defines a loot table (weighted entries: item, gold, nothing). On victory, roll the table and award items + gold to the party. Implement gold + item transactions in shops (#18). Establish a per-floor pricing curve so loot feels rewarding without inflating shop affordability.

### Pros
- Loot is the genre's most addictive feedback loop — players push deeper for the chance at rare drops.
- The tables are pure data; rebalancing is a `cargo run` away.
- Sets up rare/unique loot (named weapons, dungeon-floor-bound drops) for late-game motivation.

### Cons
- Pricing imbalance is the most-reported genre complaint (research §Pitfall 6). Expect to retune dozens of times.
- Rare-drop frustration is real: if a player kills 200 of an enemy without their drop, that's a UX failure regardless of math.
- Crafting / upgrading systems naturally extend from loot but explode scope; defer them entirely.

### What This Touches
- `src/plugins/combat/rewards.rs` for victory loot resolution.
- `assets/loot/<floor>.loot.ron` per floor.
- `src/data/items.rs` extended with `rarity` and `drop_only` flags.
- A `LootChestEvent` for chests in dungeons (extends #13's cell features).

### Impact Analysis

| Dimension | Baseline | After |
|-----------|----------|-------|
| LOC Δ | ~9750 | +400 to +700 |
| Deps Δ | 8 | 0 |
| Compile Δ | small | +0.3s |
| Asset Δ | +48-84 | +3-6 (loot tables, chest assets) |
| Test count | ~110 | +6-10 |

### Broad Todo List

- Define `LootTable { entries: Vec<LootEntry { weight, drop: LootDrop }> }`, `LootDrop { Gold(min, max), Item(id, count), Nothing }`.
- Implement `award_loot` system on combat victory.
- Add chest cells to the dungeon RON schema; `Interact` opens chest, awards loot.
- Author per-floor loot tables.
- Implement a "loot-scaling" multiplier toggle (difficulty setting) for #25.
- Add a debug command to dump loot rolls for an enemy across 1000 simulated kills.

### Additional Notes
The pity timer pattern (e.g. "guaranteed rare drop after 100 kills") is a much more modern QoL than any classic Wizardry approach. Worth implementing early as an opt-in, not bolted on after launch.

---

## 22. FOE / Visible Enemies

### Difficulty Rating
**3.5/5** — pathfinding + grid-aware AI is moderate, but the interplay with player movement timing has subtle bugs.

### Overview
Implement FOEs (Field-On-Enemies, Etrian Odyssey term): visible enemy entities on the dungeon grid that move 1 cell per player step toward the player using A* (`pathfinding` crate). On collision, transition to combat (using #16's `start_combat(enemy_group)`). FOEs are powerful mini-bosses; defeating them is optional but rewards rare loot.

### Pros
- FOEs are the modern QoL upgrade over pure-random encounters (research §Modern Takes) — players love the agency of "I can see it coming and decide whether to fight".
- Reuses the billboard sprite pipeline (#17) and the encounter starter (#16) — most heavy lifting is already done.
- Adds spatial puzzle gameplay: avoiding/luring FOEs is a meta-mini-game on top of dungeon traversal.

### Cons
- A* on a small grid is fast, but FOEs that re-plan every player step can feel uncannily prescient. Add a perception range and patrol patterns to keep them feeling fair.
- FOE state must persist across save/load (where they were last seen), expanding #23's scope.
- FOEs that can attack you in the back (you turn, they're now in front) is a UX nightmare; design carefully.

### What This Touches
- `src/plugins/dungeon/foe.rs`.
- `pathfinding = "4.8"` crate added.
- Reuses #17's billboard rendering for FOE visuals.
- Reuses #16's `start_combat` entry point.
- `assets/dungeons/floor_*.dungeon.ron` extended with FOE spawn definitions.

### Impact Analysis

| Dimension | Baseline | After |
|-----------|----------|-------|
| LOC Δ | ~10450 | +500 to +800 |
| Deps Δ | 8 | +1 (`pathfinding = "4.8"`) |
| Compile Δ | small | +0.5s clean |
| Asset Δ | +51-90 | +2-5 (FOE sprite sheets) |
| Test count | ~120 | +6-10 |

### Broad Todo List

- Define `Foe` component: `enemy_group_id`, `patrol_pattern`, `perception_range`, `state: FoeState`.
- Define `FoeState` enum: `Patrolling`, `Pursuing`, `Returning`, `Defeated`.
- Implement A*-based pathfinding using the `pathfinding` crate over the dungeon `WallType` graph.
- Implement `move_foes` system on each player `MovedEvent` step.
- Implement collision-on-cell triggering combat via `start_combat`.
- Render FOEs using #17's billboard pipeline at their grid position.
- Add FOE spawn definitions to dungeon RON.
- Persist FOE state in save data (#23).
- Tests: A* avoids walls; FOE chases reduce distance; perception range gates engagement.

### Additional Notes
A simple "tile-based perception cone" works well: FOE only chases if the player is in line-of-sight or within N tiles. This avoids the "all FOEs converge on the player from across the map" problem that breaks player trust.

---

## 23. Save / Load System

### Difficulty Rating
**3.5/5** — non-trivial because it touches every gameplay system; the worst pain comes from architectural debt accumulated *before* this feature lands.

### Overview
Implement save: serialize all `Saveable`-tagged entities + relevant resources (party, inventory, equipment, status, gold, current dungeon, grid position, facing, explored cells, FOE state, encounter step counter, RNG seed — **required, since permadeath is in scope**) to a RON file via `moonshine-save` (decision §Resolved #3 — `bevy_save` requires `bevy ^0.16`, not viable on our 0.18.1 pin). Implement load: restore state, transition to the appropriate `GameState`, re-spawn the dungeon and party. **Iron Mode**: autosave only on safe checkpoints (floor transitions, town entry); on death, the save file is deleted and the run ends.

### Pros
- Save/load is mandatory for the genre — no shipping without it.
- Designing components as serializable from day one (research §Pitfall 5) means most of the work is "decide what to mark `Saveable`" rather than refactoring.
- moonshine-save round-trips through Bevy's `DynamicScene`, giving atomic writes for free (critical for permadeath: no half-written save on crash).

### Cons
- Save touches everything. Any feature that hides state in a non-component (e.g. `Local<T>` system state, render-only data) becomes a bug magnet on load.
- `Entity` IDs change across runs; equipped items (which reference `Entity`) require a remap pass — easy to miss for tests, hard to miss in practice.
- moonshine-save is younger and less-tutorialised than bevy_save; expect to lean on its docs/examples directly rather than community blog posts.
- Permadeath/Iron Mode means **save corruption = total run loss**. The atomic-write path must be verified by integration test (kill mid-write, confirm the prior save is still loadable), not just trusted.

### What This Touches
- `src/plugins/save/{mod.rs, save_data.rs, serialization.rs}`.
- Cross-cutting: `#[derive(Serialize, Deserialize)]` on every gameplay component (already done in #11+ if discipline held).
- A `SaveSlot` resource and the title-screen "Continue / Load" UI (#25).
- `assets/saves/` (or platform-appropriate path; on Mac `~/Library/Application Support/druum/`).

### Impact Analysis

| Dimension | Baseline | After |
|-----------|----------|-------|
| LOC Δ | ~11250 | +600 to +1000 |
| Deps Δ | 9 | +1 (`moonshine-save = "=0.6.1"`, decision §Resolved #3) |
| Compile Δ | small | +1-3s clean |
| Asset Δ | +53-95 | 0 (saves write to user data dir, not `assets/`) |
| Test count | ~130 | +10-15 |

### Broad Todo List

- Audit every component for `#[derive(Serialize, Deserialize)]` and `#[derive(Reflect)]` — fix any that drifted (moonshine-save uses Bevy's reflection / `DynamicScene`).
- Add `moonshine-save = "=0.6.1"` to `Cargo.toml`; verify Bevy 0.18.1 compat against the lockfile on first build.
- Define `Saveable` marker component; tag party, inventory, FOE state, etc.
- Seed RNG deterministically from a `RngSeed` resource that ships in every save (decision §Resolved #5 — Iron Mode requires reproducibility).
- Implement Iron Mode flag: a per-save `IronMode: bool` that disables manual saves and triggers `delete_save_on_death` when the party is wiped.
- Implement save: collect all `Saveable` entities + relevant `Resource`s, serialize to RON.
- Implement load: clear current world (carefully — preserve UI / asset handles), spawn from saved state, transition to the saved `GameState`.
- Implement an `Entity` remap pass for `Equipment.weapon: Option<Entity>` and similar references.
- Persist `ExploredCells`, `EncounterState`, `Gold`, `CurrentDungeonFloor` (or its ID).
- Implement multiple save slots (3-5 typical).
- Implement save versioning header (`save_version: u32`); reject incompatible versions explicitly with a helpful message.
- Round-trip integration test: save a complex world state, load it, assert deep equality.

### Additional Notes
Permadeath / Iron Mode is in scope from day one (§Resolved #5), not deferred. The architectural cost is small ("delete save on party-wipe" plus UI confirmation) but the **discipline cost is high**: every gameplay feature touched between now and #23 must use the deterministic `RngSeed` resource for any randomness, never `thread_rng()` or `rand::random()`. A run-time lint that fails the build on `rand::thread_rng()` calls is worth setting up early.

---

## 24. Dungeon Editor Tool

### Difficulty Rating
**4/5** — building an editor is real product work, even when targeted at a single dev. Research §Pitfall 3 calls this out as a high-leverage early investment.

### Overview
Build an in-game egui-based dungeon editor: load a dungeon RON, edit cell-by-cell (toggle wall sides, place doors, add traps/teleporters/spinners, paint encounter rates, position FOEs and torches), save back to RON. Run it as a debug-mode binary (`cargo run --bin editor --features dev`) reusing the gameplay's grid + render code so what-you-see-is-what-you-play.

### Pros
- Authoring 20 floors by hand-editing RON is realistically infeasible (research §Pitfall 3). An editor pays back its cost within the first 5 floors.
- Reusing gameplay rendering (the editor *is* the dungeon view) keeps preview accurate by definition.
- An editor is a forcing function for clean grid + RON APIs; data model warts surface fast.

### Cons
- Editors are a tar pit: undo/redo, multi-cell selection, copy/paste, validation, error reporting all expand scope.
- The editor competes with gameplay for development attention; a half-built editor is worse than no editor (you'll hand-edit RON *and* fight the editor).
- An editor is a separate binary's worth of compile time and dependencies.

### What This Touches
- New `src/bin/editor.rs` or `src/plugins/editor/mod.rs` gated on `#[cfg(feature = "dev")]`.
- Reuses `src/plugins/dungeon/{grid.rs, renderer.rs}` for in-engine preview.
- New egui side panels for cell properties, wall toggles, and feature placement.
- Save-back uses `ron::ser::to_string_pretty` to match the hand-authored format.

### Impact Analysis

| Dimension | Baseline | After |
|-----------|----------|-------|
| LOC Δ | ~12150 | +1500 to +2500 |
| Deps Δ | 10 | 0 (extends existing crates) |
| Compile Δ | small | editor binary alone adds 5-10s clean |
| Asset Δ | +53-95 | 0-3 (editor icons) |
| Test count | ~145 | +5-10 |

### Broad Todo List

- Decide on architecture: separate binary vs. dev-only mode in main binary. Recommend separate binary for CI-friendly flag-gating.
- Implement load + save of `.dungeon.ron` files via file picker.
- Implement free-camera mode (orbit / pan / zoom) over the loaded dungeon.
- Implement per-cell click-to-select with a properties panel.
- Implement wall-side toggling on the selected cell with live geometry update.
- Implement feature placement: trap, teleporter, spinner, dark zone, anti-magic, encounter rate.
- Implement FOE / torch position painting.
- Implement validation: warn on orphaned teleporter targets, walls that disagree across cell pairs, unreachable cells.
- Implement undo / redo (a simple snapshot-on-action stack is enough).
- Add a "playtest from here" button that drops the player into the loaded floor for a quick run.

### Additional Notes
This is the highest-leverage non-gameplay feature in the roadmap. If you ship 5 floors hand-authored, you'll wish you'd built this earlier. If you build it before the gameplay is fun, you'll have nothing to author. Land this once #4 (grid model) and #13 (cell features) are stable, but before authoring more than 2-3 floors of content.

---

## 25. Title Screen, Settings & End-to-End Polish

### Difficulty Rating
**3/5** — many small UI tasks; the hard part is knowing when to stop.

### Overview
Implement the title screen (`GameState::TitleScreen`): new game, continue, load slot, settings, quit. Implement settings: master/per-channel volume, key/gamepad rebinding, fullscreen/windowed, resolution, difficulty, classic-vs-modern QoL toggles. Implement end-to-end UX polish: tooltips, loading transitions, dialogue boxes, font selection, color palette pass, victory/game-over screens, credits.

### Pros
- The first-impression flow from launch → title → new game shapes how every reviewer perceives the game.
- Settings done right (especially rebinding) addresses a class of player frustration that no amount of gameplay can fix.
- A polished title screen is also the easiest place to demo the game's atmosphere — high-leverage marketing surface.

### Cons
- "Polish" is unbounded; a rule of thumb (e.g. "30% of total dev time on polish") is the only way to bound it.
- Settings touch every system that has a tunable; refactoring to expose those tunables late is painful — design earlier features with `Resource`-backed config.
- Localization is the giant deferred trap here: an English-only ship is fine for v1; designing localizable text from day zero would have been better but is hard to retrofit.

### What This Touches
- `src/plugins/ui/title_screen.rs`, `src/plugins/ui/settings.rs`, `src/plugins/ui/credits.rs`.
- Touches every plugin with a tunable (audio volumes, encounter rate multiplier, difficulty modifiers).
- Global font + color palette in a `UiTheme` resource.
- `assets/ui/` populated for real, not placeholder.

### Impact Analysis

| Dimension | Baseline | After |
|-----------|----------|-------|
| LOC Δ | ~14150 | +800 to +1500 |
| Deps Δ | 10 | 0 |
| Compile Δ | small | +0.5s |
| Asset Δ | +53-98 | +10-25 (final UI art, fonts, title BGM) |
| UI screens | ~10 | +5-8 (title, settings, credits, victory, game-over, dialogue, tooltips) |

### Broad Todo List

- Implement title screen with new game / continue / load / settings / quit.
- Implement settings: audio volumes, key bindings, video mode, difficulty, QoL toggles (auto-battle, fast-forward, classic-vs-modern encounter rate).
- Implement save-slot UI on continue / load.
- Implement victory + game-over + credits screens.
- Implement a global tooltip system (egui hover helpers).
- Replace placeholder fonts with chosen typography.
- Pass a global color palette over every screen for visual consistency.
- Implement a loading transition between major state changes (covers the fade-to-black during dungeon swaps).
- Final round of egui styling: rounded corners, panel margins, icon sets.

### Additional Notes
This is the catch-all for "ship-readiness". Treat each entry as an independent micro-feature with its own done-or-not bar; don't try to land all of #25 in one PR. Even shipping with placeholder fonts is better than slipping the launch by a month for typography.

---

## Summary Table

| # | Feature | Difficulty | Impact | Effort | LOC Δ | Deps Δ |
|---|---------|------------|--------|--------|-------|--------|
| 1 | Project Skeleton & Plugin Architecture | 1.5/5 | High (foundation) | Low | +150-250 | +1 |
| 2 | Game State Machine | 1.5/5 | High (foundation) | Low | +80-120 | 0 |
| 3 | Asset Pipeline & RON Loading | 2/5 | High | Low | +150-250 | +2 |
| 4 | Dungeon Grid Data Model | 2/5 | High | Low | +250-400 | 0 |
| 5 | Input System (leafwing) | 2/5 | Medium | Low | +120-200 | +1 |
| 6 | Audio System (BGM + SFX) | 2/5 | Medium | Low | +200-350 | +1 |
| 7 | Grid Movement & First-Person Camera | 2.5/5 | Very High | Medium | +250-400 | 0 |
| 8 | 3D Dungeon Renderer (Option B) | 3/5 | Very High | Medium | +300-500 | 0 |
| 9 | Dungeon Lighting & Atmosphere | 2/5 | High | Low | +120-200 | 0 |
| 10 | Auto-Map / Minimap | 2.5/5 | Very High | Medium | +350-500 | +1 (egui) |
| 11 | Party & Character ECS Model | 3/5 | Very High (foundation) | Medium | +500-800 | 0 |
| 12 | Inventory & Equipment | 2.5/5 | High | Medium | +400-600 | 0 |
| 13 | Cell Features (Doors, Traps, Teleporters, Spinners) | 3/5 | High | Medium | +400-700 | 0 |
| 14 | Status Effects System | 2.5/5 | High | Medium | +350-500 | 0 |
| 15 | Turn-Based Combat Core | 4/5 | Very High | Very High | +1000-1800 | 0 |
| 16 | Encounter System & Random Battles | 2.5/5 | High | Medium | +400-600 | +1 (rand) |
| 17 | Enemy Billboard Sprite Rendering | 3/5 | High | Medium | +400-600 | +1 |
| 18 | Town Hub & Services | 3/5 | Very High | High | +800-1300 | 0 |
| 19 | Character Creation & Class Progression | 3/5 | Very High | High | +500-900 | 0 |
| 20 | Spells & Skill Trees | 3.5/5 | High | High | +700-1200 | 0 |
| 21 | Loot Tables & Economy | 3/5 | Very High | Medium | +400-700 | 0 |
| 22 | FOE / Visible Enemies | 3.5/5 | High | Medium | +500-800 | +1 |
| 23 | Save / Load System | 3.5/5 | Very High (mandatory) | High | +600-1000 | +1 |
| 24 | Dungeon Editor Tool | 4/5 | Very High (productivity) | Very High | +1500-2500 | 0 |
| 25 | Title Screen, Settings & End-to-End Polish | 3/5 | High | High | +800-1500 | 0 |

---

## Watch List

| Feature | Risk | Mitigation |
|---------|------|------------|
| #15 Turn-Based Combat Core | Single largest feature; intersects party, status, AI, UI, encounter — high blast radius for design churn | Land in 4-5 sub-PRs (turn manager → damage → AI → UI → polish); keep damage as a pure function; freeze the combat data model before starting AI. |
| #23 Save / Load System | Touches every gameplay component; an architectural mistake here ripples across the codebase (research §Pitfall 5) | Enforce `Serialize/Deserialize` on every component from #11 onward; pick the save crate after evaluating both `moonshine-save` and `bevy_save` against a tiny prototype world before committing. |
| #24 Dungeon Editor Tool | Easy to over-engineer; every hour spent on the editor is an hour not spent on gameplay | Land an MVP (load, edit walls, save) in <2 weeks; expand only when authoring real content forces specific gaps. |
| Bevy version churn (cross-cutting) | Bevy 0.18 → 0.19 will break things mid-development (research §Pitfall 1) | Pin `=0.18.1` and all plugins; treat upgrades as planned 1-2 week sprints, not background drift; abstract Bevy APIs behind project-local helpers where possible. |
| #20 Spells & Skill Trees + #21 Loot | Combinatorial balance explosion (research §Pitfall 6); one bad synergy invalidates a class | Build automated battle simulators that run thousands of fights with random builds; data-drive every formula; start with 3 classes / 15 spells, not 8 classes / 50 spells. |
| `bevy_sprite3d` 0.18 compatibility (#17) | Research §Open Question 2 flags exact 0.18 compat as unverified | Verify on `crates.io` *before* depending; have a 50-LOC manual billboard fallback (textured quad + camera-facing system) ready in case. |

---

## Recommended First Sprint

The first sprint should produce a walkable dungeon: black screen → loading → empty 3D corridor that the player can move and rotate through, with a working state machine and input pipeline underneath. This proves out the foundation and gives every later feature a verified base to build on.

| # | Feature | Justification |
|---|---------|---------------|
| 1 | Project Skeleton & Plugin Architecture | Nothing else can land without this; settles compile-speed config that pays back every single rebuild. |
| 2 | Game State Machine | Cheap to land; every later feature uses `run_if(in_state(...))` and benefits from this being in place from day one. |
| 3 | Asset Pipeline & RON Loading | Unlocks every later data-driven feature; dungeon, items, enemies, classes all flow through this. |
| 4 | Dungeon Grid Data Model | Pure-Rust core that's the most testable foundation in the project; de-risks #7, #8, #10, #13, #16, #22, #24. |
| 7 | Grid Movement & First-Person Camera | The first feature that produces something *playable*; turning research into a working corridor is the single biggest motivation boost in a greenfield project. |

After this sprint you have a black-window-with-loading-state, a verified asset pipeline, a tested grid model, and a corridor you can walk through. The next sprint should add #5 (input refinement), #8 (renderer), #9 (lighting) to make the corridor *feel* like a dungeon, then #10 (auto-map) to give the first piece of player-facing UI.

---

## Dependency Graph

Hard dependencies (B cannot be built without A):

```
1 Skeleton  ──┬─> 2 State Machine
              ├─> 3 Asset Pipeline ──> 4 Grid Data Model ──┬─> 7 Movement ──┬─> 8 Renderer ──> 9 Lighting
              │                                            │                │
              ├─> 5 Input ─────────────────────────────────┘                ├─> 10 Auto-Map
              │                                                             │
              ├─> 6 Audio                                                   └─> 13 Cell Features
              │
              └─> 11 Party Model ──┬─> 12 Inventory ──> 18 Town ──> 19 Char Creation ──> 20 Spells
                                   │                                                       │
                                   ├─> 14 Status Effects ──┐                              │
                                   │                       │                              │
                                   └─────────────> 15 Combat ──┬─> 16 Encounters ──> 22 FOEs
                                                              │   ├─> 17 Enemy Sprites
                                                              │   └─> 21 Loot
                                                              │
                          (everything gameplay-related) ──────┴──> 23 Save/Load
                                                              
                          4 + 13 ─> 24 Editor
                          
                          (all the above) ─> 25 Title/Settings/Polish
```

Soft dependencies:
- #9 Lighting works without #8 Renderer's textures, but feels broken without them.
- #21 Loot can technically ship before #18 Town's shop, but the gold loop is incomplete without somewhere to spend.
- #24 Editor can be started anytime after #4, but it's wasted effort before #13's full feature set is stable.

Conflicts:
- None identified. The plugin-per-subsystem architecture and data-driven content design avoid most architectural collisions. The closest thing to a conflict is `bevy_sprite3d` vs. a hand-rolled billboard implementation in #17 — pick one and stick with it.
