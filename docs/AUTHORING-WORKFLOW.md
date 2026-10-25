# Card and engine-capability execution

Use this short execution path for card campaigns. Scope, models and delivery permissions come
from current user/campaign policy, not an older checklist. Root/subsystem AGENTS.md retain authority.
Routine cards start with a shipped analogue and its relevant assertions. Reuse instructions already
present in context; consult the authoring reference for unfamiliar rules and semantic differences,
not as a mandatory opening-section read for every card. Generator/workbench references are needed
only when using or changing those tools. A concrete missing semantic contract takes the capability
path; a missing card file alone does not.

| Requirement | Source of truth |
|---|---|
| Scope, models, permissions and completion | Current campaign policy, e.g. [four-deck policy](prompts/four-deck-authoring-goal.md) |
| Source research, typed semantics, presentation and admission | [Card authoring reference](../tricerules/tricerules-cards/authoring/CARD-AUTHORING.md) |
| Supported finite authored-program syntax and target bindings | [Current program subset](../tricerules/tricerules-cards/authoring/CARD-AUTHORING.md#finite-authored-programs) |
| Commands and final affected-side verification | [Verification guide](AGENT-VERIFICATION.md) |
| Relevant engine interactions | [Interaction checklist](RULES-INTERACTION-CHECKLIST.md) |
| Tool arguments and fixture boundaries | [Workbench reference](../tricerules/tricerules-cards/authoring/BATCH-WORKBENCH.md) |
| Independent semantic review | [Reviewer contract](CARD-REVIEW.md) |

Keep one concise record with exact source/rulings, copy-from definition/map, relevant test assertions,
differences and acceptance command. Supply those paths directly in a handoff instead of making each
worker rediscover them. Reuse dependency-fresh sources, candidate decisions, checks and prior review
for unchanged content; refresh affected evidence when its inputs or assumptions change. Packets retain
mechanical facts, not semantic approval. No new index, schema, packet or timing file is needed for
routine work; use optional tools only when they save work on the actual batch.
The [composable execution contract](COMPOSABLE-CARD-EXECUTION.md) is the broader migration target;
its proposed types and operations are not all accepted authoring syntax.
Checked-in generated RON uses compact typed renderers; unresolved clone drafts retain explicit
defaults and review sentinels.

## Routine cards: reuse first

1. Confirm exact corpus identity, current registration and live ownership; reuse valid saved decisions
   and source/rulings. Start from the closest shipped definition/map and the assertions in its tests.
   Stop searching when a suitable analogue is found. Preflight is **copy from; differences; reused
   evidence; distinguishing tests; gaps**, recorded once rather than in a separate dossier.
2. Adapt the definition/map, preserving its valid primitive-list or Program form unless the new
   semantics require a change. Check copied identity, quantities, costs, player/target scope,
   ordering, choices, tokens, presentation and source comments. Reuse shared actual-card scenarios
   with independent expected outcomes; add focused coverage for differences and prove intended red
   before implementation, then green. Every clause and relevant illegal path still needs asserted
   execution evidence for the new card. Read related APIs only where the analogue/tests leave a gap;
   do not audit unrelated cards, reload whole API files or migrate an analogue as preparation.
3. Prepare compatible ready cards together using the verification guide: format, focused tests,
   lint, structural evidence and metadata. Reuse unchanged passing prechecks.
4. Freeze explicit intended paths/evidence. Obtain independent inspection-only semantic review.
   Correct material findings and review affected deltas when prior assumptions remain valid.
   Formatting, clerical fixes and a planned passing final gate do not trigger another review;
   defer optional polish. Changes affecting semantics or required evidence need delta review.
   Preserve one evidence record; follow-up requests reference the existing contract.
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
