# Four-deck new-card campaign policy

Finish source-backed ruled support for missing/new cards in the mainboards and commander sections
of these original decks. Exclude sideboards, considering/maybeboard sections, tokens and other decks.

- https://moxfield.com/decks/QSPE5wn8MUuMgx7tnv2Leg
- https://moxfield.com/decks/Zz1Ss9NGqka3P2UXo33Ffg
- https://moxfield.com/decks/dIk0nOs7k0SHGbUQBzPkIQ
- https://moxfield.com/decks/ITpDyA9B3k69J2FUaNA5oA

## Authority and scope

Work in C:\Users\pizza\CodingProjects\Cockatrice. Implementation of selected new cards and their
necessary engine capabilities, directly affected issue reconciliation, scoped reviewed commits on
master, and pushes to origin/master are explicitly authorized for this entire session. Carry this
permission forward; do not ask for renewed delivery approval. Never bypass sandbox review.
Preserve unrelated changes. There is one writer; only the root runs builds, tests, generators,
formatting, metadata and Git operations. Do not resume or message superseded paused goals.

General audits of existing registrations are deferred and are not a completion gate. Investigate
existing cards only for a concrete defect or a prerequisite of the selected new-card batch.
Preserve known defects and fail-closed admission. A registration, recipe or primitive is not proof
of complete support. Completion requires the remaining new-card scope, correctness, semantic and
presentation evidence, all required verification, acceptance and delivery.

Attempt bounded live deck retrieval. If inaccessible, the authorized operational fallback is
build/deck-coverage/oracle-map.tsv: 313 exact identities, SHA-256
1162a2f4aaa535e26d052fc72128ae5dad4bbb5e050c409766171918b0b10c55.
Verify this hash; its deck-section provenance remains unresolved. Do not imply live section proof.

## Model and collaboration policy

Root: gpt-6-luna MAX, explicitly requested for the new session. Narrow source preparation:
Luna MEDIUM. Routine independent review:
Luna HIGH. Substantial engine design and frozen implementation review: independent Sol HIGH;
use Sol XHIGH for difficult layers/dependencies, replacement ordering, resumable choices,
physical identity, hidden information, or a material finding unresolved at HIGH. Record actual
worker dispatch settings and escalation reason. Keep the requested root model; independent
reviewer models do not change the root model.

Use at most three read-only workers plus the root. Reuse workers where suitable; use self-contained
bounded packets for explicit model overrides. Workers inspect or propose; the root applies changes.
Independent next-capability research may overlap review, without competing writers or Cargo runs.
Use ambient/default build/test worker counts for ordinary checks. For bounded final Rust/CardData
coverage runs, use the configuration documented in
[the architecture improvements](../CARD-AUTHORING-ARCHITECTURE-IMPROVEMENTS.md):
`TRICERULES_CONFORMANCE_WORKERS=4`, `RUST_TEST_THREADS=1`, `CARGO_BUILD_JOBS=4`.

## Execution and handoff

Follow [the authoring execution path](../AUTHORING-WORKFLOW.md), loading only applicable reference
sections. Group compatible ready cards (often 3-8) or named complete-card unlocks sharing a necessary
engine contract. Do not delay ready delivery to reach a count or force a speculative larger batch.
Prioritize a verified playable deck, then the next, while retaining the entire four-deck objective.
Choose the completion path by expected remaining implementation and acceptance work, rather than
missing-card count alone. Start by assessing Kami's nine checkpointed remaining identities and its
hard blockers; compare another deck if its completion path is cheaper. Include compatible shared
cards and ready cards from other selected decks when they amortize preparation/review/final gates.

Start from the reviewed architecture improvements and refresh exact blocker classifications.
Flux (Kami) and Abundance (Bello) are in the frozen corpus and can use the new primitives; preflight
their complete cards before admission. Demonstration consumers outside the corpus, including
Tomorrow, Shoreline Looter, Accumulate Wisdom, Tersa and Steal the Show, do not expand this campaign.
Preserve the distinction between runtime support, generator recognition and whole-card readiness.
Deliver stable verified content before another code increment. During delivery waits, do only bounded
independent preparation. Current permissions and this policy override stale task notes.

Start with current HEAD, remote, worktree, live tracker and saved checkpoint/candidate evidence.
Use dependency-fresh source preparation; do not restart inherited research. The launch message
supplies the latest delivered SHA, counts, acceptance deferrals and carry-in work. Reconcile exact
identities before repeating counts. Old checkpoints and historical audits are context, not deliveries.

For confirmed stale Git index locks, existing user authorization allows narrow recovery: serialize,
quiesce writers and relevant Git/build/test processes, inspect the exact unchanged lock, then remove
only that confirmed stale file through supported approval. Zero size alone is insufficient; never
remove an active/ambiguous lock or change ACLs. Read-only Git uses --no-optional-locks.

Required visible two-client acceptance uses trusted native computer control and interactive Windows
ownership. If unavailable or stopped by physical Escape, stop and record unperformed acceptance.
Explicit manual deferral permits automated verification/delivery; do not mark deferred acceptance
performed or declare the entire scope complete while acceptance requirements remain unresolved.

## Measurement and continuation

Aim for a 50% improvement only on genuinely comparable completed batches. Follow the execution
path's phase accounting; retain failures, carry-in work and supervisor pauses. Separate engine
capability work from reuse-only authoring, elapsed session time from command/worker spans, warm
from cold caches when known, and historical audits from new complete-card unlocks. Model, workflow
and tooling change together: do not claim an isolated model effect or invent cost/usage.

Continue the same original new-card objective until complete. Report concise verified delivery
checkpoints, per-deck readiness, exact remaining blockers and acceptance limits. Do not stop merely
because a batch ended; settled scope, model and delivery permissions require no new user decision.
Track time to verified deck readiness alongside complete-card delivery counts. The measured
conformance improvement applies to that test stage; measure total authoring throughput separately.
