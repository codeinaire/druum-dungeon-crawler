---
name: druum-gitbutler-stacked-branch-creation
description: `but commit <new-branch-name>` no longer auto-creates branches in current `but` version (CLAUDE.md guidance is outdated) — must run `but branch new <name> --anchor <parent>` BEFORE staging when creating a stacked branch
metadata:
  type: feedback
---

When creating a stacked branch in Druum (e.g., Phase 2 stacked on Phase 1's `feature-20a-spell-registry`), the CLAUDE.md guidance saying "`but commit <branch-name>` with a branch that doesn't already exist will create a NEW branch with that name and route the commit there" is **outdated**. The current `but` version errors with `Branch '<name>' not found`.

**Correct stacked-branch creation sequence:**

```
but branch new feature-20c-spell-menu --anchor feature-20b-skill-trees    # create FIRST
but rub zz feature-20c-spell-menu                                          # then stage
but commit --message-file <path>                                           # then commit (auto-routes to applied branch)
btp feature-20c-spell-menu                                                 # then push
gh pr create --base feature-20b-skill-trees --head feature-20c-spell-menu  # then open PR
```

**Why:** Discovered during Phase 2 ship friction on Feature #20 (2026-05-14). The `but commit <new-name>` call failed with `Branch 'feature-20b-skill-trees' not found`, requiring a retry with `but branch new --anchor` upfront. Phase 3 applied this fix cleanly. The CLAUDE.md "Common pitfalls" section needs updating, but until then this memory is the source of truth.

**How to apply:** Any time you're about to create a new branch (especially a stacked one), use `but branch new <name> --anchor <parent>` first. Only after the branch exists do you stage and commit. If you're appending a fixup commit to an *existing* branch, the sequence is just `but rub zz <existing-branch>` → `but commit --message-file` → `btp <existing-branch>` (no `but branch new` needed).

Related: project's CLAUDE.md `but` command mapping (outdated for branch creation).
