---
name: Bevy 0.18 deterministic time advance for tests
description: TimeUpdateStrategy::ManualDuration lets tests advance Time::delta_secs by a known amount per app.update() — the only reliable way to test animation systems
type: reference
---

When writing tests for systems that read `Time::delta_secs()` (animation, fade, tween, cooldown), wall-clock time is non-deterministic across machines and CI. The fix is `TimeUpdateStrategy::ManualDuration`.

**Verified at `bevy_time-0.18.1/src/lib.rs:99-119`:**

```rust
#[derive(Resource, Default)]
pub enum TimeUpdateStrategy {
    #[default] Automatic,            // wall clock — flaky in tests
    ManualInstant(Instant),
    ManualDuration(Duration),         // ← the test-friendly mode
    FixedTimesteps(u32),
}
```

When `ManualDuration(d)` is set, every call to `app.update()` advances `Time::delta()` by exactly `d`. So:

```rust
let mut app = App::new();
app.add_plugins(MinimalPlugins); // includes TimePlugin (verified at bevy_internal-0.18.1/src/default_plugins.rs:139-146)
app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(50)));
// ... spawn entities, set up state ...
for _ in 0..10 {
    app.update();
}
// Time has advanced exactly 500ms total — assert animation finished, etc.
```

**MinimalPlugins includes TimePlugin** — no extra plugin needed.

**How to apply:** Whenever a Druum test needs to verify a tweened/timed effect (movement animation finishing, FadeIn ramp completing, cooldowns expiring), use `TimeUpdateStrategy::ManualDuration` instead of looping `app.update()` and hoping the wall clock advances. The audio test `fade_in_component_lifecycle` in `src/plugins/audio/mod.rs:262-296` does NOT use this and admits in its comments that it can't reliably verify the duration-based path on headless CI — that's the failure mode this resource fixes.
