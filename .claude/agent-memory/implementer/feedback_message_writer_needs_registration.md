---
name: MessageWriter<T> in test apps requires explicit app.add_message::<T>() if its plugin is absent
description: Any system with a MessageWriter<T> parameter panics "Message not initialized" if add_message::<T>() was not called — even if the system doesn't write any messages that frame.
type: feedback
---

If a system uses `MessageWriter<SfxRequest>` (or any `Message` type), and the test app does not include the plugin that registers that message (e.g., `AudioPlugin` for `SfxRequest`), every `app.update()` call will panic with "Message not initialized".

This happens even if the system never actually writes the message — Bevy validates all system parameters at registration time.

Fix: explicitly call `app.add_message::<SfxRequest>()` in test app setup when testing systems that write `SfxRequest` without including `AudioPlugin`.

**Why:** `MessageWriter<T>` requires the message channel to be initialized in the World (via `add_message`). Without it, Bevy's system parameter validation fails at runtime.

**How to apply:** When writing tests for `handle_dungeon_input` or any system that writes SFX messages, always call `app.add_message::<SfxRequest>()` in the test app setup alongside `app.add_message::<MovedEvent>()` (which DungeonPlugin registers automatically).
