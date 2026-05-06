---
name: GitHub own-PR review limitation
description: REQUEST_CHANGES is blocked on your own PR; fall back to --comment via gh pr review
type: feedback
---

GitHub API (and `gh` CLI) refuses `REQUEST_CHANGES` reviews on PRs authored by the same token owner: "Review Can not request changes on your own pull request." The `mcp__github__create_pull_request_review` tool with event=REQUEST_CHANGES fails with a permission error for the same reason.

**Why:** GitHub enforces this rule at the API level regardless of token permissions.

**How to apply:** When the PR author and the reviewer share the same GitHub account (codeinaire), use `gh pr review <num> --comment --body "..."` instead of `--request-changes`. The review content is identical; only the approval/block signal is missing. Note this in the review summary so the human can manually act on it.
