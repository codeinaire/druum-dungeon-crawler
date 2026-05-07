---
name: Bevy B0002 — MessageWriter<T> conflicts with MessageReader<T> in sibling systems
description: MessageWriter<T> takes exclusive ResMut access; MessageReader<T> takes shared Res access — they conflict even in chained system sets
type: feedback
---

Bevy 0.18's `MessageWriter<T>` requires `ResMut<Messages<T>>` (exclusive) while `MessageReader<T>` requires `Res<Messages<T>>` (shared). Bevy's conflict detection fires B0002 if any system pair in the same schedule has this pattern — even with `.chain()` or `.after()` ordering.

**Why:** Bevy's access conflict detection is structural at registration time. `.chain()` enforces ordering but does not resolve the underlying read-write conflict on the same resource.

**How to apply:** If a system both needs to write a message AND is ordered alongside systems that read the same message type, you cannot use `MessageWriter<T>` in the writer system alongside `MessageReader<T>` in the readers within the same Update schedule. Solutions:
1. Write the message in a SEPARATE system that runs in a different schedule (e.g., FixedUpdate vs Update)
2. Mutate state directly instead of publishing the message, and let downstream systems react to the mutated state on the next frame
3. Use `ResMut<Messages<T>>` directly and accept the exclusive lock (removes all readers from the same schedule)

In Feature #13, `apply_teleporter` needed to re-publish `MovedEvent` after same-floor teleport. The B0002 conflict required removing the re-publish; same-floor teleport instead mutates `GridPosition`/`Facing`/`Transform` directly.
