# The `dev` Feature Flag Pattern

The Druum project uses a single Cargo feature called `dev` for two purposes: enabling Bevy's dynamic linking for fast iteration, and conditionally compiling developer-only debug hooks. This document explains how the pattern works, how to use it, and how to extend it as new debug hooks land.

## What it is

In `Cargo.toml`:

```toml
[features]
default = []
# Enable fast incremental rebuilds via Bevy dylib. NEVER include in release builds.
# Usage: `cargo run --features dev`
dev = ["bevy/dynamic_linking"]
```

The `dev` feature is **off by default**. It gets activated only when explicitly requested via `--features dev` on a Cargo command. Activating it has two compounding effects.

## Effect 1 — Bevy dynamic linking

Without `--features dev`, Bevy compiles as a static `rlib` and links into the final binary. Any change anywhere in the Bevy dependency graph forces a re-link of the whole executable, which is slow (~30s incremental).

With `--features dev`, the `bevy/dynamic_linking` feature flag turns Bevy into a dynamic library (`.dylib` on macOS, `.so` on Linux, `.dll` on Windows). The Bevy dylib is built once; rebuilds of just our crate skip re-linking Bevy entirely. Incremental rebuild drops to ~5–8s.

The trade-off: a `--features dev` binary depends on the Bevy `.dylib` being present at runtime. **You cannot ship a `--features dev` build to users** — it will fail to start without the dynamic library next to it. Release artifacts must always be built without this feature.

## Effect 2 — Conditional compilation of debug hooks

Code annotated with `#[cfg(feature = "dev")]` is compiled in only when `--features dev` is active. Anything else is removed entirely by the compiler — not just hidden, but absent from the binary.

This is used for developer-only conveniences that should never reach a player. The first example, in `src/plugins/state/mod.rs`, is the F9 game-state cycler:

```rust
#[cfg(feature = "dev")]
fn cycle_game_state_on_f9(
    keys: Res<ButtonInput<KeyCode>>,
    current: Res<State<GameState>>,
    mut next: ResMut<NextState<GameState>>,
) {
    if !keys.just_pressed(KeyCode::F9) {
        return;
    }
    let upcoming = match current.get() {
        GameState::Loading     => GameState::TitleScreen,
        GameState::TitleScreen => GameState::Town,
        GameState::Town        => GameState::Dungeon,
        GameState::Dungeon     => GameState::Combat,
        GameState::Combat      => GameState::GameOver,
        GameState::GameOver    => GameState::Loading,
    };
    next.set(upcoming);
}
```

And inside the `StatePlugin::build` impl, the registration of that system is *also* gated:

```rust
#[cfg(feature = "dev")]
app.add_systems(Update, cycle_game_state_on_f9);
```

**Both** the function definition and the `add_systems` call must be cfg-gated. Gating only one of them produces an `unused function` clippy warning in release builds, which fails CI under `clippy --all-targets -- -D warnings`.

## What the F9 cycler actually does

Pressing F9 in a dev build advances the top-level `GameState` by one position in this fixed order:

```
Loading → TitleScreen → Town → Dungeon → Combat → GameOver → Loading → ...
```

It is **not** a game restart. It only changes the `GameState` enum value. Entities, resources, save data, party state, RNG seeds — none of those are touched. If you press F9 from `Combat` with a wounded party, you land in `GameOver` with the same wounded party.

The cycler is a developer convenience for jumping between game phases without going through real gameplay. As features land (loading screen, title, town hub, dungeon renderer, combat UI), F9 will start landing you in visible scenes instead of the current black-window-with-log-lines.

Sub-states (`DungeonSubState`, `CombatPhase`, `TownLocation`) are **not** cycled by F9. They reset to their default each time their parent state is re-entered. If a future feature needs sub-state cycling for testing, it should add its own dev-gated hotkey rather than overload F9.

## Commands cheat sheet

| Command | Bevy linking | Dev hooks compiled | Tests run |
|---|---|---|---|
| `cargo check` | static (slow incrementals) | excluded | 1 |
| `cargo check --features dev` | dynamic (fast incrementals) | included | 2 |
| `cargo clippy --all-targets -- -D warnings` | static | excluded | n/a |
| `cargo clippy --all-targets --features dev -- -D warnings` | dynamic | included | n/a |
| `cargo test` | static | excluded | non-dev only |
| `cargo test --features dev` | dynamic | included | all |
| `cargo run --features dev` | dynamic | included (F9 works) | n/a |
| `cargo build --release` | static, fully optimized | excluded | n/a |

**Day-to-day development:** `cargo run --features dev`.

**Before committing:** run both `cargo check` and `cargo check --features dev` to confirm both code paths compile cleanly.

## Why CI runs both feature sets

Skipping the no-`dev` build hides two classes of bug:

1. **Asymmetric cfg gating.** If you forget to gate the `add_systems` registration but remember to gate the function (or vice versa), the no-dev build will fail with either a missing-symbol error or an unused-function warning. The dev build won't catch it.
2. **Accidental dev-feature dependencies.** If a release-path module accidentally calls into something only defined under `#[cfg(feature = "dev")]`, the no-dev build is the only way to surface it.

Therefore every verification run does both:
- `cargo clippy --all-targets -- -D warnings`
- `cargo clippy --all-targets --features dev -- -D warnings`

Each clippy invocation rebuilds the project under that feature set; expect ~30–60s extra wall-clock time per feature combination on a clean cache.

## Tests under `--features dev`

When the `dev` feature is on, the F9 cycler system is registered with the app — and Bevy 0.18 validates every registered system's parameters at every `app.update()` call. That means even unit tests that don't exercise F9 still need `ButtonInput<KeyCode>` to exist as a resource if the cycler is registered.

The pattern in `src/plugins/state/mod.rs` tests:

```rust
#[cfg(feature = "dev")]
app.init_resource::<ButtonInput<KeyCode>>();
```

This inserts the resource without the rest of `InputPlugin`'s machinery (specifically `keyboard_input_system`, which clears `just_pressed` in `PreUpdate` and would defeat F9 testing). See `.claude/agent-memory/implementer/feedback_bevy_test_input_setup.md` for the original discovery.

## Adding new dev-only hooks

When future features add their own debug hotkeys (debug map overlay, party stat printer, encounter forcer, godmode toggle, etc.), follow the same template:

1. Cfg-gate the function definition: `#[cfg(feature = "dev")]`
2. Cfg-gate the `add_systems` call inside the plugin's `build` impl
3. Pick a function key that isn't already in use (F9 is taken). Avoid letter keys — they conflict with future gameplay input.
4. Document the hotkey in this file, the plugin's module-level doc, and (when one exists) a developer-facing controls cheat sheet.

If a hook needs additional resources to function, gate the resource registration too — but consider whether the resource is dev-only or genuinely useful in release. A debug-stats resource is dev-only; a `RngSeed` resource (for permadeath) is release-required.

## References

- `Cargo.toml` — `[features]` block
- `src/plugins/state/mod.rs` — first concrete example (F9 cycler)
- `project/plans/20260429-031500-bevy-0-18-1-state-machine-feature-2.md` §Critical — gating discipline
- `project/implemented/20260429-173500-bevy-0-18-1-state-machine-feature-2.md` §Deviations — input test setup discovery
- `project/roadmaps/20260429-01-bevy-dungeon-crawler-roadmap.md` §Feature 1 — original decision to never include `dynamic_linking` in `default`
- Bevy book on fast compiles: https://bevyengine.org/learn/quick-start/getting-started/setup/#enable-fast-compiles-optional
