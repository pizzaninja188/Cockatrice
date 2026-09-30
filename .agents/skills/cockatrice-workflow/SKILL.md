---
name: cockatrice-workflow
description: Plan Cockatrice issues, implement requested rules or client changes, and carry out authorized delivery using the Windows verification and card-data workflows.
---

# Cockatrice workflow

Determine the phase from the request. Carry forward scope, delivery authorization and manual
acceptance deferrals. Do not ask again for the same authorized action, scope and destination.
Read root [AGENTS.md](../../../AGENTS.md), then load only relevant subsystem guidance.

| Need | Owner |
|---|---|
| Card or engine-capability execution | [Authoring workflow](../../../docs/AUTHORING-WORKFLOW.md) |
| Exact typed semantics, source research and presentation | [Card authoring reference](../../../tricerules/tricerules-cards/authoring/CARD-AUTHORING.md) |
| Commands, affected-side gates and evidence reuse | [Verification guide](../../../docs/AGENT-VERIFICATION.md) |
| Independent inspection-only review | [Reviewer contract](reviewer-template.md) |
| UI and physical two-client acceptance | [Game guide](../../../cockatrice/src/game/AGENTS.md) |
| Structural ownership | [Architecture](../../../docs/ARCHITECTURE.md) and [roadmap](../../../docs/REFACTOR-ROADMAP.md) |

For selection, query the live `pizzaninja188/Cockatrice` tracker with `gh`, then inspect current
code/history/worktree. Keep planning read-only. Plans and candidate reports are research aids,
not current implementation truth. Use decision-complete issues for new design decisions or
deferred blockers; routine supported-card batches need no separate issue or dossier.

For authorized delivery, inspect valid verification evidence, stage only reviewed paths/hunks,
inspect the staged diff, commit, push to the authorized remote/branch, and verify the remote SHA.
Reconcile directly affected issues when authorized. Preserve unrelated work. A permission error
on the index lock is an access boundary: use supported approval, never bypass it. An ambiguous
or active lock must not be removed; confirmed stale recovery follows existing user authorization.

Report behavior, exact gate status, acceptance performed/deferred/N/A, delivery state and MTG
applicability. Do not create another persistent workflow tracker.
