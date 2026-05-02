# Feature #5 — Input System (leafwing-input-manager) — Research

**Researched:** 2026-05-01
**Domain:** Input mapping for Bevy 0.18.1 — `leafwing-input-manager` integration, action-enum design, state-scoped plugin wiring, test simulation, F9 cycler refactor decision
**Confidence:** MEDIUM overall (Bevy 0.18 first-party APIs HIGH from on-disk source; `leafwing-input-manager` MEDIUM from training-data + crates.io conventions, requires one verification recipe before locking `Cargo.toml`)

---

## Tooling Limitation Disclosure (read this first)

This research session ran with only `Read`, `Write`, `Grep`, `Glob`, `Edit`. **No Bash, no MCP servers (despite the `context7` system reminder), no WebFetch, no WebSearch.** Same constraint as Features #1-#4 research sessions.

**HIGH-confidence sources (verified against on-disk source):**

- `bevy_input-0.18.1/src/{lib.rs, keyboard.rs, button_input.rs}` — `InputPlugin`, `keyboard_input_system`, `ButtonInput` API, schedule placement (`PreUpdate`)
- `bevy_app-0.18.1`, `bevy_ecs-0.18.1`, `bevy_state-0.18.1` — `Plugin` trait, `App::add_plugins` shape, `IntoSystemConfigs::run_if`, `States` machinery
- `bevy_reflect-0.18.1` — `#[derive(Reflect)]` enum support (verified in Feature #4 research)
- `Cargo.toml`, `Cargo.lock`, `src/plugins/state/mod.rs` — current project state

**MEDIUM-confidence sources (training data, no live verification possible this session):**

- `leafwing-input-manager` exact published version supporting Bevy 0.18 — **NOT extracted on disk** (verified by `Glob /Users/nousunio/.cargo/registry/**/leafwing*` returning zero hits)
- `Actionlike` trait shape, `InputMap` constructor surface, `ActionState` query path, `MockInput` test API — all from training data; cross-referenced with the prior research §Standard Stack (`research/20260326-01-bevy-first-person-dungeon-crawler-rpg.md:88, 989, 1318`) which cites `leafwing-input-manager 0.18.0` as the Bevy-0.18-compat release

**Why MEDIUM is acceptable here:** The exact crate version IS a fail-stop gate (RQ1) — but the verification recipe (Step A in §RQ1) is a single `cargo add` invocation the implementer/planner can run in seconds. The trait/API shape is recoverable: even if the public surface differs from what's described here, `cargo build` will surface every difference at compile time, and the action-enum / state-scoped wiring concepts are stable across leafwing 0.13–0.18 (no fundamental architectural changes in that window per training data).

**What changes the rating to HIGH:** running the Step A recipe (below) and pasting the output here. The planner SHOULD do this before locking `Cargo.toml`, exactly as Feature #3 did for `bevy_common_assets` and `bevy_asset_loader`.

---

## Recommendation Header (for the planner)

Feature #5 is structurally similar to Feature #3 in shape: one new external crate (`leafwing-input-manager`), gated behind a "Bevy 0.18 compat verification" fail-stop, plus a new plugin module (`src/plugins/input/mod.rs`). The decision surface is **mostly design-call** (which actions in which enum, which keys on which actions, where the F9 cycler ends up) — there is little Bevy-API-shape risk for the input wiring once the crate version is verified.

**Top-level recommendation:**

1. **RQ1 — Verify-before-pin gate.** Run the Step A recipe (`cargo add leafwing-input-manager --dry-run`) once before locking `Cargo.toml`. If a published version supports Bevy `=0.18`, pin with `=`. If not — **escalate to user** with the same `moonshine-save` playbook used for Feature #3 (`Cargo.toml` change does not happen until user picks: wait for upstream / fork / hand-roll). Training-data expectation: `leafwing-input-manager 0.18.0` (released ~2026-03 per the original research §Sources line 1318) supports Bevy 0.18; HIGH-likelihood there is also a `0.18.1` patch by 2026-05-01.

2. **RQ2 — Define `Actionlike` enums per game context.** Four enums: `MenuAction`, `DungeonAction`, `CombatAction`, `DevAction`. **Skip a separate `TownAction` for now** — town navigation is menu-style and can reuse `MenuAction`; a real `TownAction` lands later if/when town gets distinct movement. Each enum derives `Actionlike, PartialEq, Eq, Hash, Clone, Copy, Debug, Reflect` (per leafwing's documented bound; verify the exact bound list against the resolved crate version).

3. **RQ3 — Use `InputMap<T>` as a `Resource`** (the global / per-app shape). Per-entity `Component` `InputMap<T>` is leafwing's newer 0.15+ idiom and has ergonomic upsides (per-player rebinding for couch co-op), but Druum is single-player and the global resource shape needs no spawning entity, no `Single<>` query, and has zero churn for the test simulation pattern. **Reassess only if/when the rebinding settings UI (Feature #25) surfaces a multi-player requirement.**

4. **RQ4 — `InputManagerPlugin::<T>::default()` is registered ONCE at app build time, NOT `.run_if(in_state(...))` on the plugin itself.** Plugins are not run-conditioned in Bevy; only systems are. The state-scoping happens at the **system level inside the consuming plugin** (e.g. a future Feature #7 `handle_dungeon_movement` system has `.run_if(in_state(GameState::Dungeon))`). leafwing's internal systems (the input-mapping update step) run unconditionally in `PreUpdate` after `InputSystems` — this is what we want, because it keeps `ActionState<T>` always-present and queryable, with state-scoped *consumers*. **The roadmap line 314's `.run_if(in_state(GameState::Dungeon))` on the plugin call is wrong API shape — flag this in the plan.**

5. **RQ5 — Consumers read `Res<ActionState<T>>` and call `.just_pressed(Action::X)` / `.pressed(Action::X)`.** Same shape as `ButtonInput<KeyCode>::just_pressed` for digital actions. For axes/dual-axes (none in Druum v1 since gameplay is discrete grid), `.value()` returns `f32`. The query path is identical to `ButtonInput<KeyCode>` from a system-author perspective — replacing `Res<ButtonInput<KeyCode>>` with `Res<ActionState<DungeonAction>>` is a one-line change at every call site.

6. **RQ6 — Test simulation requires the full `InputPlugin` and message-injection (NOT direct `ButtonInput::press`).** Recommended path: (a) add `bevy::input::InputPlugin` + leafwing's `InputManagerPlugin::<T>` + `InputMap<T>` resource to the test app, (b) **inject a `KeyboardInput` message** (or use leafwing's `MockInput::send_input` if it exists in the resolved version) so the `keyboard_input_system` populates `ButtonInput<KeyCode>` naturally, (c) call `app.update()` so leafwing's update system maps `KeyCode::W -> DungeonAction::MoveForward`, (d) assert `app.world().resource::<ActionState<DungeonAction>>().just_pressed(&DungeonAction::MoveForward)`. **Critical:** the F9 cycler test setup pattern from Feature #2 (`init_resource::<ButtonInput<KeyCode>>` instead of `InputPlugin`) does NOT transfer to Feature #5 — for action-state tests, the full `InputPlugin` is required so `KeyboardInput` messages flow to `keyboard_input_system` to fill `ButtonInput<KeyCode>` for leafwing to consume. **Direct `ButtonInput::press` calls race against `keyboard_input_system`'s frame-start clear** — see §RQ6 for the detailed race analysis. The safe pattern is message injection, not resource mutation.

7. **RQ7 — Keep the F9 cycler on `Res<ButtonInput<KeyCode>>` (Option A, no refactor).** The friction of refactoring to `DevAction::CycleGameState` exceeds the value: it forces `InputManagerPlugin::<DevAction>` to be `#[cfg(feature = "dev")]`-gated end-to-end, doubles the test-setup complexity for `f9_advances_game_state`, and adds no rebinding value (F9 is dev-only and never user-rebindable). The "consistency" argument is real but weak — F9 is structurally a dev hotkey, not gameplay. Feature #5 should explicitly document this carve-out so future contributors don't try to "fix the inconsistency". The F9 test pattern (`init_resource::<ButtonInput<KeyCode>>` to bypass `keyboard_input_system`) stays unchanged.

8. **RQ8 — Per-context action enums** (recommendation: 4 enums — `MenuAction`, `DungeonAction`, `CombatAction`, `DevAction`). One mega-enum (`Action::DungeonForward`, `Action::CombatConfirm`, …) is shorter to type but defeats type-safety: a system that should only react to combat input could accidentally handle a dungeon action variant, and `.run_if(in_state(...))` is the only safety net. Per-context enums make this a compile-time error (a system reading `Res<ActionState<DungeonAction>>` in combat code is obviously wrong). Each enum is 6-10 variants; the boilerplate cost is one derive line per enum.

9. **RQ9 — Default keymap follows Wizardry/Etrian conventions** with WASD-or-arrows for movement, QE for turning, Esc for pause/cancel, Enter for confirm. Full matrix in §RQ9. Bind both WASD and arrow keys to dungeon movement (many-to-many is leafwing's selling point — use it). No number-key skill slots in Feature #5 (defers to Feature #15).

10. **RQ10 — Cargo.toml: add `leafwing-input-manager = "=X.Y.Z"` (verify version) with no extra features for v1.** Disable default features only if the resolved crate has heavy default features (e.g. `egui`, `asset`). Training-data expectation: leafwing's defaults are minimal (no UI, no asset). **Do NOT add `bevy_egui` to Druum just because leafwing supports an egui feature** — that's Feature #25's call. **`serde` is already in Druum's `[dependencies]` at version `1`**, so leafwing's `serde` feature (if present) does not double-pull serde — Cargo unifies on the same major.

11. **RQ11 — Bevy 0.18 Event/Message rename.** leafwing predates 0.18 by years; the 0.18-compat release should already use `MessageReader` internally, but if leafwing emits any user-facing reactive types (e.g. an `ActionDiff` event for networking), they MAY be `Event` or `Message` depending on how the maintainers handled the rename. **Mitigation:** Druum v1 does not need to read leafwing events — `Res<ActionState<T>>` polling is sufficient. If a future feature adds event-based input replay or networking, verify the type at that time.

12. **RQ12 — Other 0.18 traps.** Schedule ordering: leafwing's update system MUST run after `InputSystems` (Bevy's keyboard/mouse update) — leafwing handles this in its plugin, but verify with a `.in_set()` chain after first compile. Reflect derive: `Actionlike` should compose with `Reflect` — verify the resolved crate doesn't require `#[reflect(...)]` attributes on data-carrying variants.

**Six things the planner must NOT skip:**

1. **Run the Step A verification recipe (§RQ1) before editing `Cargo.toml`.** This is the same fail-stop gate Feature #3 had for `bevy_common_assets`/`bevy_asset_loader`. If leafwing has not published a Bevy-0.18-compat release, escalate to the user — do not downgrade Bevy.
2. **The roadmap's `.run_if(in_state(GameState::Dungeon))` on the plugin call is wrong** — plugins are not run-conditioned in Bevy. Apply `run_if` to consuming systems only. Document this carve-out in the plan so the implementer doesn't paste roadmap code verbatim.
3. **The F9 cycler stays on `Res<ButtonInput<KeyCode>>`** — Option A in §RQ7. If the planner overrides to Option B, they MUST account for the test-setup complexity blowup (the `init_resource::<ButtonInput<KeyCode>>` workaround in `f9_advances_game_state` will need to become a full `InputPlugin + InputManagerPlugin::<DevAction>` setup, AND the test will need to advance `app.update()` an extra time for leafwing's update system to map the press through to `ActionState<DevAction>`).
4. **Tests need `InputPlugin` for action-state simulation**, NOT `init_resource::<ButtonInput<KeyCode>>()`. This is a different test setup than Feature #2's F9 test. The implementer feedback memory `feedback_bevy_test_input_setup.md` describes the Feature #2 pattern; Feature #5 tests need the OPPOSITE pattern (full plugin) for action-state mapping to occur. Document this distinction in the plan.
5. **Do NOT add `bevy/dynamic_linking` or `bevy/file_watcher` to leafwing's feature flags.** Those are Bevy-only umbrella features for the `dev` Cargo feature. leafwing's own features (e.g. `egui`, `asset`, `serde`) are a separate axis.
6. **`#[cfg(feature = "dev")]` symmetric gating** — every dev-only system definition AND its `add_systems(...)` registration must be cfg-gated. This is the same rule from Feature #2 and Feature #3 (resource: `project/resources/20260501-102842-dev-feature-pattern.md`). Feature #5 inherits it; if any new dev hotkey lands here (none planned for v1 besides keeping F9), follow the rule.

---

## Standard Stack

### Core

| Library | Version | Purpose | License | Maintained? | Why Standard |
| ------- | ------- | ------- | ------- | ----------- | ------------ |
| `leafwing-input-manager` | **MEDIUM-confidence pin: `=0.18.x` (run Step A first; the 2026-03-26 prior research §Sources line 1318 cites `0.18.0` as the Bevy 0.18 release)** | Many-to-many key/mouse/gamepad-to-action mapping, action-state polling, serializable bindings for future settings UI. | MIT/Apache-2.0 (typical; verify on resolve) | Active — Leafwing-Studios maintains; tracks Bevy minor releases | Listed in `research/20260326-01-bevy-first-person-dungeon-crawler-rpg.md:989` §Don't Hand-Roll. De-facto standard for non-trivial Bevy input handling. The alternative (`bevy_enhanced_input`) is younger and Unreal-inspired. |
| `bevy_input` (re-exported via `bevy::input` and `bevy::prelude`) | 0.18.1 | `KeyCode` enum (variant references like `KeyCode::KeyW`), `ButtonInput<KeyCode>` resource (used by leafwing internally and by F9 cycler directly). Already enabled by our `features = ["3d"]`. | MIT/Apache-2.0 | Yes — co-released with Bevy core | Built-in; the underlying primitive leafwing maps from. |
| `bevy_state` (re-exported via `bevy::state` and `bevy::prelude`) | 0.18.1 | `States` / `SubStates` for `.run_if(in_state(GameState::X))` gating on consuming systems. Already in use via Feature #2's `StatePlugin`. | MIT/Apache-2.0 | Yes | Built-in. |
| `bevy_reflect` (re-exported via `bevy::reflect` and `bevy::prelude`) | 0.18.1 | `#[derive(Reflect)]` for action enums — leafwing typically requires this for type registration. Already enabled by 3d feature. | MIT/Apache-2.0 | Yes | Built-in. |

### Supporting

| Library | Version | Purpose | When to Use |
| ------- | ------- | ------- | ----------- |
| `serde` | `1` (already in `[dependencies]`) | Future RON-based binding files (Feature #25 settings UI). NOT used in Feature #5 itself — bindings are code-defined here. | Listed for forward compatibility; no new usage in Feature #5. |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
| ---------- | --------- | -------- |
| `leafwing-input-manager` | `bevy_enhanced_input` | Enhanced input is Unreal-inspired, observer-based, and younger. Better for input chains/contexts/conditions in complex action games. Druum's "press a key, do a thing" model maps cleanly to leafwing's action-state polling. (Original research §Alternatives line 112.) |
| `leafwing-input-manager` | Direct `Res<ButtonInput<KeyCode>>` everywhere (status quo) | Status quo works but requires per-system key-to-game-action mapping code. Rebinding becomes a per-system rewrite. Listed in `research/20260326-01-bevy-first-person-dungeon-crawler-rpg.md:989` §Don't Hand-Roll explicitly because "many-to-many input mapping, gamepad support, serializable bindings — surprisingly complex to get right." |
| Per-context action enums | One mega-enum (`Action::*`) | Mega-enum is fewer types but defeats compile-time scoping. A system reading `Res<ActionState<Action>>` in combat code can accidentally handle dungeon actions. Per-context enums make state-scoping a type-system property. |
| `InputMap<T>` as `Resource` (global) | `InputMap<T>` as `Component` (per-entity) | Per-entity is leafwing's 0.15+ idiom and supports per-player rebinding (couch co-op). Druum is single-player; the resource shape has no spawning entity, simpler tests, and zero ergonomic loss. Reassess at Feature #25 if multi-player is ever scoped. |

**Installation:**

```bash
# Step A (verification — run BEFORE editing Cargo.toml):
cd /Users/nousunio/Repos/Learnings/claude-code/druum
cargo add leafwing-input-manager --dry-run
# Read the resolved version. Confirm its Cargo.toml requires `bevy = "0.18"` or
# `bevy = ">=0.18, <0.19"` (i.e. supports our pinned 0.18.1).
# If the resolved version requires bevy <0.18 → escalate to user.

# Step B (lock with `=` after verifying):
cargo add leafwing-input-manager@=<RESOLVED-VERSION>

# Step C (post-add — confirm Cargo.lock + verification commands still pass):
cargo check
cargo check --features dev
cargo clippy --all-targets -- -D warnings
cargo clippy --all-targets --features dev -- -D warnings
cargo test
cargo test --features dev
```

---

## Architecture Options

The roadmap surfaces three orthogonal organisational choices: where the input code lives, the action enum granularity, and the F9 cycler's fate. They're independent and each has a clear winner under Druum's constraints.

### Option 1: Plugin module location

| Option | Description | Pros | Cons | Best When |
| ------ | ----------- | ---- | ---- | --------- |
| **A — `src/plugins/input/mod.rs`** | New plugin sibling to `state/`, `dungeon/`, etc. | Matches the established 9-flat-plugin pattern from Features #1-#3. Greppable as `InputPlugin`. Trivially adds to `main.rs::add_plugins(...)`. **Naming clash with `bevy::input::InputPlugin`** — name ours `DruumInputPlugin` or `ActionsPlugin` to disambiguate. | Adds one more plugin to the tuple. | Always — the project's plugin convention is established. |
| B — `src/input.rs` at crate root (per roadmap §What This Touches line 313) | Single file, no `mod.rs`. | Slightly fewer files. | Inconsistent with established pattern (every other plugin is `src/plugins/<name>/mod.rs`). Not greppable as a directory. | Single-system features that don't warrant a folder. Feature #5 will likely grow (rebinding UI later). |
| C — Define enums in each consuming plugin (`DungeonAction` in `src/plugins/dungeon/`, `CombatAction` in `src/plugins/combat/`) | Close to use site. | Each enum lives where it's consumed. | Splits `InputManagerPlugin::<T>` registrations across plugins; harder to see all bindings in one place; the F9-style "all input in one file" scan becomes a multi-file scan. | If/when input becomes very plugin-specific (rare). |

**Recommendation:** **Option A** with the plugin named `ActionsPlugin` (avoids the `InputPlugin` clash and reads as "this owns the action enums"). All four action enums (`MenuAction`, `DungeonAction`, `CombatAction`, `DevAction`) and their `InputMap<T>` defaults live in this one module. Consuming plugins (Dungeon, Combat, etc.) only import the enum types; they do NOT call `app.add_plugins(InputManagerPlugin::<T>)`. This keeps the registration site singular and greppable.

### Counterarguments to Option A

- **"`InputManagerPlugin` should live next to the systems that consume it, not in a central module."** Counter: the `InputManagerPlugin::<T>` for an action enum is *not* state-scoped (per RQ4) and runs unconditionally — it doesn't belong to one game-state plugin. Centralizing in `ActionsPlugin` matches the "one plugin owns the registration" model that's already used for `StatePlugin`.
- **"Naming `ActionsPlugin` is bikeshedding; `InputPlugin` is fine if we just `use druum::plugins::input::InputPlugin as DruumInputPlugin`."** Counter: import aliases hide intent. `ActionsPlugin` is one word and unambiguous; `InputPlugin` would collide with `bevy::input::InputPlugin` in any file that imports `bevy::prelude::*` AND our plugin, requiring per-file aliasing. Consistency matters more than minor naming flexibility here.

### Option 2: Action enum granularity

| Option | Description | Pros | Cons | Best When |
| ------ | ----------- | ---- | ---- | --------- |
| **A — Per-context enums** (`MenuAction`, `DungeonAction`, `CombatAction`, `DevAction`) | One enum per game-state context, ~6-10 variants each. | Type-system enforces scoping: a combat system reading `Res<ActionState<DungeonAction>>` is a compile error, not a runtime bug. Matches leafwing's documented multi-action-set pattern. Each `InputManagerPlugin::<T>` is registered separately, so adding a new context is additive. | 4× the boilerplate (4 enums, 4 InputMap defaults, 4 plugin registrations). | Game has multiple distinct input contexts with non-overlapping action vocabularies — exactly Druum's case. |
| B — One mega-enum (`Action::DungeonForward`, `Action::CombatConfirm`, …) | Single enum, ~25 variants. | Less boilerplate. One InputMap, one plugin registration. | Type-system gives no scoping; misuse is runtime-only. Bindings file (future) is one giant table. Scaling: every new feature adds variants to the same enum. | Simple games with a single input context (clicker-style). |
| C — Hierarchical enum (`Action::Dungeon(DungeonAction)`, `Action::Combat(CombatAction)`) | Outer enum tags the context, inner enum is the action. | Both worlds — scoping at the inner enum, single registration at outer. | leafwing's `Actionlike` derive may not handle nested enums cleanly (training-data MEDIUM); requires verification. Adds a layer of indirection at every call site (`actions.just_pressed(Action::Dungeon(DungeonAction::MoveForward))`). | If leafwing supports it natively. Probably not worth the complexity. |

**Recommendation:** **Option A** — per-context enums. The 4× boilerplate is one-time and the type-safety win is permanent.

### Counterarguments to Option A

- **"4× boilerplate for ~8 variants per enum is silly."** Counter: derive macros do the heavy lifting; the actual hand-written code is ~8 lines per enum (the enum body) plus ~10 lines per `InputMap` default. Total Feature #5 LOC is well under the +120-200 budget.
- **"The state-scoping enforcement is illusory because we still need `.run_if(in_state(GameState::X))` on the consuming systems."** Counter: yes, but the type system *prevents the wrong enum from being read at all*. The `run_if` is the second line of defense (and prevents action-state-stale-from-last-frame races); type-scoping is the first.
- **"What if a future feature has a hotkey that should work in BOTH dungeon and combat (e.g. open inventory)?"** Counter: that's two action enum variants (`DungeonAction::OpenInventory`, `CombatAction::OpenInventory`) bound to the same KeyCode. leafwing handles this natively — multiple action enums can share a KeyCode. The system handlers in dungeon vs combat plugins each react independently. This is in fact one of leafwing's selling points.

### Option 3: F9 cycler — keep direct vs refactor

(Full RQ7 analysis below.) Recommendation: **Option A — keep direct `Res<ButtonInput<KeyCode>>`.** Detailed trade-offs in §RQ7.

---

## Architecture Patterns

### Recommended Project Structure

```
src/
├── plugins/
│   ├── input/
│   │   └── mod.rs        # NEW — ActionsPlugin: 4 enums + 4 InputMap defaults + 4 InputManagerPlugin::<T> registrations
│   ├── state/
│   │   └── mod.rs        # UNCHANGED — F9 cycler stays on Res<ButtonInput<KeyCode>>
│   ├── dungeon/mod.rs    # UNCHANGED for Feature #5 — Feature #7 will add a system reading Res<ActionState<DungeonAction>>
│   ├── combat/mod.rs     # UNCHANGED for Feature #5 — Feature #15 will add a system reading Res<ActionState<CombatAction>>
│   ├── audio/, loading/, party/, save/, town/, ui/ — UNCHANGED
│   └── mod.rs            # ADD `pub mod input;`
├── data/                 # UNCHANGED
└── main.rs               # ADD ActionsPlugin to add_plugins(...) tuple
```

### Pattern 1: ActionsPlugin (the one new module)

**What:** A single plugin that owns all four action enums, their default keybindings, and all four `InputManagerPlugin::<T>` registrations.

**When to use:** This is the only place input-mapping registrations should happen in Druum. Consumer plugins (Dungeon, Combat, etc.) import the enum types but never call `app.add_plugins(InputManagerPlugin::<T>)`.

**Example (conceptual — exact API surface to verify against the resolved crate version, see §RQ1):**

```rust
// Source: synthesis of leafwing-input-manager training-data README patterns
//          + Bevy 0.18 plugin convention from src/plugins/state/mod.rs
//          + Druum naming convention from src/plugins/state/mod.rs

use bevy::prelude::*;
use leafwing_input_manager::prelude::*;

#[derive(Actionlike, PartialEq, Eq, Hash, Clone, Copy, Debug, Reflect)]
pub enum MenuAction {
    Up,
    Down,
    Left,
    Right,
    Confirm,
    Cancel,
}

#[derive(Actionlike, PartialEq, Eq, Hash, Clone, Copy, Debug, Reflect)]
pub enum DungeonAction {
    MoveForward,
    MoveBackward,
    StrafeLeft,
    StrafeRight,
    TurnLeft,
    TurnRight,
    Interact,
    OpenInventory,
    OpenMap,
    Pause,
}

#[derive(Actionlike, PartialEq, Eq, Hash, Clone, Copy, Debug, Reflect)]
pub enum CombatAction {
    Up,
    Down,
    Left,
    Right,
    Confirm,
    Cancel,
}

#[derive(Actionlike, PartialEq, Eq, Hash, Clone, Copy, Debug, Reflect)]
pub enum DevAction {
    // Reserved for future dev-only hotkeys via leafwing.
    // F9 cycler stays on Res<ButtonInput<KeyCode>> per §RQ7 — no DevAction variant for it.
    // This enum is registered behind #[cfg(feature = "dev")] so it doesn't ship in release.
    DebugDummy, // placeholder; remove when first real DevAction variant lands
}

pub struct ActionsPlugin;

impl Plugin for ActionsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            InputManagerPlugin::<MenuAction>::default(),
            InputManagerPlugin::<DungeonAction>::default(),
            InputManagerPlugin::<CombatAction>::default(),
        ))
        .insert_resource(default_menu_input_map())
        .insert_resource(default_dungeon_input_map())
        .insert_resource(default_combat_input_map());

        #[cfg(feature = "dev")]
        app.add_plugins(InputManagerPlugin::<DevAction>::default())
            .insert_resource(default_dev_input_map());
    }
}

fn default_dungeon_input_map() -> InputMap<DungeonAction> {
    use DungeonAction::*;
    InputMap::default()
        // Movement: WASD AND arrow keys (many-to-many is leafwing's selling point).
        .insert(MoveForward,    KeyCode::KeyW)
        .insert(MoveForward,    KeyCode::ArrowUp)
        .insert(MoveBackward,   KeyCode::KeyS)
        .insert(MoveBackward,   KeyCode::ArrowDown)
        .insert(StrafeLeft,     KeyCode::KeyA)
        .insert(StrafeLeft,     KeyCode::ArrowLeft) // BUT see counterargument below — arrows could mean "turn"
        .insert(StrafeRight,    KeyCode::KeyD)
        .insert(StrafeRight,    KeyCode::ArrowRight)
        .insert(TurnLeft,       KeyCode::KeyQ)
        .insert(TurnRight,      KeyCode::KeyE)
        .insert(Interact,       KeyCode::Space)
        .insert(OpenInventory,  KeyCode::Tab)
        .insert(OpenMap,        KeyCode::KeyM)
        .insert(Pause,          KeyCode::Escape)
        .build()  // .build() may or may not be needed depending on the version's API
}

// (Other default_*_input_map functions follow the same shape.)
```

### Pattern 2: Consumer system (Feature #7+ pattern, NOT in Feature #5 scope)

**What:** A system in a feature plugin that reads `Res<ActionState<T>>` to drive game logic. State-scoping happens here via `.run_if(in_state(...))`.

**When to use:** Every gameplay system that needs input. Replace any `Res<ButtonInput<KeyCode>>` reads with `Res<ActionState<<context>Action>>`.

**Example (illustrative — Feature #7 will write the real version):**

```rust
// In src/plugins/dungeon/movement.rs (Feature #7 — NOT Feature #5)
use bevy::prelude::*;
use leafwing_input_manager::prelude::*;
use crate::plugins::input::DungeonAction;

pub fn handle_dungeon_movement(
    actions: Res<ActionState<DungeonAction>>,
    // ... other params: query for player position, etc.
) {
    if actions.just_pressed(&DungeonAction::MoveForward) {
        // queue forward step
    }
    if actions.just_pressed(&DungeonAction::TurnLeft) {
        // queue turn-left
    }
    // ...
}

// And in DungeonPlugin::build:
app.add_systems(
    Update,
    handle_dungeon_movement.run_if(in_state(GameState::Dungeon)),
);
```

### Anti-Patterns to Avoid

- **Calling `InputManagerPlugin::<T>::default().run_if(in_state(...))`.** Plugins are not run-conditioned in Bevy. The roadmap line 314 wording is wrong API shape; flag this in the plan and apply `run_if` to consuming systems only.
- **Reading `ButtonInput<KeyCode>` directly in gameplay code after Feature #5 ships.** Defeats the rebinding story for Feature #25. Only F9 (a dev-only hotkey, never user-rebindable) should read `ButtonInput<KeyCode>` directly.
- **Putting `InputManagerPlugin::<T>` registrations in consumer plugins** (e.g. `DungeonPlugin::build`). Splits the input-registration story across files. Centralize in `ActionsPlugin`.
- **Using one mega-Action enum** to "save boilerplate." Defeats type-scoping. Per-context enums are the leafwing-idiomatic approach.
- **Writing `if actions.pressed(DungeonAction::MoveForward)` for "move once per press" logic.** `.pressed()` is true-while-held; you want `.just_pressed()` for one-shot actions. (Same trap as `ButtonInput::pressed` vs `just_pressed`.)
- **Forgetting `Reflect` on the action enum.** Some leafwing versions require it for type registration; others don't. Always include it for forward-compatibility.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
| ------- | ----------- | ----------- | --- |
| Key-to-action mapping | Per-system `match key { KeyCode::W => move_forward(), ... }` | `leafwing-input-manager` | Many-to-many bindings, gamepad, rebinding all need to be re-implemented per system. Listed in `research/20260326-01-bevy-first-person-dungeon-crawler-rpg.md:989` §Don't Hand-Roll. |
| Input simulation in tests | Custom event injection | `ButtonInput<KeyCode>::press(...)` (Bevy built-in) + `app.update()` | Bevy's primitive already exposes a press/release API. leafwing's update system reads from `ButtonInput<KeyCode>`, so simulating at that layer naturally exercises leafwing too. |
| Modifier chord handling (Shift+S, Ctrl+M) | Custom shift-state tracking | `ChordedInput::modified(KeyCode::ShiftLeft, KeyCode::KeyS)` (leafwing) | Single API, handles both shift sides, no race conditions. (Training-data MEDIUM — verify leafwing's exact chord API on resolve.) |
| Future settings UI rebinding storage | Manual key-to-action HashMap | `InputMap<T>` Serialize/Deserialize derives | Already in the crate. Feature #25 work becomes "edit the resource", not "rewrite the input layer". |

---

## Common Pitfalls

### Pitfall 1: Plugin run_if (Bevy-wide, not leafwing-specific)

**What goes wrong:** Reading the roadmap line 314 verbatim, the implementer writes `app.add_plugins(InputManagerPlugin::<DungeonAction>::default().run_if(in_state(GameState::Dungeon)))`. Compile error or silent no-op (depending on Bevy version's plugin shape).

**Why it happens:** `run_if` is on `IntoSystemConfigs`, not on `Plugin`. Plugins always build into the app unconditionally; only systems can be conditioned. The roadmap was written before this distinction was nailed down for Druum.

**How to avoid:** Register `InputManagerPlugin::<T>` unconditionally in `ActionsPlugin::build`. Apply `run_if` to the *consuming* gameplay systems in their respective plugins (Feature #7 for dungeon, Feature #15 for combat).

### Pitfall 2: Test setup divergence from Feature #2

**What goes wrong:** Implementer copies the Feature #2 pattern (`init_resource::<ButtonInput<KeyCode>>()` instead of `InputPlugin`) for an action-state test, and `ActionState<DungeonAction>::just_pressed` returns false even after pressing the bound key.

**Why it happens:** The Feature #2 pattern *bypasses* `keyboard_input_system` to keep manually-set `just_pressed` alive into `Update`. But leafwing's update system READS `ButtonInput<KeyCode>` and translates it into `ActionState<T>`. With the bypass, leafwing has no way to know the press happened from a real input chain — actually, more precisely: pressing via `ButtonInput::press` directly does flow into `pressed`/`just_pressed` (the F9 cycler test proves it works at the `ButtonInput` layer), but leafwing's update may run BEFORE the user's manual press because schedules run in a fixed order. Or it may run only if its expected `InputSystems` set is present.

**How to avoid:** For Feature #5 tests of `ActionState<T>`, use the *full* `InputPlugin` plus leafwing's `InputManagerPlugin::<T>`, then press via `ButtonInput::press`, then `app.update()` twice (once for leafwing's update to consume the press, once for downstream systems to react). For F9 tests (Feature #2), the existing `init_resource::<ButtonInput<KeyCode>>()` pattern stays — F9 doesn't go through leafwing.

### Pitfall 3: ActionState query path on per-entity InputMap

**What goes wrong:** If the planner or implementer chooses per-entity `InputMap<T>` instead of resource (the leafwing 0.15+ idiom), the consumer query becomes `Query<&ActionState<DungeonAction>>` and tests need to spawn an entity with the `InputMap` and `ActionState` components. F9 cycler tests (which don't go through leafwing) are unaffected.

**How to avoid:** Stick with `Resource` shape for `InputMap<T>`, per the Recommendation. Document this in the plan so the implementer doesn't drift.

### Pitfall 4: Bevy 0.18 Event/Message rename

**What goes wrong:** A future feature wants to react to leafwing's "action diff" events (e.g. for networking or replay) and writes `EventReader<ActionDiff<T>>`. Compile error in Bevy 0.18 because the type might be a `Message`, not an `Event`.

**How to avoid:** Druum v1 polls `Res<ActionState<T>>` and never reads leafwing events directly. If a future feature needs event reading, verify the type kind on the resolved leafwing version (`grep "derive(Message" leafwing/src/`) and use `MessageReader` if it's a `Message`. Same trap that bit Feature #2 (`StateTransitionEvent`) and was avoided in Feature #3 (`AssetEvent`).

### Pitfall 5: Forgetting that some leafwing APIs take `&Action` references, not `Action` by value

**What goes wrong:** `actions.just_pressed(DungeonAction::MoveForward)` — compile error in newer leafwing versions that take `&T` instead of `T`. Confusion because Bevy's `ButtonInput::just_pressed` takes by value.

**How to avoid:** When writing the consumer system code in Feature #7+, check the resolved version's signature: `fn just_pressed(&self, action: &T) -> bool` vs `fn just_pressed(&self, action: T) -> bool`. The example in Pattern 2 above uses `&DungeonAction::MoveForward` defensively — adjust to value-shape if the resolved API takes by value. Either way, the implementer will see the compile error immediately.

### Pitfall 6: "Default features" of leafwing pulling in egui or other heavy deps

**What goes wrong:** `cargo add leafwing-input-manager` enables default features which include `egui` (for binding-display widgets) or `asset` (for RON-bound input maps). Druum doesn't yet have `bevy_egui`; pulling it in transitively bloats build time.

**How to avoid:** After Step A, inspect the resolved `Cargo.toml` `[features]` block and `default = [...]`. If defaults include features Druum doesn't need, write the dep with `default-features = false` and explicit feature opt-in. Training-data expectation: leafwing's defaults are minimal; verify on resolve.

---

## Security

### Known Vulnerabilities

| Library | CVE / Advisory | Severity | Status | Action |
| ------- | -------------- | -------- | ------ | ------ |
| `leafwing-input-manager` | None known to training data; original research §Security line 1092 ("Monitor"). | — | — | **Run `cargo audit` after Step B and check `https://rustsec.org/advisories/`.** Verify on resolve. |
| `bevy_input` 0.18.1 | None | — | — | Tracks Bevy's own audit cadence. |

(No new architectural security risks introduced by Feature #5 — input mapping is a local-machine concern with no network or cross-trust-boundary surface area. Future settings UI for rebinding (Feature #25) adds a file-write surface — that's Feature #25's risk, not this one's.)

### Architectural Security Risks

| Risk | Affected Architecture Options | How It Manifests | Secure Pattern | Anti-Pattern to Avoid |
| ---- | ----------------------------- | ---------------- | -------------- | --------------------- |
| Untrusted binding file (Feature #25 future) | If/when bindings load from RON | Malicious RON could specify a binding for a non-existent action variant or name an unknown KeyCode. | Use serde with explicit deserializer + a validator that maps known variants. Reject unknown identifiers. | Loading raw user-supplied RON without validation. |

### Trust Boundaries

For Feature #5 itself: **none.** All bindings are code-defined. Settings UI (Feature #25) will introduce a file-load boundary; flag at that time.

---

## Performance

| Metric | Value / Range | Source | Notes |
| ------ | ------------- | ------ | ----- |
| Per-frame action-state update cost | O(n_actions × n_bindings_per_action) per registered enum, typically <1µs for druum's scale | Training-data MEDIUM — leafwing benchmarks not present in research | Druum's largest enum is `DungeonAction` with ~10 variants × ~2 bindings = 20 ops. Negligible. |
| Crate compile time | +0.5-1s clean (per roadmap §Impact line 322) | Roadmap | Roughly aligned with `bevy_common_assets` add in Feature #3 (~0.5s observed). |
| Binary size impact | ~50-100 KB stripped (training-data MEDIUM) | Estimation based on similar-sized crate footprints | Acceptable; well below profiling threshold. |
| `InputMap<T>` resource memory | O(n_actions × n_bindings × ~16 bytes) per enum | Estimation | <10 KB for Druum's full binding set. Negligible. |

_(No domain-specific benchmarks found in available sources — flag for validation during implementation if performance becomes a concern. For Druum's input volume (~handful of presses per second, never input-bound), this is HIGH-likelihood non-issue.)_

---

## Code Examples

The illustrative ActionsPlugin and consumer-system examples in §Pattern 1 and §Pattern 2 above are conceptual and depend on the resolved leafwing API shape. After Step A, the implementer should sanity-check by reading the resolved crate's `examples/` and `README.md` and adjusting the exact method names (`.insert` vs `.add` vs `.bind`, `.build` may or may not exist, etc.).

### Verifying the F9 cycler still works after Feature #5

**Source pattern:** `src/plugins/state/mod.rs:117-137` (existing test).

The F9 test does NOT change in Feature #5 because the F9 cycler keeps reading `Res<ButtonInput<KeyCode>>` directly (per §RQ7). The only Feature #5 effect on the F9 test is: if `ActionsPlugin` is added to the test app (it shouldn't be — F9 test is `StatePlugin`-scoped only), it would change the test setup. The recommended split:

- F9 test (`f9_advances_game_state`): no `ActionsPlugin`, no `InputPlugin`, just `init_resource::<ButtonInput<KeyCode>>()` (existing pattern).
- ActionState tests (new in Feature #5): add `InputPlugin` + `ActionsPlugin`, press via `ButtonInput::press`, assert `ActionState::just_pressed`.

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
| ------------ | ---------------- | ------------ | ------ |
| `Res<Input<KeyCode>>` (Bevy ≤0.12) | `Res<ButtonInput<KeyCode>>` (Bevy 0.13+) | Bevy 0.13 | Just a rename; same API surface. The F9 cycler in Druum already uses the new name. |
| Per-system manual key-to-action `match` | leafwing `Actionlike` enum + `InputMap<T>` | leafwing 0.6 (~2022) | Druum's adoption now (Feature #5) follows the established pattern. |
| `InputMap<T>` as Resource only | `InputMap<T>` as Resource OR Component (per-entity) | leafwing 0.15+ | Druum stays on Resource (single-player). |
| Action enum derives `Actionlike` only | `Actionlike + Reflect + serde::{Serialize, Deserialize}` for full settings-UI support | Gradual | Druum's plan: include `Reflect` from day one; add serde derives only when Feature #25 lands. |

**Deprecated / outdated:**

- Anything from a `bevy 0.12 + leafwing 0.10` era blog post — three full Bevy minors stale. Action enum derive shape has changed (added `Reflect`, possibly `Eq + Hash` requirement details). Consult docs.rs latest, not blog posts.

---

## Validation Architecture

### Test Framework

| Property | Value |
| -------- | ----- |
| Framework | Cargo's built-in test (`#[cfg(test)] mod tests`) — same as Features #1-#4 |
| Config file | None — test discovery is automatic. Per-file modules in `src/plugins/<name>/mod.rs`. |
| Quick run command | `cargo test --features dev` |
| Full suite command | All 6 verification commands (per project standard): `cargo check`, `cargo check --features dev`, `cargo clippy --all-targets -- -D warnings`, `cargo clippy --all-targets --features dev -- -D warnings`, `cargo test`, `cargo test --features dev` |

### Requirements → Test Map

| Requirement | Behavior | Test Type | Automated Command | File Exists? |
| ----------- | -------- | --------- | ----------------- | ------------ |
| ActionsPlugin compiles and registers all 4 enums | Plugin builds without panic in `App::new().add_plugins((StatesPlugin, ActionsPlugin))` test | unit | `cargo test --package druum actions_plugin_builds` | needs creating |
| Default DungeonAction binding maps W → MoveForward | Press `KeyCode::KeyW` → `ActionState<DungeonAction>::just_pressed(MoveForward)` returns true | integration (App-level) | `cargo test --package druum dungeon_w_maps_to_move_forward` | needs creating |
| Many-to-many: arrow Up also maps to MoveForward | Press `KeyCode::ArrowUp` → same action just_pressed | integration | `cargo test --package druum dungeon_arrow_up_maps_to_move_forward` | needs creating |
| F9 cycler test (Feature #2) still passes after ActionsPlugin lands | Existing `f9_advances_game_state` runs unchanged | unit | `cargo test --package druum --features dev f9_advances_game_state` | exists at `src/plugins/state/mod.rs:117` |
| MenuAction Confirm bound to Enter | Press Enter → `ActionState<MenuAction>::just_pressed(Confirm)` true | integration | `cargo test --package druum menu_enter_maps_to_confirm` | needs creating |
| CombatAction Cancel bound to Escape | Press Esc → `ActionState<CombatAction>::just_pressed(Cancel)` true | integration | `cargo test --package druum combat_escape_maps_to_cancel` | needs creating |
| `cargo clippy --all-targets -- -D warnings` passes (no `#[cfg(feature = "dev")]` asymmetry) | Full clippy clean | static | `cargo clippy --all-targets -- -D warnings` | enforced by CI convention |
| `cargo clippy --all-targets --features dev -- -D warnings` passes | Full clippy clean under dev | static | `cargo clippy --all-targets --features dev -- -D warnings` | enforced by CI convention |

### Gaps (files to create before / during implementation)

- [ ] `src/plugins/input/mod.rs` — the new `ActionsPlugin` and 4 action enums + 4 `InputMap` defaults
- [ ] Inline `#[cfg(test)] mod tests` in `src/plugins/input/mod.rs` — covers all "ActionState" tests above
- [ ] One-line addition to `src/plugins/mod.rs`: `pub mod input;`
- [ ] One-line addition to `src/main.rs::add_plugins(...)` tuple: `input::ActionsPlugin,`

_(No new test config; cargo's built-in test infrastructure suffices, matching Features #1-#4.)_

---

## RQ1 — leafwing-input-manager Bevy 0.18.1 compatibility

### Confidence: MEDIUM (training data + prior research; HIGH after running Step A)

### What we know

- The original Druum research dated 2026-03-26 cites `leafwing-input-manager 0.18.0` as the Bevy-0.18 release (`research/20260326-01-bevy-first-person-dungeon-crawler-rpg.md:88, 1318`). That research's evidence quality for that specific cell was not directly verifiable in this session (no live crates.io access).
- leafwing's release naming convention historically aligns the *minor* version with the supported Bevy minor (e.g. `leafwing-input-manager 0.13.x` for Bevy 0.13, `0.14.x` for Bevy 0.14, …). On this convention, `0.18.x` is the expected Bevy-0.18 release line.
- **leafwing-input-manager is NOT extracted on disk** at `/Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/`. This was verified by `Glob /Users/nousunio/.cargo/registry/**/leafwing*` returning zero hits.
- Today's date (per system context) is 2026-05-01. Bevy 0.18.0 shipped 2026-03-04 (per Druum research §Standard Stack). That's ~2 months — a comfortable window for leafwing to have released its 0.18-compatible version.

### What we DON'T know without external verification

- Whether the published version is `0.18.0`, `0.18.1`, `0.19.x` (if leafwing fast-tracked), or something else.
- Whether the `bevy = "..."` requirement in leafwing's Cargo.toml is `"^0.18"` (accepts 0.18.1 ✓), `"=0.18.0"` (rejects 0.18.1 ✗), or `">=0.18, <0.19"` (accepts 0.18.1 ✓).
- Whether there are any in-flight 0.18 PRs/branches if no released version supports 0.18 yet (extremely unlikely given the timeline; flagged for completeness).

### Verification recipe (Step A — run once, paste result)

```bash
cd /Users/nousunio/Repos/Learnings/claude-code/druum

# Show what version cargo would resolve (no Cargo.toml changes yet):
cargo add leafwing-input-manager --dry-run 2>&1 | tee /tmp/leafwing-resolve.txt

# Confirm the resolved version's bevy dep:
# (Replace <RESOLVED-VERSION> with the version cargo resolved.)
cargo info leafwing-input-manager --version <RESOLVED-VERSION> 2>&1 | grep -E "bevy"
```

**Expected outcome (HIGH-likelihood):** `leafwing-input-manager 0.18.x` (some `x` ≥ 0) with `bevy = "0.18"` requirement.

**If the resolved version requires `bevy = "0.17"` or older:** Halt. Escalate to user with the same `moonshine-save` playbook (Feature #3 Decision §Resolved #3, 2026-04-29). Options to surface: (a) wait for an upstream 0.18-compat release, (b) use a fork (e.g. a community PR), (c) hand-roll a minimal `Actionlike` + `InputMap` + `ActionState` (~150-300 LOC; same complexity rough order as `bevy_common_assets`'s ~30 LOC reach for `RonAssetPlugin` but more involved because input mapping has more surface area).

**If the resolved version requires `bevy = "0.19"` or newer:** Same halt — leafwing has fast-tracked past Bevy 0.18 and dropped 0.18 support. Escalate.

### Pin format (after verification)

```toml
# In Druum's Cargo.toml [dependencies]
leafwing-input-manager = "=<RESOLVED-VERSION>"  # use `=` per project convention (see Cargo.toml lines 22-23, 27)
```

### Sources

- `research/20260326-01-bevy-first-person-dungeon-crawler-rpg.md:88, 1318` — prior research citing 0.18.0 for Bevy 0.18.
- `Cargo.toml:22-23, 27` — Druum's pinning convention (`=` for all external deps).
- Glob check: `/Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/leafwing*` returns zero results (confirms no on-disk source for direct verification).

---

## RQ2 — Action trait + enum design

### Confidence: MEDIUM (training data + leafwing convention; HIGH after Step A read of the resolved crate's docs)

### Trait derive list

For each action enum (`MenuAction`, `DungeonAction`, `CombatAction`, `DevAction`):

```rust
#[derive(Actionlike, PartialEq, Eq, Hash, Clone, Copy, Debug, Reflect)]
```

**Each derive's role:**

| Derive | Required by | Purpose |
| ------ | ----------- | ------- |
| `Actionlike` | leafwing's input-map machinery | Marker trait the macro provides; turns the enum into a leafwing action-set. |
| `PartialEq, Eq, Hash` | leafwing's internal `HashMap<Action, ActionData>` storage | Same trait bound family as Bevy's `States` derive (also requires PartialEq + Eq + Hash). |
| `Clone, Copy` | leafwing's API takes actions by value at many sites | `Copy` is cheap for unit-variant enums; both are typical. |
| `Debug` | log output, panic messages | Standard. |
| `Reflect` | leafwing's type registration (some versions) | Bevy-idiomatic; matches Druum's existing pattern (Feature #4 stub asset types all derive Reflect). Verified `Reflect` derive auto-supports unit enum variants per `reference_bevy_reflect_018_derive.md`. |

**Notes:**

- **No serde derives in Feature #5.** The settings UI (Feature #25) will need them; add at that time.
- **Variants must be unit variants only** for the simplest leafwing use. Data-carrying variants (e.g. `MoveTo(GridPos)`) are NOT typical leafwing patterns and may not compose with the `Actionlike` derive. Stick to unit variants. (Training-data MEDIUM — verify on resolve.)
- **`#[actionlike(...)]` attribute** for axis or dual-axis grouping: only relevant if Druum had analog input (gamepad sticks). For keyboard-only v1, no `#[actionlike(...)]` attributes needed. Defer for v2 if gamepad support lands.
- **Separate trait flavors for digital vs analog:** in newer leafwing versions, `Actionlike` is the unified trait and the `.value()` accessor returns `f32` (1.0 for digital, smooth for analog). `.just_pressed()` and `.pressed()` work on digital. Keyboard-only Druum uses the digital path exclusively.

### Why no `Default` derive

Unlike `States` enums (which require `Default` because Bevy derives an initial value), action enums do NOT need `Default` — there's no concept of a "default action." Omit `Default` and one fewer derive.

### Sources

- Training-data recall of leafwing 0.13-0.17 README patterns. Cross-checked against `research/20260326-01-bevy-first-person-dungeon-crawler-rpg.md:88` "many-to-many input-action mapping" description.
- `bevy_reflect-0.18.1/` enum derive support verified in `.claude/agent-memory/researcher/reference_bevy_reflect_018_derive.md`.
- `bevy_state-0.18.1/` States derive bound list (`PartialEq + Eq + Hash`) in `bevy_state-0.18.1/src/state/states.rs` — same bound family as leafwing requires.

---

## RQ3 — InputMap construction and binding

### Confidence: MEDIUM (training-data API; HIGH after reading resolved crate)

### Constructor + binding pattern (training-data shape)

```rust
let map = InputMap::default()
    .insert(DungeonAction::MoveForward, KeyCode::KeyW)
    .insert(DungeonAction::MoveForward, KeyCode::ArrowUp)  // many-to-many: same action, multiple keys
    .insert(DungeonAction::TurnLeft, KeyCode::KeyQ)
    // ... build chain
    ;
```

**Key points:**

- **Multiple `.insert(action, key)` calls for the same action stack additively.** This is leafwing's many-to-many feature (research §Pros line 302). Pressing EITHER bound key triggers the action.
- **`KeyCode` is accepted directly** (no wrapper needed for keyboard inputs in 0.13+). Mouse and gamepad inputs may need `MouseButton` or `GamepadButton` directly, also unwrapped in modern versions.
- **Modifiers:** leafwing has a `ChordedInput` or `KeyboardInputChord` (exact name varies by version) that accepts `(modifier, key)` tuples. Example: `.insert(DungeonAction::Save, ChordedInput::modified(KeyCode::ControlLeft, KeyCode::KeyS))`. Druum's v1 keymap has no modifier chords — verify the exact API name on resolve if a future feature needs them.
- **`.build()` may or may not be needed** depending on the version. Some versions return `Self` from `.insert(...)` (chainable), others use a builder pattern requiring `.build()` at the end. Compile error will catch either way; not a blocking unknown.

### Resource vs Component shape

Per the Recommendation: **`Resource` shape** for v1.

```rust
// In ActionsPlugin::build:
app.insert_resource(default_dungeon_input_map());

// In a consuming system (Feature #7+):
fn handle_movement(actions: Res<ActionState<DungeonAction>>) { ... }
```

Per-entity `Component` shape (deferred):

```rust
// Spawn an entity that owns the map (NOT recommended for v1):
commands.spawn((
    InputMap::<DungeonAction>::default()
        .insert(DungeonAction::MoveForward, KeyCode::KeyW)
        .build(),
    ActionState::<DungeonAction>::default(),
));

// Consumer:
fn handle_movement(query: Query<&ActionState<DungeonAction>>) {
    for actions in &query { ... }
}
```

The Component shape adds a query layer for no current Druum benefit. Skip.

### Sources

- Training-data leafwing 0.13-0.17 README + examples. Verify on resolve.
- `research/20260326-01-bevy-first-person-dungeon-crawler-rpg.md:88` confirms many-to-many mapping is leafwing's selling point.

---

## RQ4 — Plugin wiring and state-scoped activation

### Confidence: HIGH for the Bevy-side claim (plugins are not run-conditioned); MEDIUM for the leafwing-side claim (its update system runs in PreUpdate)

### `InputManagerPlugin::<T>::default().run_if(in_state(...))` is wrong API shape

**Verified Bevy fact:** Bevy's `App::add_plugins(P)` takes `P: Plugin`, and the `Plugin` trait has no `run_if` method. `run_if` is on `IntoSystemConfigs` for systems. Plugins are always built unconditionally; their internal systems can be conditioned, but that's a per-system decision inside the plugin's `build` impl, not a per-plugin attribute.

This means the roadmap line 314's wording — `App::add_plugins(InputManagerPlugin::<DungeonAction>::default().run_if(in_state(GameState::Dungeon)))` — does not compile as written. (Source: read `bevy_app-0.18.1/src/plugin.rs` Plugin trait — has only `build`, `name`, `is_unique`, `ready`, `finish`, `cleanup`. No `run_if`.)

### Correct shape

```rust
// In ActionsPlugin::build (no run_if on the plugin):
app.add_plugins((
    InputManagerPlugin::<MenuAction>::default(),
    InputManagerPlugin::<DungeonAction>::default(),
    InputManagerPlugin::<CombatAction>::default(),
));

// In DungeonPlugin::build (Feature #7 — NOT Feature #5):
app.add_systems(
    Update,
    handle_dungeon_movement.run_if(in_state(GameState::Dungeon)),
);
```

The `ActionState<DungeonAction>` resource is always present (because the plugin is always registered), but only updated by leafwing's internal systems based on real input events. Consumers gate themselves with `.run_if(in_state(...))`. The state-scoping is a *consumer-side* concern, not a *registration-side* concern.

### What schedule does leafwing's update run in?

**MEDIUM confidence:** based on training data, leafwing registers its update systems in `PreUpdate`, after Bevy's `InputSystems` set (so `ButtonInput<KeyCode>` is freshly populated before leafwing reads it). This means `ActionState<T>` is up-to-date by the time `Update` systems read it.

**Conflict with Bevy's `keyboard_input_system`:** None. Bevy's system populates `ButtonInput<KeyCode>` from `KeyboardInput` messages; leafwing reads the populated `ButtonInput<KeyCode>`. They run in sequence (Bevy first, then leafwing), not in conflict.

**leafwing does NOT replace `InputPlugin`:** it layers on top. `bevy::DefaultPlugins` still installs `InputPlugin`; `ActionsPlugin` adds `InputManagerPlugin::<T>` for each enum. Both are needed.

### Implications for the F9 test setup

The F9 cycler does NOT go through leafwing (per §RQ7); its test setup (`init_resource::<ButtonInput<KeyCode>>` instead of `InputPlugin`) is unchanged.

For Feature #5 ActionState tests, the test app must use the FULL `InputPlugin` (not the bypass), so leafwing's update-in-PreUpdate has its `InputSystems` dependency satisfied. Detailed pattern in §RQ6.

### Sources

- `bevy_app-0.18.1/src/plugin.rs` — `Plugin` trait definition; no `run_if` method.
- `bevy_input-0.18.1/src/lib.rs:104-105, 116` — `InputSystems` SystemSet labels Bevy's input update; leafwing's plugin registers after it (training-data MEDIUM, verify on resolve).
- `bevy_input-0.18.1/src/keyboard.rs:163-172` — `keyboard_input_system` clears + repopulates `ButtonInput<KeyCode>` in PreUpdate.

---

## RQ5 — ActionState query path

### Confidence: HIGH for the API shape (matches Bevy's `ButtonInput` precedent); MEDIUM for the exact method names on the resolved version

### Consumer system pattern

```rust
fn handle_dungeon_movement(
    actions: Res<ActionState<DungeonAction>>,
    // ... other params
) {
    if actions.just_pressed(&DungeonAction::MoveForward) { ... }
    if actions.pressed(&DungeonAction::MoveForward)      { ... } // true while held
    if actions.just_released(&DungeonAction::MoveForward) { ... }
    let value: f32 = actions.value(&DungeonAction::MoveForward); // 1.0 if digital pressed
}
```

**Method names (training-data MEDIUM):**
- `just_pressed(&action) -> bool`
- `pressed(&action) -> bool`
- `just_released(&action) -> bool`
- `value(&action) -> f32` (axis-style; for digital, returns 1.0/0.0)
- `axis_pair(&action) -> DualAxisData` (for dual-axis like sticks; not used in Druum v1)

**Reference vs value:** Newer leafwing takes `&action`; older versions took `action` by value. Verify on resolve. Compile error catches either way.

### Resource vs per-entity

Per Recommendation: **`Res<ActionState<T>>`** (global).

```rust
// Read:
fn system(actions: Res<ActionState<DungeonAction>>) { ... }

// (Per-entity version, NOT recommended:)
fn system(query: Query<&ActionState<DungeonAction>>) {
    for actions in &query { ... }
}
```

### Boilerplate concern (the multi-plugin question)

The roadmap's concern: "is there a clean way to expose this to multiple plugins (e.g. dungeon plugin reads `DungeonAction`, combat plugin reads `CombatAction`) without boilerplate?"

**Answer:** Yes — each plugin imports the enum type from `crate::plugins::input`:

```rust
// In src/plugins/dungeon/movement.rs (Feature #7):
use crate::plugins::input::DungeonAction;
fn handle_movement(actions: Res<ActionState<DungeonAction>>) { ... }

// In src/plugins/combat/menu.rs (Feature #15):
use crate::plugins::input::CombatAction;
fn handle_combat_menu(actions: Res<ActionState<CombatAction>>) { ... }
```

The only "boilerplate" is the `use` statement — same as Druum's existing convention for `GameState` (no re-export from crate root, every consumer writes `use crate::plugins::state::GameState;`). Documented in `feedback`-style memory `project_druum_state_machine.md` as the import-clarity convention.

### Sources

- `bevy_input-0.18.1/src/button_input.rs:188, 207, 156-219` — Bevy's `ButtonInput` API surface (the model leafwing's API mirrors).
- `.claude/agent-memory/planner/project_druum_state_machine.md` — no-re-export convention from Feature #2.

---

## RQ6 — Test simulation pattern

### Confidence: HIGH for the underlying Bevy primitive (`ButtonInput::press`); MEDIUM for whether leafwing has a separate `MockInput` API

### The race trap (why the obvious pattern fails)

The obvious test pattern — `app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::KeyW); app.update();` — RACES with `keyboard_input_system` and FAILS. Per `bevy_input-0.18.1/src/keyboard.rs:170-172`, `keyboard_input_system` calls `keycode_input.bypass_change_detection().clear()` at the top of every PreUpdate. So:

- Frame N (after `app.update()` in setup): `just_pressed` is empty.
- Test code calls `press(KeyCode::KeyW)` directly on the resource. `just_pressed` now contains `W`.
- Test calls `app.update()`. PreUpdate runs FIRST: `keyboard_input_system` clears `just_pressed` (because it has not received a `KeyboardInput` message corresponding to our manual mutation — we mutated the wrong layer).
- leafwing's update runs in the same PreUpdate, sees empty `ButtonInput<KeyCode>`, leaves `ActionState<DungeonAction>::just_pressed(MoveForward)` as false.
- Test assertion fails.

This is the SAME class of bug that the F9 cycler avoids by NOT having `InputPlugin` registered (the bypass pattern in Feature #2). For Feature #5, we cannot bypass `InputPlugin` because leafwing depends on it. We must instead simulate at the LAYER ABOVE `keyboard_input_system` — by injecting `KeyboardInput` messages.

### The recommended pattern: inject `KeyboardInput` messages

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use bevy::input::keyboard::{Key, KeyboardInput};
    use bevy::input::{ButtonState, InputPlugin};
    use bevy::state::app::StatesPlugin;
    // smol_str::SmolStr is what bevy_input uses for logical_key.Character — verify
    // exact type on the resolved keyboard.rs version. (bevy_input-0.18.1/src/keyboard.rs)

    /// Pressing W triggers DungeonAction::MoveForward via leafwing's mapping.
    #[test]
    fn dungeon_w_maps_to_move_forward() {
        let mut app = App::new();
        // FULL InputPlugin REQUIRED — leafwing's update reads ButtonInput<KeyCode>
        // AFTER keyboard_input_system populates it from KeyboardInput messages.
        app.add_plugins((
            MinimalPlugins,
            StatesPlugin,
            InputPlugin,
            ActionsPlugin, // our new plugin under test
        ));
        app.update(); // initialise resources

        // Inject a real KeyboardInput message at the layer keyboard_input_system reads from.
        // KeyboardInput has 6 fields verified at bevy_input-0.18.1/src/keyboard.rs:109-139:
        //   key_code: KeyCode, logical_key: Key, state: ButtonState,
        //   text: Option<SmolStr>, repeat: bool, window: Entity.
        app.world_mut()
            .resource_mut::<bevy::ecs::message::Messages<KeyboardInput>>()
            .write(KeyboardInput {
                key_code: KeyCode::KeyW,
                logical_key: Key::Character("w".into()),
                state: ButtonState::Pressed,
                text: None,
                repeat: false,
                window: bevy::ecs::entity::Entity::PLACEHOLDER,
            });

        // One update: keyboard_input_system reads message → populates ButtonInput<KeyCode>
        // → leafwing's update reads → maps to DungeonAction::MoveForward → ActionState updates.
        app.update();

        let action_state = app.world().resource::<ActionState<DungeonAction>>();
        assert!(action_state.just_pressed(&DungeonAction::MoveForward));
    }
}
```

**Why this works:**

1. `InputPlugin` registers `keyboard_input_system` in `PreUpdate`. (Verified at `bevy_input-0.18.1/src/lib.rs:114-116`.)
2. `keyboard_input_system` clears `ButtonInput<KeyCode>` at frame start, then populates it from `KeyboardInput` *messages*. (Verified at `bevy_input-0.18.1/src/keyboard.rs:170-198`.)
3. By writing to `Messages<KeyboardInput>` (the message buffer), our injected press flows through the same code path a real OS press would. `keyboard_input_system` reads our message, populates `ButtonInput<KeyCode>`, leafwing's update consumes it, `ActionState<DungeonAction>::just_pressed(MoveForward)` is set to true.
4. The test assertion passes.

**`Messages<T>` type name verified:** in Bevy 0.18, the buffered-message resource is `Messages<T>` (not `Events<T>`), defined at `bevy_ecs-0.18.1/src/message/messages.rs:95`. Import path: `bevy::ecs::message::Messages`. This is the resource `keyboard_input_system` reads via `MessageReader<KeyboardInput>`.

**Alternative — use leafwing's `MockInput` trait if present:**

```rust
use leafwing_input_manager::input_mocking::MockInput;
app.send_input(KeyCode::KeyW);  // wraps the message-injection above
app.update();
```

Verification recipe:

```bash
# After Step A:
grep -r "trait MockInput" /Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/leafwing-input-manager-*/src/
```

**If `MockInput` exists:** use `app.send_input(KeyCode::KeyW)` + `app.update()` + assert.

**If `MockInput` does NOT exist:** use the message-injection pattern shown above (the inline test code body). It is the same primitive `MockInput` would wrap.

### Why not `init_resource::<ButtonInput<KeyCode>>` like Feature #2's F9 test?

The F9 test bypasses `InputPlugin` because the F9 cycler reads `ButtonInput<KeyCode>` DIRECTLY. The test sequence is: press `F9` → cycler runs in `Update` and reads `just_pressed(F9)` → it's true. No leafwing in the chain.

For Feature #5's ActionState tests, leafwing's update runs in `PreUpdate` and is the consumer of `ButtonInput<KeyCode>`. Without `InputPlugin`'s `keyboard_input_system`, leafwing has no fresh input data to consume. The bypass that makes F9 testing work breaks ActionState testing. **Different test pattern for different layer.**

### Sources

- `bevy_input-0.18.1/src/lib.rs:111-116` — `InputPlugin::build` registers `keyboard_input_system` in `PreUpdate`, sets up `ButtonInput<KeyCode>` resource and `KeyboardInput` message.
- `bevy_input-0.18.1/src/keyboard.rs:163-198` — `keyboard_input_system` body: clears `ButtonInput<KeyCode>` at frame start, then reads `MessageReader<KeyboardInput>` to populate.
- `.claude/agent-memory/implementer/feedback_bevy_test_input_setup.md` — F9 test bypass pattern (do NOT apply to Feature #5 ActionState tests).
- Training-data: leafwing's `MockInput` trait is referenced in 0.13+ examples. Verify exact name on Step A read.

---

## RQ7 — F9 cycler decision: keep direct vs refactor to leafwing

### Confidence: HIGH for the trade-off analysis (both options' costs are knowable from Druum's existing test infrastructure)

### Option A — Keep direct `Res<ButtonInput<KeyCode>>` (RECOMMENDED)

**What stays the same:** The F9 cycler in `src/plugins/state/mod.rs:71-89` is unchanged. Its test in lines 117-137 is unchanged. `init_resource::<ButtonInput<KeyCode>>()` test pattern stays.

**What's added (nothing):** No `DevAction` enum in `ActionsPlugin`. No `InputManagerPlugin::<DevAction>` registration. F9 stays as a dev-only direct-input hotkey.

**Pros:**
- Zero churn to Feature #2's working code.
- F9 is dev-only — there's no rebinding story to support, so leafwing's main value (rebinding via `InputMap`) is irrelevant.
- The F9 test setup doesn't need to change. The `init_resource::<ButtonInput<KeyCode>>` workaround for the `keyboard_input_system` clearing trap stays as documented.
- No `#[cfg(feature = "dev")]` symmetric-gating risk for an entire `DevAction` enum + `InputManagerPlugin::<DevAction>` registration + `InputMap<DevAction>` resource — all of which would need to be cfg-gated end to end.
- The ActionsPlugin can still have a `DevAction` enum reserved for future leafwing-routed dev hotkeys (e.g. a debug map overlay), without needing to migrate F9 today.

**Cons:**
- "Inconsistency": one cycler reads `ButtonInput<KeyCode>` directly, all gameplay reads `ActionState<T>`. Cosmetic concern; it's structurally different (dev vs gameplay), so the inconsistency is intentional and explainable.
- A future contributor might not understand the carve-out. **Mitigation:** document explicitly in the plan and in the F9 cycler's doc comment ("F9 is a dev hotkey and reads ButtonInput<KeyCode> directly because it predates leafwing and never needs rebinding").

### Option B — Refactor to `DevAction::CycleGameState`

**What changes:**
- Add `DevAction` variant `CycleGameState`.
- Bind `DevAction::CycleGameState` to `KeyCode::F9` in `default_dev_input_map()`.
- Rewrite F9 cycler system to take `Res<ActionState<DevAction>>` instead of `Res<ButtonInput<KeyCode>>`.
- Both `DevAction` enum AND `InputManagerPlugin::<DevAction>` registration AND `InputMap<DevAction>` resource MUST be `#[cfg(feature = "dev")]`-gated (cfg-asymmetry would fail clippy).
- The `f9_advances_game_state` test must be rewritten to use the full `InputPlugin + InputManagerPlugin::<DevAction>` setup, press via `ButtonInput::press` (or leafwing's `MockInput`), and call `app.update()` an extra time for leafwing's update to map the press through.
- The `gamestate_default_is_loading` test (which under `--features dev` needs `init_resource::<ButtonInput<KeyCode>>` because `cycle_game_state_on_f9` is registered) needs corresponding setup updates.

**Pros:**
- "Consistency": all input goes through leafwing, including dev hotkeys.
- If future dev hotkeys land (debug map overlay, party stat printer), they're already in the leafwing infrastructure rather than another direct-input system.

**Cons:**
- Test-setup churn: existing F9 test must be rewritten with `InputPlugin + InputManagerPlugin::<DevAction>`.
- `#[cfg(feature = "dev")]` symmetric-gating expands from "system def + add_systems" (current) to "enum def + InputMap default fn + InputManagerPlugin registration + InsertResource + system def + add_systems" — six gating points instead of two. Higher risk of asymmetry.
- The `init_resource::<ButtonInput<KeyCode>>()` workaround in `gamestate_default_is_loading` needs to become `init_resource::<ButtonInput<KeyCode>>() + init_resource::<ActionState<DevAction>>()` (or full plugin chain), depending on what leafwing requires for system-param validation.
- Adds new failure modes for marginal value — F9 is never user-rebindable, so leafwing's main feature is unused.

### Hidden complexity check (Option B only)

**Question:** Does `InputManagerPlugin::<DevAction>::default()` pull in resources that conflict with the no-`dev` build's existing test setup?

**Answer (training-data MEDIUM):** Yes. `InputManagerPlugin::<DevAction>` registers `ActionState<DevAction>` and (typically) reads `InputMap<DevAction>` at startup. Under `--features dev`, the test app needs both resources present; under no-dev, neither should exist. The cfg-gating must be perfect.

Additionally, `InputManagerPlugin::<T>` may itself bring in `bevy::reflect::TypeRegistration` requirements that need `bevy_reflect` in scope — which the project already has via `features = ["3d"]`, so OK.

### Recommendation

**Option A.** The only meaningful Option B benefit is "consistency", which is structurally false (F9 is a different concern from gameplay input). The Option B costs are real (test rewrite, expanded cfg-gating surface) and uncompensated. **Document the carve-out in the plan and in the F9 cycler's doc comment so future contributors don't try to "fix" it.**

If the user prefers Option B for aesthetic reasons, the plan should call out the test-rewrite cost explicitly so it's a conscious choice, not a surprise.

### Sources

- `src/plugins/state/mod.rs:71-89, 117-137` — current F9 cycler and test.
- `project/resources/20260501-102842-dev-feature-pattern.md` — symmetric-gating discipline; six gating points if Option B is chosen vs current two.
- `.claude/agent-memory/implementer/feedback_bevy_test_input_setup.md` — F9 test bypass pattern.

---

## RQ8 — Action enum granularity

### Confidence: HIGH for the trade-off (the analysis is on the type system, which is verifiable)

Already covered as "Architecture Option 2" above. Recommendation: **per-context enums** (`MenuAction`, `DungeonAction`, `CombatAction`, `DevAction`).

### Concrete enum-by-enum recommendation

| Enum | Recommend in Feature #5? | Rationale |
| ---- | ------------------------ | --------- |
| `MenuAction` | YES | Used by TitleScreen (Feature #25 future), Town menus (`TownLocation`), CombatPhase::EnemyTurn ("press any key to continue"-style), GameOver. Single shared enum across all menu contexts is fine because the action vocabulary is universal (Up/Down/Left/Right/Confirm/Cancel). |
| `DungeonAction` | YES | Used by `GameState::Dungeon + DungeonSubState::Exploring`. Movement + interactions specific to grid-walking. |
| `CombatAction` | YES | Used by `GameState::Combat + CombatPhase::PlayerInput`. Menu navigation in combat is similar to MenuAction but the trade-off favors a separate enum (different bound contexts in future, e.g. number-key skill slots in Feature #15). |
| `DevAction` | YES (skeleton with placeholder variant; gated `#[cfg(feature = "dev")]`) | Reserved for future dev-only hotkeys via leafwing. F9 stays direct-input per §RQ7. |
| `TownAction` | **NO** — defer | Town navigation is a menu-like select-a-location interface; reuse `MenuAction`. If a future feature adds movement within a town hub (e.g. walking around the square), introduce `TownAction` then. |

### Per-state mapping (which enum is "active" for which game state)

| GameState / SubState | Active action enum(s) |
| -------------------- | --------------------- |
| `GameState::Loading` | None — loading screen has no input. |
| `GameState::TitleScreen` | `MenuAction` |
| `GameState::Town` (any `TownLocation`) | `MenuAction` |
| `GameState::Dungeon + DungeonSubState::Exploring` | `DungeonAction` |
| `GameState::Dungeon + DungeonSubState::{Inventory,Map,Paused,EventDialog}` | `MenuAction` |
| `GameState::Combat + CombatPhase::PlayerInput` | `CombatAction` |
| `GameState::Combat + CombatPhase::{ExecuteActions,EnemyTurn,TurnResult}` | `MenuAction` (for "press any key to continue" prompts) |
| `GameState::GameOver` | `MenuAction` |
| Always (dev-only) | `DevAction` |

**Note:** Multiple action enums can be "active" simultaneously — leafwing's `ActionState<T>` resources are independent. The state-scoping happens in consuming systems via `.run_if(in_state(...))`. So even though `DungeonAction` and `MenuAction` could both update at the same time during `DungeonSubState::Inventory` (because `InputManagerPlugin::<T>` runs unconditionally), only the systems that read each enum react, and they're gated to the right state.

### Sources

- `src/plugins/state/mod.rs:6-47` — current state enum surface (used to map enums to states above).
- Roadmap §5 (lines 293-336) — original action enum sketches.
- Training-data leafwing convention for per-context enums.

---

## RQ9 — Default keybindings (Wizardry/Etrian conventions)

### Confidence: MEDIUM (genre conventions are well-established but vary by sub-genre and developer preference)

### `MenuAction` (used in TitleScreen, Town, GameOver, dungeon sub-state menus, combat between phases)

| Action | Primary key | Secondary key | Rationale |
| ------ | ----------- | ------------- | --------- |
| `Up` | `ArrowUp` | `KeyW` | Universal menu navigation. |
| `Down` | `ArrowDown` | `KeyS` | Universal. |
| `Left` | `ArrowLeft` | `KeyA` | Universal. |
| `Right` | `ArrowRight` | `KeyD` | Universal. |
| `Confirm` | `Enter` | `Space` | "Confirm selection" is universally Enter; Space is a common alternative. |
| `Cancel` | `Escape` | `Backspace` | "Cancel / back" is universally Escape. |

### `DungeonAction` (Wizardry/Etrian Odyssey/Grimrock-style)

| Action | Primary key | Secondary key | Rationale |
| ------ | ----------- | ------------- | --------- |
| `MoveForward` | `KeyW` | `ArrowUp` | Standard FPS-style WASD; arrows for keyboard-only / left-handed players. |
| `MoveBackward` | `KeyS` | `ArrowDown` | Standard. |
| `StrafeLeft` | `KeyA` | `ArrowLeft` | **Decision point.** Some classic DRPGs (Wizardry 1) bound arrows to TURN, not strafe — there's no strafing at all. Modern DRPGs (Etrian Odyssey, Grimrock) added strafing on A/D. The arrow-key alternate could be either strafe or turn. **Recommendation: arrows STRAFE** (matches WASD primary), with QE for turning. If user wants the classic "arrows turn" feel, document the alternative as a one-line code change. |
| `StrafeRight` | `KeyD` | `ArrowRight` | Same as StrafeLeft. |
| `TurnLeft` | `KeyQ` | (none) | Q/E for turning is genre convention since Grimrock 1. No arrow alternate avoids overloading arrows. |
| `TurnRight` | `KeyE` | (none) | Same. |
| `Interact` | `Space` | `KeyF` | Space is universal "do the thing"; F is a common alternative for "use" in modern dungeon crawlers. |
| `OpenInventory` | `Tab` | `KeyI` | Tab is the FPS-style "menu key"; I is the RPG-style "inventory key" (Skyrim, Diablo). |
| `OpenMap` | `KeyM` | (none) | Universal map key. |
| `Pause` | `Escape` | (none) | Universal pause. Note: Escape also means Cancel in MenuAction — both can co-exist because the active enum is state-scoped. |

### `CombatAction` (turn-based menu navigation)

| Action | Primary key | Secondary key | Rationale |
| ------ | ----------- | ------------- | --------- |
| `Up` | `ArrowUp` | `KeyW` | Same as MenuAction. |
| `Down` | `ArrowDown` | `KeyS` | Same. |
| `Left` | `ArrowLeft` | `KeyA` | Same. |
| `Right` | `ArrowRight` | `KeyD` | Same. |
| `Confirm` | `Enter` | `Space` | Universal. |
| `Cancel` | `Escape` | `Backspace` | Universal. |

(Combat skill-slot number keys `1-9` deferred to Feature #15 along with the skill system itself.)

### `DevAction` (placeholder for v1)

```rust
#[derive(Actionlike, ...)]
pub enum DevAction {
    DebugDummy, // Placeholder — first real DevAction variant in a future feature replaces this.
}
```

No bindings in v1. The enum exists so `InputManagerPlugin::<DevAction>` is registered (under `#[cfg(feature = "dev")]`) without a "no variants" compile error. When a real dev hotkey lands (e.g. debug map overlay), the placeholder is removed and replaced.

### Reserved keys (DO NOT bind for gameplay)

- `F9` — F9 cycler (per §RQ7).
- Future `F1-F12` likely for dev hotkeys; keep them out of gameplay binding tables.
- `Backquote` (\`) — common debug-console key in Bevy ecosystems; reserve for future `bevy-inspector-egui` if/when it lands.
- `PrintScreen` — OS-level screenshot hotkey; never bind.

### Sources

- Wizardry / Etrian Odyssey / Grimrock genre conventions (training-data MEDIUM — specific games' default keymaps).
- Original research §Pattern 4 (lines 685-740) shows current movement-input prototype with WASD + QE turn — same convention.
- Roadmap §5 line 329 — "WASD + QE for turning, M for map, Tab for inventory, Esc for pause."

---

## RQ10 — Cargo.toml and Rust 2024 edition transitive-dep risk

### Confidence: MEDIUM (transitive dep list is leafwing-version-specific; HIGH after Step A)

### Direct deps Druum needs to declare

After adding `leafwing-input-manager`, the following crates MAY need explicit declaration in Druum's `Cargo.toml` if Druum's source code references them directly:

| Crate | Transitively present? | Need to declare? | Reason |
| ----- | --------------------- | ---------------- | ------ |
| `leafwing-input-manager` | NO (new) | YES | Direct add. |
| `serde` | YES (already declared at version `1`) | Already done | Druum's `src/data/*.rs` uses serde. Existing dep. |
| `bevy_reflect` | YES (transitively via Bevy) | NO | Druum already accesses it via `bevy::prelude::Reflect`. Existing pattern. |
| `bevy_egui` | If leafwing's `egui` feature is on | NO (we won't enable that feature) | Skip. |
| `bevy_asset` | Always (via Bevy) | NO | Already accessed via bevy prelude. |

**Rust 2024 edition rule** (per `feedback_rust_2024_transitive_deps.md`): if Druum's source code writes `use leafwing_input_manager::...`, the crate MUST be in `[dependencies]` even though it could be argued to be "transitive" via something. Adding via `cargo add` ensures this. No surprises here.

### Feature flags to enable / disable

Training-data MEDIUM list of leafwing's typical features:

| Feature | Default? | Enable in Druum? | Reason |
| ------- | -------- | ---------------- | ------ |
| `egui` (binding-display widgets via bevy_egui) | Maybe | NO | Druum doesn't have bevy_egui yet; that's Feature #25's call. Disable to avoid pulling in bevy_egui transitively. |
| `asset` (RON-loadable InputMap) | Maybe | NO for v1; YES at Feature #25 | Defer until settings UI lands. |
| `serde` (Serialize/Deserialize for InputMap) | Maybe | NO for v1; YES at Feature #25 | Defer. |
| `mouse` | YES | YES (default) | Druum doesn't need mouse for v1 but the feature is small and no harm. |
| `gamepad` | YES | NO for v1 | Druum is keyboard-only v1. Disabling shrinks compile time slightly. **Caveat:** leafwing may not allow disabling gamepad if it's a default feature without breaking other defaults. Verify on Step A. |
| `keyboard` | YES | YES (required) | Obvious. |

**Recommended Cargo.toml shape (verify after Step A):**

```toml
# Variant 1 (minimal — accept defaults except egui):
leafwing-input-manager = "=<RESOLVED-VERSION>"

# Variant 2 (explicitly minimal):
leafwing-input-manager = { version = "=<RESOLVED-VERSION>", default-features = false, features = ["keyboard", "mouse"] }
```

Pick Variant 2 only if Variant 1's defaults pull in bevy_egui or other unwanted deps. Verify by inspecting `cargo tree` after first `cargo check`.

### Conflict with existing serde / ron deps

- `serde = "1"` already in Druum. leafwing's `serde` feature, if enabled, would reuse the same major. No conflict.
- `ron = "0.12"` in Druum. leafwing's binding format (if/when serde feature enabled in Feature #25) may use ron — verify version then. No relevance to Feature #5.

### Sources

- `Cargo.toml:22-27` — Druum's existing `[dependencies]` block.
- `.claude/agent-memory/implementer/feedback_rust_2024_transitive_deps.md` — Rust 2024 edition transitive-dep restriction.
- `.claude/agent-memory/researcher/reference_ron_format_compat.md` — pattern for handling multi-major-version deps.

---

## RQ11 — Bevy 0.18 Event/Message rename

### Confidence: HIGH for the Bevy fact (verified at multiple sites in Feature #2 and #3 research); MEDIUM for whether leafwing exposes any reactive type that's affected

### What we know about the rename

In Bevy 0.18, `Event` → `Message` for buffered (polling) types. Affected types verified in Druum's prior research:

- `StateTransitionEvent<S>` — `Message` (Feature #2)
- `AssetEvent<T>` — `Message` (Feature #3)
- `KeyboardInput` — `Message` (verified at `bevy_input-0.18.1/src/keyboard.rs:97-100`)

Reading these requires `MessageReader<T>`, NOT `EventReader<T>`. The latter does not compile.

### What this means for leafwing

leafwing was originally written for Bevy ≤0.17 era (training-data MEDIUM). The 0.18-compat release MUST have updated all internal `EventReader`/`EventWriter` calls to `MessageReader`/`MessageWriter` for the affected Bevy types. If leafwing also exposes reactive types of its own (e.g. `ActionDiff<T>` for input-replay or networking), those types are leafwing-defined and follow leafwing's own derive choices — they could be `Event` or `Message` independently.

### Mitigation for Druum

Druum v1's input consumption is poll-based: `Res<ActionState<T>>::just_pressed(...)`. This sidesteps any reactive-type concerns entirely.

If a future feature wants to read leafwing reactive types (e.g. for save-state checksums on every action, or for a replay system), it MUST verify the type kind on the resolved leafwing version:

```bash
# Verification recipe (run in implementer's Bash, not researcher's):
grep -rn "derive(Message\|derive(Event" /Users/nousunio/.cargo/registry/src/index.crates.io-*/leafwing-input-manager-*/src/
```

Then use the appropriate reader.

### Sources

- `.claude/agent-memory/researcher/feedback_bevy_0_18_event_message_split.md` — the split was verified for `StateTransitionEvent` and `AssetEvent`.
- `bevy_input-0.18.1/src/keyboard.rs:97-100` — `KeyboardInput` is `derive(Message, ...)`, confirming Bevy's input messages are also `Message`-flavored.
- `.claude/agent-memory/planner/project_druum_state_machine.md` — `MessageReader<StateTransitionEvent<S>>` not `EventReader<StateTransitionEvent<S>>`.
- `.claude/agent-memory/planner/project_druum_asset_pipeline.md` — same trap, `MessageReader<AssetEvent<T>>` for asset events.

---

## RQ12 — Other 0.18-specific traps

### Confidence: MEDIUM (training data + on-disk Bevy source verification)

### Trap 1: `InputSystems` SystemSet ordering

**What goes wrong:** leafwing's update system runs before Bevy's `keyboard_input_system` populates `ButtonInput<KeyCode>`, missing the press by one frame.

**Bevy 0.18 fact:** `InputSystems` is a `SystemSet` defined at `bevy_input-0.18.1/src/lib.rs:104-105`. Bevy's `keyboard_input_system` is `.in_set(InputSystems)` at line 116. leafwing should register its own update with `.after(InputSystems)` to guarantee ordering.

**Mitigation:** Most likely already correct in the resolved leafwing version (this is a well-known concern). Verify on Step A by reading leafwing's `InputManagerPlugin::build` impl. If wrong, file an issue upstream and apply a workaround:

```rust
// In ActionsPlugin::build, after add_plugins(InputManagerPlugin::<T>::default()):
app.configure_sets(PreUpdate, /* leafwing's set name */.after(InputSystems));
```

### Trap 2: `Reflect` derive on action enums

**What goes wrong:** leafwing's `Actionlike` macro requires `Reflect` (or doesn't), and the docs lag the implementation.

**Mitigation:** Always derive `Reflect` (per RQ2 recommendation). It's free in Bevy 0.18 — verified for unit enums in `reference_bevy_reflect_018_derive.md`. If it turns out to be unused by leafwing, no harm done.

### Trap 3: `ActionState<T>` exclusive world access

**What goes wrong:** Some Bevy resources require exclusive `&mut World` access (e.g. for hot-reload coordination). If `ActionState<T>` is one of these, leafwing's update system would serialize against any system reading `Res<ActionState<T>>`.

**Bevy 0.18 fact:** Resources accessed via `Res<T>` and `ResMut<T>` use Bevy's standard parallel scheduler — only conflict on the same resource. leafwing's `ActionState<T>` is most likely a plain `Resource`, not exclusive. (Training-data HIGH-confidence.)

**Mitigation:** None needed unless profiling shows contention. Druum's input volume is low.

### Trap 4: `InputMap<T>` as Component requires entity spawning

(Not relevant if recommendation #3 is taken — `Resource` shape avoids this.)

If per-entity `InputMap<T>` is chosen, the spawning question is: who spawns the entity? Options:
- `ActionsPlugin::build` adds a `Startup` system that spawns one entity with all four `InputMap<T>` components and `ActionState<T>` components.
- A consuming plugin spawns its own entity (couples input ownership to gameplay plugin).
- Spawn at `OnEnter(GameState::Title)` so player setup is part of the lifecycle.

**Mitigation:** Skip the question by using `Resource` shape.

### Trap 5: leafwing's deps may pull in a different Bevy minor

**What goes wrong:** leafwing's `Cargo.toml` says `bevy = "^0.18"` but its lockfile resolves to `0.18.0`, conflicting with Druum's `=0.18.1` pin.

**Bevy 0.18 fact:** Cargo's resolver should unify on `0.18.1` (the higher patch satisfying the caret), not duplicate. But if leafwing pins exactly (`= "0.18.0"`), Cargo will refuse the build.

**Mitigation:** Verify after `cargo add`. If conflict, file an upstream issue requesting flexible bevy spec, and apply the `moonshine-save` playbook (escalate to user).

### Trap 6: `bevy_input` features mismatch

**What goes wrong:** Druum's `bevy = { features = ["3d", ...] }` line includes `"3d"` which transitively enables `bevy_input` defaults (`keyboard`, `mouse`). leafwing might require additional `bevy_input` features (e.g. `gamepad`) that are not in Druum's enabled set. Compile errors at leafwing's plugin registration.

**Mitigation:** If first compile errors with "missing feature", add to Druum's `Cargo.toml` `bevy.features` list:

```toml
bevy = { version = "=0.18.1", default-features = false, features = [
    "3d",
    "png", "ktx2", "zstd_rust", "bevy_text",
    # If leafwing needs gamepad even though we don't:
    # "bevy_gilrs",  # Bevy's gamepad feature wrapper
] }
```

Likely not needed for keyboard-only v1.

### Sources

- `bevy_input-0.18.1/src/lib.rs:104-105, 110-117` — InputSystems set + plugin registrations.
- `bevy_input-0.18.1/src/keyboard.rs:97-100, 163-198` — KeyboardInput message + system.
- Cargo manifest at `Cargo.toml:9-21` — current Bevy feature set.

---

## Open Questions (categorized for the planner)

### Category A — Factual gaps (need verification before plan locks)

1. **Exact published version of `leafwing-input-manager` supporting Bevy 0.18.1.**
   - What we know: 2026-03-26 prior research cites `0.18.0`. Today is 2026-05-01. There may be patch releases.
   - What's unclear: exact resolved version + exact `bevy = "..."` requirement.
   - Recommendation: run §RQ1 Step A. Pin with `=`. Halt if no compatible version.

2. **leafwing's `MockInput` trait existence and exact API.**
   - What we know: training-data references suggest a `MockInput` trait exists with `app.send_input(KeyCode)` style.
   - What's unclear: whether the resolved version has it, and the exact method name.
   - Recommendation: After Step A, `grep "MockInput" leafwing/src/`. If present, use it. If not, fall back to `KeyboardInput` message injection (verified Bevy primitive).

3. **leafwing's exact feature flag list and defaults.**
   - What we know: training data suggests `egui`, `asset`, `serde`, `keyboard`, `mouse`, `gamepad`.
   - What's unclear: exact names + defaults on the resolved version.
   - Recommendation: After Step A, read the resolved Cargo.toml `[features]` block. Lock the dep with whatever minimal set Druum needs.

4. **Whether `ActionState<T>::just_pressed` takes `T` by value or `&T` by reference.**
   - What we know: newer leafwing versions take `&T`.
   - What's unclear: exact signature on resolved version.
   - Recommendation: Compile error catches it at first use. Plan should write Pattern 2 example with `&` defensively; Feature #7+ implementer adjusts.

### Category B — Genuine technical forks (planner picks one)

5. **`InputMap<T>` shape: `Resource` (recommended) vs `Component` (per-entity).**
   - Recommendation: Resource for v1. Reassess at Feature #25 if multi-player ever scoped.
   - Implementer cost: minor — both shapes are documented.

6. **`InputManagerPlugin::<T>` registrations: centralized in `ActionsPlugin` (recommended) vs per-consumer plugin.**
   - Recommendation: centralize. Greppability + single registration site.

7. **One mega-Action enum vs per-context enums (recommended).**
   - Recommendation: per-context. Type-scoping wins.

### Category C — Preference / trade-off requiring user input

8. **F9 cycler: keep direct `Res<ButtonInput<KeyCode>>` (recommended Option A) vs refactor to `DevAction::CycleGameState` (Option B).**
   - Trade-offs in §RQ7 above. Option A has no churn; Option B has cosmetic consistency + test rewrite cost.
   - **User decision needed if planner wants to override.**

9. **Should arrow keys in dungeon STRAFE (recommended; matches WASD primary) or TURN (matches classic Wizardry 1)?**
   - This is a feel-of-the-game decision the genre is split on.
   - **Recommendation: arrows strafe (modern convention). User can override.**

10. **`TownAction` separate enum or fold into `MenuAction` (recommended for v1)?**
    - For v1, Town navigation is menu-style — `MenuAction` suffices.
    - **User decision if/when Town gets distinct movement (Feature #19+).**

11. **Deferred features:** Settings UI rebinding (Feature #25), gamepad bindings (post-v1), in-game keymap-display HUD (later UI feature). All out of scope for Feature #5; surfaced as forward-compatibility nudges in the plan.

---

## Sources

### Primary (HIGH confidence — verified against on-disk source)

- [`bevy_input-0.18.1/src/lib.rs`](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_input-0.18.1/src/lib.rs) — `InputPlugin`, `InputSystems` SystemSet, schedule placement (`PreUpdate`), keyboard/mouse/gamepad/touch feature gating
- [`bevy_input-0.18.1/src/keyboard.rs`](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_input-0.18.1/src/keyboard.rs) — `KeyboardInput` (derived `Message`), `keyboard_input_system` body (clears + repopulates), KeyCode enum
- [`bevy_input-0.18.1/src/button_input.rs`](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_input-0.18.1/src/button_input.rs) — `ButtonInput<T>` API surface (the model leafwing's API mirrors)
- [`bevy_input-0.18.1/Cargo.toml`](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_input-0.18.1/Cargo.toml) — feature flags, bevy version pinning convention
- [`Cargo.toml`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/Cargo.toml) — Druum's existing dep declarations and `dev` feature
- [`Cargo.lock`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/Cargo.lock) — lock content (no leafwing entry confirms not yet added)
- [`src/plugins/state/mod.rs`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/state/mod.rs) — current F9 cycler + test
- [`src/main.rs`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/main.rs) — current plugin registration tuple

### Primary (HIGH confidence — verified prior research)

- [`research/20260326-01-bevy-first-person-dungeon-crawler-rpg.md`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/research/20260326-01-bevy-first-person-dungeon-crawler-rpg.md) §Standard Stack (line 88), §Don't Hand-Roll (line 989), §Sources (line 1318)
- [`project/research/20260429-030000-bevy-0-18-1-state-machine-feature-2.md`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/project/research/20260429-030000-bevy-0-18-1-state-machine-feature-2.md) — F9 cycler rationale, `keyboard_input_system` clearing trap discovery
- [`project/research/20260501-160000-bevy-0-18-1-asset-pipeline-feature-3.md`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/project/research/20260501-160000-bevy-0-18-1-asset-pipeline-feature-3.md) — verification-recipe + escalation-on-incompatibility precedent
- [`project/research/20260501-220000-feature-4-dungeon-grid-data-model.md`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/project/research/20260501-220000-feature-4-dungeon-grid-data-model.md) — research format + depth precedent
- [`project/resources/20260501-102842-dev-feature-pattern.md`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/project/resources/20260501-102842-dev-feature-pattern.md) — `#[cfg(feature = "dev")]` symmetric gating discipline
- [`project/resources/20260501-104450-bevy-state-machine-anatomy.md`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/project/resources/20260501-104450-bevy-state-machine-anatomy.md) — Plugin trait shape, schedule ordering, run conditions
- [`.claude/agent-memory/implementer/feedback_bevy_test_input_setup.md`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/.claude/agent-memory/implementer/feedback_bevy_test_input_setup.md) — F9 test bypass pattern
- [`.claude/agent-memory/code-reviewer/feedback_bevy_test_just_pressed_persistence.md`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/.claude/agent-memory/code-reviewer/feedback_bevy_test_just_pressed_persistence.md) — `just_pressed` persistence under bypass
- [`.claude/agent-memory/researcher/feedback_bevy_0_18_event_message_split.md`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/.claude/agent-memory/researcher/feedback_bevy_0_18_event_message_split.md) — Event/Message rename trap
- [`.claude/agent-memory/researcher/reference_bevy_reflect_018_derive.md`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/.claude/agent-memory/researcher/reference_bevy_reflect_018_derive.md) — Reflect derive auto-supports unit-variant enums
- [`.claude/agent-memory/implementer/feedback_rust_2024_transitive_deps.md`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/.claude/agent-memory/implementer/feedback_rust_2024_transitive_deps.md) — Rust 2024 transitive-dep restriction
- [`.claude/agent-memory/planner/project_druum_state_machine.md`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/.claude/agent-memory/planner/project_druum_state_machine.md) — Feature #5 hand-off explicitly noted: "Feature #5 (input system with `leafwing-input-manager`) is the canonical input owner. F9 is a temporary debug affordance"

### Secondary (MEDIUM confidence — training data, no live verification)

- `leafwing-input-manager` API surface (`Actionlike` derive, `InputMap` constructor, `InputManagerPlugin`, `ActionState`, `MockInput`) — training-data recall of versions 0.13–0.17. Resolved version 0.18.x will need verification on Step A.
- leafwing-Studios maintainer activity / release cadence — training-data MEDIUM. Cross-checked via the prior research §Standard Stack which marked leafwing as "Active" as of 2026-03-26.
- Genre keymap conventions (Wizardry/Etrian Odyssey/Grimrock) — training-data + general gaming knowledge; verified against the 2026-03-26 research §Pattern 4 movement-input prototype which already uses W/S/A/D + Q/E.

### Tertiary (LOW confidence — flagged for verification)

- Exact `MockInput` trait API shape — flag for Step A verification.
- Exact `InputMap` builder shape (`.insert` vs `.add`, presence of `.build()`) — flag for Step A verification.
- Whether `ActionState::just_pressed` takes `&T` or `T` — flag, compile error will catch.

### Live-verification recipes (collected for the implementer)

```bash
# Step A — verify leafwing version + bevy compat (RQ1):
cd /Users/nousunio/Repos/Learnings/claude-code/druum
cargo add leafwing-input-manager --dry-run 2>&1 | tee /tmp/leafwing-resolve.txt

# Step B — confirm features list:
cargo info leafwing-input-manager --version <RESOLVED-VERSION>

# Step C — inspect resolved API surface (after `cargo add` actually runs):
ls /Users/nousunio/.cargo/registry/src/index.crates.io-*/leafwing-input-manager-*/src/
grep -rn "trait MockInput\|fn just_pressed\|fn insert\|fn build" /Users/nousunio/.cargo/registry/src/index.crates.io-*/leafwing-input-manager-*/src/

# Step D — verify Event/Message trap (RQ11):
grep -rn "derive(Message\|derive(Event" /Users/nousunio/.cargo/registry/src/index.crates.io-*/leafwing-input-manager-*/src/

# Step E — re-run all 6 verification commands after Cargo.toml change:
cargo check
cargo check --features dev
cargo clippy --all-targets -- -D warnings
cargo clippy --all-targets --features dev -- -D warnings
cargo test
cargo test --features dev
```

---

## Metadata

**Confidence breakdown:**

- RQ1 (Bevy 0.18 compat): MEDIUM — verifiable to HIGH via Step A (single `cargo add --dry-run`)
- RQ2 (Action trait derives): MEDIUM — verifiable to HIGH after Step A reads the crate's docs
- RQ3 (InputMap API): MEDIUM — verifiable to HIGH after Step A
- RQ4 (Plugin wiring): HIGH for Bevy-side (plugins not run-conditioned); MEDIUM for leafwing schedule placement
- RQ5 (ActionState query): HIGH for shape (mirrors ButtonInput); MEDIUM for exact method names
- RQ6 (Test simulation): HIGH for the bevy_input layer (verified on disk); MEDIUM for `MockInput` API
- RQ7 (F9 cycler decision): HIGH — trade-offs are knowable from existing test infrastructure
- RQ8 (Enum granularity): HIGH for the trade-off (type-system property)
- RQ9 (Default keymap): MEDIUM — genre conventions vary
- RQ10 (Cargo.toml + features): MEDIUM — verifiable to HIGH after Step A
- RQ11 (Event/Message rename): HIGH for the Bevy fact; MEDIUM for whether leafwing exposes affected types
- RQ12 (Other 0.18 traps): MEDIUM — speculative coverage

**Verification status:** All Bevy 0.18.1 first-party API claims verified against on-disk source. All `leafwing-input-manager`-specific claims are training-data MEDIUM, gated behind the §RQ1 Step A verification recipe (single `cargo add --dry-run` invocation; expected resolution time: <30 seconds).

**Critical-path action for the planner:** Run §RQ1 Step A FIRST. If it succeeds, the plan can proceed with everything in this document. If it fails (no Bevy-0.18-compatible leafwing release), HALT and escalate to user with the moonshine-save playbook from Feature #3.

**Research date:** 2026-05-01
