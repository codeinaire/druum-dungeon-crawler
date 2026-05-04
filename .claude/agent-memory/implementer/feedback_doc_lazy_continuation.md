---
name: clippy::doc_lazy_continuation — continuation lines in doc lists need indentation
description: Doc comment list items following a bullet list need indentation (or blank line) to avoid doc_lazy_continuation lint under -D warnings
type: feedback
---

When adding a line after a bullet-list item in `//!` or `///` doc comments, either indent it (as a sub-item) or add a blank `//!` line before it to start a new paragraph. A bare "Plus ..." or "Note ..." continuation line after a bullet list triggers `clippy::doc_lazy_continuation`.

**Why:** `clippy::doc_lazy_continuation` is enabled under `-D warnings`. The lint fires when a non-blank, non-indented line follows a list item because rustdoc interprets it as a lazy continuation (ambiguous rendering).

**How to apply:** In any `//!` module-level doc or `///` item doc that has a bulleted list, ensure any following text is either:
- A new list item (`- ...`)
- Indented to match the list level
- Separated by a blank `//!` line (new paragraph)
