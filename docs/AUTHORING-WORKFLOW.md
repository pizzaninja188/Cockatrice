# Card and engine-capability execution

Use this short execution path for card campaigns. Scope, models and delivery permissions come
from current user/campaign policy, not an older checklist. Root/subsystem AGENTS.md retain authority.
Load reference sections when their subject applies.

| Requirement | Source of truth |
|---|---|
| Scope, models, permissions and completion | Current campaign policy, e.g. [four-deck policy](prompts/four-deck-authoring-goal.md) |
| Source research, typed semantics, presentation and admission | [Card authoring reference](../tricerules/tricerules-cards/authoring/CARD-AUTHORING.md) |
| Commands and final affected-side verification | [Verification guide](AGENT-VERIFICATION.md) |
| Relevant engine interactions | [Interaction checklist](RULES-INTERACTION-CHECKLIST.md) |
| Tool arguments and fixture boundaries | [Workbench reference](../tricerules/tricerules-cards/authoring/BATCH-WORKBENCH.md) |
| Independent semantic review | [Reviewer contract](../.agents/skills/cockatrice-workflow/reviewer-template.md) |

Reuse these records. A checklist may point to an existing preflight, test or review; it need not
repeat the narrative. Packets retain mechanical facts, not semantic approval. Optional indexes,
draft schemas and packet generation are unnecessary when existing evidence supplies the contract.

## Reuse-only cards

1. Confirm exact corpus identity, current registration and live ownership. Reuse dependency-fresh
   saved source/rulings and candidate decisions. Inspect the closest shipped definition and its
   assertions. Preflight: **copy from; differences; reused evidence; distinguishing tests; gaps**.
2. Author complete definitions/maps. Reuse independently expected actual-card scenarios for
   unchanged patterns; add focused tests for semantic differences. Every clause still needs
   executed/asserted evidence and relevant illegal paths. Do not audit unrelated cards.
3. Prepare compatible ready cards together using the verification guide: format, focused tests,
   lint, structural evidence and metadata. Reuse unchanged passing prechecks.
4. Freeze explicit intended paths/evidence. Obtain independent inspection-only semantic review.
   Correct material findings and review affected deltas when prior assumptions remain valid.
5. Run the full affected-side gate on stable reviewed content, record acceptance, and deliver under
   existing authorization before another code increment. Do not ask again for an authorized
   scope/destination. Actual sandbox rejection remains an access boundary.

## Necessary engine capabilities

1. Select a concrete missing contract by named complete-card unlocks in the selected corpus.
   Check remaining card prerequisites; a primitive-only mapping is not a complete card. Compare
   the closest existing primitive. Generalize only for demonstrated uses; justify specialized
   behavior if only one use fits.
   Use existing analogue, candidate and dependency reports to inspect several representative
   remaining cards before choosing a capability family. Compare shared semantics and differences
   in ordering, replacements, layers, identity and private choice/resume behavior against current
   code, then select the smallest complete shared primitive. Reports route this comparison; they
   do not prove semantic compatibility. Avoid speculative extensions, unrelated audits or extra
   dossiers, and deliver a stable verified batch before expanding it.
2. Before substantial implementation, obtain bounded independent design review: intended behavior,
   exact sources/rulings, existing APIs/analogue, affected producers/consumers, choice/resume,
   identity, relevant checklist risks and proposed regressions. One concise record suffices.
   This settles technical design, not user permission or final code approval.
3. Write/run the smallest regression and confirm the intended red. Implement one coherent increment
   and prove focused green. Cover relevant ordering, multiplayer actors, stale identity, rejected
   input preserving state, and interrupted resolution. N/A needs a reason. Keep expected outcomes
   independent of emitted effects. Share setup only for demonstrated reuse.
4. Stabilize with relevant package/interaction checks. Integrate actual cards whose final requirement
   is met. Prepare/freeze the implementation and evidence, then obtain independent final review.
   Early design review does not replace frozen-patch review or full final affected-side verification.
5. Deliver the bounded verified batch with acceptance deferrals recorded. Read-only preparation of
   the next capability may overlap review; writers, builds and Git operations remain serialized.

## Evidence and throughput

Record sources, exact tests/results, patch identity and verdict once; link them from other records.
Measure research, implementation/first green, preparation, review/rework, final gates, recovery and
delivery separately. Preserve failed attempts/carry-in preparation. Separate engine from reuse-only
work, command spans from elapsed session time, and pauses from active authoring windows. Do not rerun
gates merely for timing. Admission remains fail-closed.
