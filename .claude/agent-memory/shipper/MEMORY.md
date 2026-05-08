# Memory Index

- [GitButler workflow: always use `but`, never `git commit/push/add`](feedback_gitbutler_workflow.md) — pre-commit hook blocks raw `git commit` on workspace; `but commit --message-file` is the only safe path
- [Commit message file written to project/shipper/ before committing](feedback_commit_msg_file_location.md) — heredoc on `-m` unreliable through `but`; write to file first, use `--message-file`
- [PR body written to project/shipper/ then passed via --body-file](feedback_pr_body_file_location.md) — avoids shell escaping issues with `gh pr create`
