---
name: cockatrice-workflow
description: Plan Cockatrice fork issues, implement requested rules or client changes, and carry out explicitly requested delivery using the repository's Windows verification and card-data workflows. Use for Cockatrice issue work, not unrelated projects or general MTG questions.
---

# Cockatrice workflow

Determine the phase from the current request and accepted plan. Carry forward the user's testing
availability, scope, and existing delivery authorization. A direct implementation or fix request,
or an accepted plan, authorizes implementation within that scope without another plan approval.
Neither by itself authorizes a commit, push, or issue mutation. Infer routine details from current
code and context; ask only when unresolved information materially changes scope or correctness.
Do not reopen settled design choices or ask again for actions already authorized.

The repository root is three directories above this skill. Read the root
[AGENTS.md](../../../AGENTS.md) and only the subsystem guides relevant to the task. They own the
requirements; this skill routes the work rather than replacing them.

## Select and plan

- Query the live `pizzaninja188/Cockatrice` tracker using `gh` with the explicit repository. Check
  dependencies, current code, local history, and existing changes before selecting a candidate.
  `docs/issues.md` is a pointer; upstream Cockatrice issues are a different queue.
- For high-volume card coverage, use `gen-cards --candidate-report` over the full pinned corpus or
  an exact-name target file. Rank printing-independent unsupported-clause clusters instead of
  authoring alphabetically or by set; the report routes research and never selects mechanics.
- Honor whether the user is available for UI testing. Prefer a bounded Rust/card-data candidate
  when requested, but trace presentation, protocol, relay, physical identity, and Qt consumers
  before declaring those gates N/A.
- Compare proposed primitives with their closest existing consumers. Explain reuse, extension,
  or necessary separation; complete the
  [rules interaction checklist](../../../docs/RULES-INTERACTION-CHECKLIST.md) for substantive ruled work.
- Finish a decision-complete plan for the selected candidate. If current code already implements
  a candidate, continue selection within the user's criteria rather than planning duplicate work.
  Keep this phase read-only, including tracker state.

## Implement and verify

- For card work, load the canonical
  [card authoring guide](../../../tricerules/tricerules-cards/authoring/CARD-AUTHORING.md).
  Use its Oracle/rulings research, complete-support boundary, presentation mappings, and blocker
  tracking. Do not substitute nearby legacy RON for the guide.
- Apply the guide's [format scope and rules correctness](../../../tricerules/tricerules-cards/authoring/CARD-AUTHORING.md#format-scope-and-rules-correctness)
  boundary: campaign format filters select work, not primitive semantics. Check relevant
  cross-cohort interactions without requiring unrelated card authoring or exhaustive searches.
  Follow campaign scope for broader admissions and report their coverage separately.
- Preflight the complete card before writing anything: enumerate every face, clause, cost, target,
  choice, token, and presentation prerequisite. Use reviewed handwritten RON by default for
  supported cards, including repeated templates; reuse a shipped complete recipe when convenient.
  Missing generator recognition is not a runtime blocker. File a blocker for missing runtime, choice, target,
  or presentation contracts. An empty route reports its limitation; it never authorizes widening
  a grammar. A recognized clause or recipe name is not evidence of complete support.
- For an unsupported data-only card or batch, use the source-backed scaffold mode to populate only
  clerical source fields. Keep scaffolds outside embedded `data/`; mechanically author and review
  every unresolved field before removing both sentinels and promoting a file to `.ron`.
- New recipe development needs a concrete expected throughput benefit for a named cohort;
  two matching cards alone do not justify it. Actual generator changes still require stable recipe
  IDs, typed emission, positive calibrations and negative near-misses. Preserve existing generated
  cards and checks; do not migrate them solely to standardize routes.
- Separate ready cards, unassessed cards, generator limitations and genuine runtime blockers.
  Keep one primary issue owner per unimplemented identity, with links to capability dependencies.
  Update obsolete issue requirements before execution. Prefer 5-10 compatible ready cards per
  batch when available, smaller when semantic risk warrants it. Reuse actual-card semantic
  fixtures with independent expectations; add helpers only for demonstrated repetition.
- Follow the [verification ladder](../../../docs/AGENT-VERIFICATION.md): focused red/green tests
  through the quiet runner, then the full affected-side entry point. Choose the affected side
  from the actual contract, not merely changed file extensions. Use Preview if the selected
  final command sequence needs inspection.
- Freeze the intended patch before independent review: snapshot and hash the diff, give a
  read-only reviewer that exact patch plus its evidence, and run the focused and full gates on the
  same frozen content. Return review findings as rework before treating evidence as final.
  Use default effort for routine authoring and review. Run formatting and focused lint before
  review; resolve required findings before the final gate. Optional polish need not cause rework.
  Escalate review depth for demonstrated semantic risk; keep engine and primitive changes on the
  deeper-review path. Reuse prior valid evidence instead of re-running gates solely to measure.
- When authored cards change generated metadata, explicitly run
  `scripts/update-card-data.ps1 -Mode Refresh` from the root and inspect the generated diff.
  Regeneration from existing local inputs is part of authorized card implementation and needs
  no separate approval. Updating external source datasets requires separate authorization.
  A reviewed recipe that deliberately qualifies new cards requires the
  `gen-cards --dry-run --include-new` preview followed by `gen-cards --include-new`; Refresh alone
  updates only already tracked generated files. Inspect every newly generated card before the
  final gate.
  Final card verification uses the read-only Check mode through
  `scripts/verify.ps1 -Side Rust -CardData` or `-Side Both -CardData`.
  Do not silently refresh external sources or accept unrelated generated churn.
- Select shared blockers by verified complete-card unlocks, not unsupported-clause frequency.
  Assign a blocker only when its deliverable is the last remaining reviewed requirement for named
  identities, verify its runtime/protocol/UI impact first, keep one owner for the mechanism, and
  return the unlocked identities to a single routine owner. Do not implement unrelated blockers
  to grow a batch, and never count a blocker-only mapping as implemented coverage.
- For a reported UI defect, use the
  [game guide](../../../cockatrice/src/game/AGENTS.md) to trace the engine offer through physical
  identity into the actual click/render path. Reuse the existing two-client launcher and logged
  dev setup. Record exact setup, observations, and any remaining acceptance steps.
- Report manual acceptance as agent-performed, user-confirmed, deferred, or N/A with a reason.
  Explicit deferral does not block applicable automated verification or authorized delivery.
  Automated E2E success is separate from hands-on GUI acceptance.

## Deliver when requested

- Inspect the intended diff and current verification evidence; stage only the reviewed paths,
  inspect the staged diff, and make the focused commit when authorized. Preserve unrelated edits.
  Reuse passing final gates when tested content, dependencies, and the relevant environment
  remain unchanged, as specified in the verification guide; a commit request alone needs no rerun.
- For an authorized push, verify the configured remote URL and branch against the requested
  destination. Carry the exact destination and issue through the delivery operation. A generic
  implementation request is not publication authorization.
- Complete the authorized push and issue reconciliation, verifying their actual results before
  reporting completion. Do not close an issue solely because a local commit exists. Leave
  unrelated follow-ups alone unless their publication was requested.
- Treat `.git/index.lock: Permission denied` as a permissions failure, not evidence of a stale
  lock. Use the supported approval path for the narrow authorized operation; do not delete locks
  or bypass a rejected review. If an operation remains blocked, name the exact unfinished step
  and the review's stated reason.
- Summarize behavior changed, actual verification exit codes and evidence, manual acceptance,
  delivery state, and MTG applicability. Keep this in the task response; do not create another
  tracker or persistent workflow-state file.
