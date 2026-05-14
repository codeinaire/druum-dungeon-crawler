---
name: druum-fix-review-findings-before-completion
description: For Druum review cycles, default to fixup-first-then-next-phase rather than defer-and-move-on — user chose this 3-for-3 across Feature #20 phases for all LOW findings, including cosmetic ones
metadata:
  type: feedback
---

When code-reviewer flags LOW (or higher) findings on a Druum PR, the user's consistent preference is to apply a narrow fixup commit to the existing branch **before** moving to the next phase or closing the pipeline — even for cosmetic-only findings like missing doc-comments. Three-for-three across Feature #20 (Phase 1 MED+LOW, Phase 2 2×LOW, Phase 3 1×LOW). Each cycle: narrow implementer brief, user-driven ship (gates + `but rub zz` + `but commit` + `btp`), narrow re-review via addendum appended to existing review file.

**Why:** The user values a clean review record at PR-merge time. Defer-and-move-on leaves open review threads that compound over multiple PRs; fixup-first keeps each PR's review state at zero open findings at merge time. The user said this explicitly during Phase 3's post-review checkpoint: "I want the fixup despite the 'skip is also valid' framing — consistent across all three phases."

**How to apply:** When presenting review findings to the user at the post-review checkpoint, frame the question as "Apply fixup or skip?" rather than implying skip is fine. Anticipate fixup. Pre-stage the fixup implementer prompt as the default next action. Only skip if the user explicitly says so. Each fixup cycle pattern:

1. Implementer with narrow brief listing the exact files + lines + change shape (verbatim from review file).
2. Implementer stops at gate — user runs gates + stages + commits + pushes to the **existing** branch (no new branch, no new PR).
3. Narrow re-reviewer reads only the fixup commit, appends an **addendum** section to the existing review file (do not create a new review file), and queues the addendum body for posting to the existing PR thread.

Related: [[druum-dungeon-assets-fixture-fan-out]] (one class of finding that recurs across phases).
