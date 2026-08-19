# Issue tracker

> **Status (2026-08-19):** the active tracker moved to
> [GitHub Issues](https://github.com/pizzaninja188/Cockatrice/issues).

GitHub Issues is the sole source of truth for current work, priorities, status,
and dependencies. Query the live tracker before selecting or planning work, then
reconcile the selected issue with current code and Git history.

The migration preserved every legacy numbered item: file-tracker IDs `#1`
through `#130` are the same GitHub issue numbers. The two former unprioritized
backlog entries became GitHub issues `#131` and `#132`. Completed file-tracker
items are closed with the `historical-completed` label and include their
tracker-removal evidence; their GitHub created and closed timestamps reflect the
migration rather than their original implementation dates.

For new work, create a GitHub issue and use its assigned number. Close work with
an implementation commit or pull request that uses `Fixes #N`, or close the issue
with a comment linking the implementation commit and verification evidence.

This file remains as a stable pointer for historical source comments that refer
to `docs/issues.md #N`; it must not become a second task list.
