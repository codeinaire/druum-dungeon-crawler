# CLAUDE.md — Druum project guidance

## Version control: use GitButler (`but`), not `git`

This project uses [GitButler](https://gitbutler.com) for commits, branches, and pushes. The `gitbutler/workspace` branch is GitButler-managed; a pre-commit hook **blocks direct `git commit` on it**.

**Default to `but` for any operation that mutates history or branch state.** Reach for `git` only for read-only inspection (`git log`, `git diff`, `git show`) or operations GitButler doesn't cover (`git fetch`, `gh ...`).

### Command mapping

| Operation | Use | Not |
|---|---|---|
| Stage uncommitted changes to a branch | `but rub zz <branch>` (or `but stage <file> <branch>`) | `git add` |
| Commit (auto-routes to applied branch when only one is staged) | `but commit --message-file <path>` | `git commit -m` |
| Push (runs husky hooks first via the `btp` alias) | `but push -u origin <branch>` | `git push` |
| List GitButler stacks + their commits | `but status` | (`git status` works for working-tree only) |
| Create a branch in the workspace | `but branch new <name>` | `git branch <name>` |
| Delete a stray/empty branch | `but branch delete <name>` | `git branch -d <name>` |
| Move a commit to a different branch | `but rub <commit> <branch>` | `git cherry-pick` |
| Undo last commit (back to unassigned) | `but uncommit <commit>` | `git reset HEAD~1` |
| Pull from remote | `but pull` | `git pull` |
| Operation history (safe-undo source) | `but oplog` | `git reflog` |

### Why

- **GitButler stacks are first-class.** Multiple parallel branches share one workspace; `but` understands them, `git` doesn't.
- **Pre-commit hook on `gitbutler/workspace`** rejects direct `git commit`. Trying it wastes a turn — see prior session where `git commit` was blocked and we had to redo via `but commit`.
- **`but undo` and `but oplog`** are non-destructive — safer than `git reset --hard`.
- **The husky-hook wrapper** in the user's zsh plugin (`btp` alias) ensures pre-push hooks run; raw `git push` skips them.

### Common pitfalls

- **`but commit <branch-name>`** with a branch that doesn't already exist will create a NEW branch with that name and route the commit there — NOT append to your existing branch. To append: stage to the right branch first (`but rub zz <branch>`), then `but commit` with no positional arg.
- After `but commit`, if a stray branch was auto-created, `but uncommit <commit-sha>` returns the changes to unassigned and `but branch delete <stray>` cleans up.
- `but commit` requires `--message-file <path>` for multi-line messages — heredoc on `-m` is not reliable through the `but` invocation.

### Shortcuts

The user's zsh plugin at `~/.oh-my-zsh/custom/plugins/gitbutler/gitbutler.plugin.zsh` defines `bt`-prefixed aliases (`bts` = status, `btc` = commit, `btp` = push-with-hooks, `btpu` = pull, `btu` = undo, `bto` = oplog, `btrb` = rub, `btbd` = branch delete, etc.). When suggesting commands the user will run interactively, prefer the short alias. When running commands myself in a non-interactive bash, use the full `but ...` form (zsh aliases don't load).

### What still uses `git`

- `git fetch origin` (no `but` equivalent for fetch-only without applying)
- `git log`, `git diff`, `git show`, `git blame` (read-only inspection)
- `gh pr ...`, `gh api ...` (GitHub CLI is unrelated to GitButler)

### What still uses raw `gh`

PR creation, PR comments, PR review, status checks. GitButler has `but review` for cross-AI review workflows but not for opening regular GitHub PRs from a stacked branch — `gh pr create` remains the path.
