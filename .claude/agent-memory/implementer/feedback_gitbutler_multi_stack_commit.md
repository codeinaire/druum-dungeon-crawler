---
name: GitButler — unapply competing branches before committing
description: With multiple applied stacks in GitButler workspace, but commit <branch> fails with cherry-pick merge conflict; unapply competing branches first
type: feedback
---

When two or more GitButler branches are applied simultaneously (visible as multiple `╭┄` stacks in `but status`), `but commit <branch-name>` fails with "Failed to merge bases while cherry picking" — even with `--only`.

**Why:** GitButler's workspace commit merges all applied branches into a single working tree. Committing to one branch while another is also applied causes a cherry-pick base conflict.

**How to apply:** Before committing to the new feature branch, run `but unapply <other-branch>`. Commit all feature-branch changes first. You can re-apply the other branch afterward if needed. The other branch's changes remain safely stored in GitButler — `but unapply` is non-destructive.

Pattern for a clean commit workflow when starting a new feature while a prior feature's branch is still applied:
1. `but unapply <prior-feature-branch>`
2. Make changes, `but rub zz <new-branch>`, `but commit --message-file <file>`
3. Optionally `but apply <prior-feature-branch>` to restore it
