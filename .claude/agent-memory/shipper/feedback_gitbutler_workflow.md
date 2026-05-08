---
name: GitButler workflow enforcement
description: This project uses GitButler (but CLI) for all branch/commit/push operations; raw git commit is blocked by a pre-commit hook on gitbutler/workspace
type: feedback
---

Always use `but` for any operation that mutates branch state in this project.

**Why:** The `gitbutler/workspace` branch has a pre-commit hook that rejects direct `git commit`. A prior session tried `git commit` and was blocked, wasting a turn.

**How to apply:**
- Stage: `but rub zz <branch>` (not `git add`)
- Commit: `but commit --message-file <path>` (not `git commit -m`)
- Push: `but push -u origin <branch>` (not `git push`)
- Create branch: `but branch new <name>` (not `git branch`)
- In non-interactive bash: use full `but ...` form (zsh aliases don't load)
- In interactive suggestions to the user: prefer `bt`-prefixed aliases (`bts`, `btp`, etc.)
