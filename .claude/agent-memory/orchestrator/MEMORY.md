# Memory Index

- [Druum DungeonAssets fixture fan-out](druum-dungeon-assets-fixture-fan-out.md) — init_asset registration sites span 7+ files in tests/ and plugins/; new Res<Assets<T>> system params require updating every site or cargo check fails non-obviously
- [Druum GitButler stacked-branch creation](druum-gitbutler-stacked-branch-creation.md) — `but commit <new-name>` no longer auto-creates branches; use `but branch new <name> --anchor <parent>` FIRST; CLAUDE.md guidance is outdated
- [Druum fix review findings before completion](druum-fix-review-findings-before-completion.md) — user's 3-for-3 pattern across Feature #20: fixup-first-then-next-phase, even for cosmetic LOWs; default to anticipating fixup at post-review checkpoint
