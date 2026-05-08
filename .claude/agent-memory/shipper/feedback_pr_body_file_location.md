---
name: Write PR body to project/shipper/ then pass via --body-file
description: gh pr create --body-file avoids shell escaping issues; bodies go in project/shipper/
type: feedback
---

Always write the PR body to `project/shipper/<branch>-pr-body.md` and pass it to `gh pr create --body-file`.

**Why:** Inline heredoc bodies in `gh pr create` can mangle Mermaid diagrams, backtick fences, and special characters through shell expansion. `--body-file` is safe.

**How to apply:** Filename pattern: `<feature-slug>-pr-body.md`. Always pair `--head <branch> --base main` explicitly when on `gitbutler/workspace` so `gh` doesn't infer the wrong head.
