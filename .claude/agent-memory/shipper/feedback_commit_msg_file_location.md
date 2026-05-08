---
name: Write commit message to project/shipper/ before committing
description: but commit requires --message-file with a path; heredoc on -m is unreliable; files go in project/shipper/
type: feedback
---

Always write the commit message to `project/shipper/<branch>-commit-msg.txt` before running `but commit --message-file`.

**Why:** Heredoc on `but commit -m` is not reliable through the `but` invocation (documented in CLAUDE.md). Writing to a file first is safe and auditable.

**How to apply:** Use `project/shipper/` as the canonical staging area for commit messages and PR bodies. Filename pattern: `<feature-slug>-commit-msg.txt`.
