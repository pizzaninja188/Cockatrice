---
name: cockatrice-workflow
description: Plan Cockatrice fork issues, implement an approved rules or client change, and carry out explicitly requested delivery using the repository's Windows verification and card-data workflows. Use for Cockatrice issue work, not unrelated projects or general MTG questions.
---

# Cockatrice workflow

Determine the phase from the current request and accepted plan. Carry forward the user's testing
availability, scope, and existing delivery authorization. An accepted plan authorizes its
implementation; it does not by itself authorize a commit, push, or issue mutation. Do not reopen
settled design choices or ask again for actions already authorized.

The repository root is three directories above this skill. Read the root
[AGENTS.md](../../../AGENTS.md) and only the subsystem guides relevant to the task. They own the
requirements; this skill routes the work rather than replacing them.

## Select and plan

- Query the live `pizzaninja188/Cockatrice` tracker using `gh` with the explicit repository. Check
  dependencies, current code, local history, and existing changes before selecting a candidate.
  `docs/issues.md` is a pointer; upstream Cockatrice issues are a different queue.
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
- Follow the [verification ladder](../../../docs/AGENT-VERIFICATION.md): focused red/green tests
  through the quiet runner, then the full affected-side entry point. Choose the affected side
  from the actual contract, not merely changed file extensions. Use Preview if the selected
  final command sequence needs inspection.
- When authored cards change generated metadata, explicitly run
  `scripts/update-card-data.ps1 -Mode Refresh` from the root and inspect the generated diff.
  Final card verification uses the read-only Check mode through
  `scripts/verify.ps1 -Side Rust -CardData` or `-Side Both -CardData`.
  Do not silently refresh external sources or accept unrelated generated churn.
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
