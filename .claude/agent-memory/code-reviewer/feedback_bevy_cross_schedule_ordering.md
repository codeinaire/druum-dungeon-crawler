---
name: Bevy cross-schedule .after()/.before() is silently ignored
description: In Bevy 0.18.1, ordering constraints between systems in different schedules (e.g., .after(system_in_Update) on a system in EguiPrimaryContextPass) are silently no-ops; verify ordering via schedule topology instead
type: feedback
---

In Bevy 0.18.1, calling `.after(system_fn)` or `.before(system_fn)` on a system in schedule A, where `system_fn` is registered in schedule B, is silently ignored — no panic, no warning. The constraint is never enforced.

**Why:** `bevy_ecs-0.18.1/src/schedule/config.rs:358` documents: "if `GameSystem::B` is placed in a different schedule than `GameSystem::A`, any ordering calls between them—whether using `.before`, `.after`, or `.chain`—will be silently ignored."

**How to apply:** When reviewing systems that call `.after(some_fn)` and the referenced function is in a different schedule, flag this as dead code. Verify the intended ordering via schedule topology instead (e.g., Update runs before PostUpdate; EguiPrimaryContextPass runs inside PostUpdate). The correct fix is either to remove the dead constraint and add a comment explaining the schedule-topology guarantee, or to move both systems into the same schedule.

Concretely: `MinimapPlugin` painters in `EguiPrimaryContextPass` calling `.after(update_explored_on_move)` (which is in `Update`) — the constraint is silently ignored. Order is guaranteed by Update→PostUpdate topology, not by the .after() call.
