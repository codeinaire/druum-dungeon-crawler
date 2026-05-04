---
name: Druum third-party crate Step A/B/C verification gate pattern
description: For ANY new external crate addition (especially when the local Cargo cache lacks the crate), Druum demands a 3-step verification gate BEFORE editing Cargo.toml — established by Features #3 (bevy_common_assets/asset_loader), #5 (leafwing-input-manager), and reused by #10 (bevy_egui)
type: feedback
---

When researching or planning a new third-party Bevy crate addition for Druum, ALWAYS structure the work around the established three-step verification gate. Skipping this has burned past planning rounds (`bevy_kira_audio` deviated to native; `leafwing-input-manager` resolved to 0.20.0 not the 0.18.x roadmap value).

**Why:** Roadmap-suggested versions are stale by the time features are scheduled (one project precedent every 2-3 features). The cost of running the gate is 5-10 minutes; the cost of guessing wrong is hours of debugging Cargo resolution failures or silent feature-flag mismatches.

**How to apply:**

**Step A — Version resolution (NO Cargo.toml edit yet):**
```bash
cargo add <crate> --dry-run 2>&1 | tee /tmp/<crate>-resolve.txt
# Read the resolved version, verify its `bevy = "..."` requirement accepts the project's pinned bevy.
# Halt + escalate to user if incompatible.
```

**Step B — Feature flag audit (NO Cargo.toml edit yet):**
```bash
# After resolving, inspect the [features] block
cat ~/.cargo/registry/src/index.crates.io-*/<crate>-<version>/Cargo.toml | sed -n '/^\[features\]/,/^\[/p'
# Decide: keep defaults if minimal, opt out via `default-features = false` if defaults pull heavy chains
# (egui, asset, serde, accesskit, winit/x11/wayland are common heavy defaults)
```

**Step C — API verification grep (NO Cargo.toml edit yet):**
```bash
REG=~/.cargo/registry/src/index.crates.io-*/<crate>-<version>/src
grep -rn "pub struct <ExpectedType>" $REG
grep -rn "pub fn <expected_method>" $REG
grep -rn "pub trait <ExpectedTrait>" $REG
ls ~/.cargo/registry/src/index.crates.io-*/<crate>-<version>/examples/
```

**Then Cargo.toml gets ONE edit** with:
- Pin with `=<resolved-version>` per Druum convention.
- `default-features = false, features = [...]` if Step B mandated.
- Verify `git diff Cargo.lock` is reviewable in one screen — no unrelated transitive bumps.

When researching such a feature without tooling access (Bash, MCP, WebFetch unavailable), provide the implementer with the verification recipe AND mark the version-related findings MEDIUM until Step A elevates them to HIGH. Do NOT silently assume the roadmap version is correct.

The pattern is documented inline in research docs and the resulting plans for #3, #5, and #10 — see `project/plans/20260502-000000-feature-5-input-system-leafwing.md` Steps 1-4 for the canonical execution.
