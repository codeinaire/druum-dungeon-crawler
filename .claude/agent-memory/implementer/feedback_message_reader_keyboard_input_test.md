---
name: feedback_message_reader_keyboard_input_test
description: MessageReader<KeyboardInput> in MinimalPlugins test app requires app.add_message::<KeyboardInput>() — InputPlugin is normally the registrar
metadata:
  type: feedback
---

When a system uses `MessageReader<KeyboardInput>` and is registered in a test app built with `MinimalPlugins` (which omits `InputPlugin`), the system's param validation panics because `Messages<KeyboardInput>` is not registered.

**Why:** `InputPlugin` normally registers `KeyboardInput` as a message/event. `MinimalPlugins` excludes `InputPlugin`, so `Messages<KeyboardInput>` doesn't exist. Even if the system early-returns (via mode guard), Bevy validates all system params before the body executes — causing a panic on the `Res<Messages<KeyboardInput>>` access inside `MessageReader`.

**How to apply:** In any test app with `MinimalPlugins` that registers a system using `MessageReader<KeyboardInput>`, add `app.add_message::<KeyboardInput>()` explicitly (or add `InputPlugin` — but that brings in mouse-input resources that can panic leafwing tests). See `guild_create.rs`'s `make_create_test_app()`. The same pattern applies to any Bevy built-in event type normally registered by an omitted plugin.

See also: [[feedback_message_writer_needs_registration]] for `MessageWriter<T>` registration.
