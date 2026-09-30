# Prepared batches and external semantic rows

These optional tools reduce repeated preparation and test-writing. They do not decide rules,
admit cards, fetch sources, approve semantics, or replace the full affected-side gate. Production
still loads embedded RON. Prepared outputs belong under ignored `build/`, not a new tracker.

## Prepare several candidates once

Write exact Oracle names, one per line, then:

```powershell
./scripts/card-author.ps1 prepare --names build/next-names.txt --corpus build/deck-coverage/oracle-map.tsv --out build/next-preparation
./scripts/card-author.ps1 queue-check --entry build/next-preparation/dependencies.json
```

Use `--bulk` for an existing local bulk with matching adjacent metadata. Preparation loads the
source and live on-disk registry once for the whole batch. It preserves exact source records/all
faces, numbered Oracle lines, registered status, ranked analogue paths/hashes, source differences,
and existing review-map references. Similarity is advisory. All routes start `unassessed`.
The optional corpus must contain exactly one matching Oracle ID/canonical-name row per candidate;
this check preserves the corpus's existing provenance limitations, not deck-section proof.
Output directories must be new and outside embedded data.

Freshness fingerprints sources, corpus, packet, selected analogue definitions/maps, engine/schema,
fixture helpers and lockfile. Unrelated card additions do not invalidate an unchanged analogue.
Missing/changed inputs invalidate preparation. Recheck live ownership separately before selection;
an old issue snapshot or `registered: true` does not establish complete-card support.

Use `clone` to copy the selected analogue into a draft outside `data/`. Resolve the existing
mechanics/presentation sentinels after exact source review. Keep readiness assessments concise.
Do not broaden a batch to fill a target count when compatible ready cards are unavailable.

## Remember assessed candidates by exact identity

After a concrete preflight, save the exact Oracle ID, routing decision and reason once:

```powershell
./scripts/authoring-batch.ps1 candidate-save --oracle-id <id> --name "Exact Name" `
  --status blocked --reason "Missing recipient choice" --depends build/source.json `
  --depends build/rulings.json --depends tricerules/tricerules-core/src/engine/relevant_contract.rs `
  --out build/candidates/exact-name.json
./scripts/authoring-batch.ps1 candidate-list --directory build/candidates
```

Include source, rulings, relevant implementation/schema contracts and any selection prerequisites
that could invalidate the assessment. File membership additions require including the relevant
module/index file too. Unrelated card changes leave the decision intact; changed or missing
dependencies reopen it as `unassessed` with the prior reason preserved. Add a new version of the
record after reassessment; saves refuse overwrite. `ready` is a routing claim, never approval.
Check this queue before selecting new work, then recheck current registry and live ownership.
Do not repeatedly investigate a fresh blocked/held identity to avoid an unresolved policy question.

## Generate evidence scaffolds from typed definitions

```powershell
./scripts/card-author.ps1 inspect --draft build/draft.ron
# Save the JSON result as build/typed.json; source.json is the exact saved card API record.
./scripts/authoring-batch.ps1 map-scaffold --typed build/typed.json --source build/source.json `
  --test "scenario module::actual_card_test" --out build/map-scaffold
```

The new directory contains `review-map.json` and a hashed `typed-paths.json` catalogue. Recursive
OracleLines mappings include nested granted abilities; unmapped clauses stay explicitly unresolved.
Token references are collected with the final validator's CreateTokens/CreateAttackingTokens rule.
Primitive references and independently asserted coverage descriptions still need manual review.
The map always starts unconfirmed, including when every line has an explicit presentation pointer.
Neither copying OracleLines nor a successful structural inspection proves mechanical equivalence.
Scaffolds cannot be emitted into embedded data and refuse to overwrite existing outputs.

Use `prepare-card-batch.ps1 -FocusedTests ... -ReviewMapPath ...` before freezing review. The optional
focused-test file is an array of exact `{package,target,test,features?}` entries. Test both default
and authoring feature configurations when conditional helper code changes. Resolve structural
findings before review, then run one final gate on the stable compatible batch and deliver it.
Do not add another card after final verification while delivery is pending; prepare the next queue
read-only, or resolve the blocker. The final gate always checks the complete corpus and approval.

## More reusable, independently expected rows

For engine capability preparation, use `scripts/prepare-engine-batch.ps1` as described in
[the verification guide](../../../docs/AGENT-VERIFICATION.md#engine-capability-preparation).
It shares exact-test selection with card preparation, accepts `target: "lib"` for library tests,
retains failure logs, and optionally freezes explicit paths plus evidence without granting approval.

The shared schema is `src/authoring_schema.rs`. The calibration matrix is
`../tricerules-core/tests/scenario/authoring_extended_rows.json` (relative to this package).
Rows name real cards and expectations supplied by the author, never inferred from RON effects.
Existing draw/destruction rows remain supported. New families are deliberately limited:

| Family | Supported surface and assertions |
|---|---|
| `mana_activation` | Untargeted tap ability with one fixed output. Exact mana, immediate resolution, source identity, wrong-player and tapped-source rejection. |
| `pump` | Spell or tap activation targeting an opposing Grizzly Bears. Exact paid mana/P/T, source destination/generation, named gained keywords, untouched bystander, unaffordable/invalid-target rejection, cleanup expiry. |
| `mill` | Spell or tap activation targeting player 0/1. Exact payment, source destination/generation, library order, milled physical objects/generations and graveyard membership, unaffected player, nonexistent-player rejection. |
| `upkeep_damage` | At most one nonlethal trigger on opponent upkeep; independent hand sizes at trigger/resolution, exact trigger count and damage, unaffected controller, no controller-upkeep trigger. Covers intervening conditions and live amounts. |
| `graveyard_recovery` | Target one own graveyard card into hand, spell or activation. Exact payment/returned identity/hand size, unaffected same-name cards, opponent-target rejection, source-self rejection before sacrifice. |

Pump/mill/recovery use optional `ability_index`; omission means a spell. Mana is `[W,U,B,R,G,C]`.
Pump expects final `power`/`toughness` of the 2/2 target, not deltas. Keywords use the existing
enum spelling, e.g. `"Trample"`. Recovery requires `target` and `sacrifice_source`. Other choices,
variable X, recipients, durations, layouts and compositions need dedicated scenarios. A row proves
only its asserted surface; it never establishes complete support for another ability on the card.

## Shared setup and engine-offered activations

Dedicated scenarios and registry conformance share
`tricerules-core/tests/scenario/helpers/authoring_fixture.rs`. Its `game(seed, players, card, ability)`
seeds the reviewed target/cost resources; `ability_source` places the reviewed activation source.
The narrow resources include Decimate's enchantment, Inventors' Fair's three artifacts,
Trash for Treasure's graveyard artifact, Fanatic's Ferocious/Graveyard setup and Chandra's loyalty.
Add a necessary new resource here once and exercise it in the new card's actual scenario, rather
than building a second unrelated setup inside conformance. Uncovered zone/composition requirements
still need an explicit fixture. Direct setup skips entry events and is not entry-semantic evidence.

For positive activation scenarios, use `helpers::authoring_actions::activation(&mut engine,
actor_id, source_object_id, ability_index)`. It consumes current engine offers for source zone,
generation, targets and nonmana choices, then fills and checks the payment preview. It returns an
error for an unoffered actor/ability or insufficient resources; it does not fund or advance the game.
Its deterministic target/cost choices are for simple fixtures: assert the chosen identities and
independent expected results, and use dedicated selection for more complex cases. Unsupported X,
aggregate/counter costs and other unsupported choices fail closed. Keep raw command constructors
for intentional wrong-player, invalid-target, stale-generation and unaffordable negative tests.

`helpers::pass_priority_round` completes the current round using actual priority holders,
remaining players and already recorded passes;
`advance_to_main1_from_game_start` uses it for both opening steps. It does not answer arbitrary
resolution/target/payment choices or silently drain a scenario. These helpers are test-only;
production, protocol, relay and UI contracts are unchanged.

`helpers::authoring_fixture::library_top(engine, player_index, card_ids)` seeds named physical
objects in the supplied draw order. Tolarian Winds and Greater Good share this setup while retaining
their distinct expected hand/graveyard identities, counts and choices. It bypasses entry events
and is not proof of casting, ownership, library search, or replacement behavior.

```powershell
./scripts/card-author.ps1 validate-batch --batch build/my-drafts/batch.json
./scripts/test-card-drafts.ps1 -BatchPath build/my-drafts/batch.json
```

`validate-batch` shares the executor's strict row schema and runtime registry validation. It rejects
unresolved/duplicate/noncanonical drafts, absent fixture IDs, invalid ability indices, unknown row
fields, empty batches and drafts without mapped rows. It does not execute scenarios. The draft
wrapper runs this cheap check before compiling/running scenarios and honors ambient worker settings.
Edited external RON/rows still do not rebuild embedded card data.

## Structural readiness before review

Write an assessment JSON alongside your drafts. Paths resolve relative to the assessment:

```json
{
  "version": 1,
  "packet": "../next-preparation/packet.json",
  "freshness": "../next-preparation/dependencies.json",
  "draft_batch": "batch.json",
  "cards": [{
    "id": "new_card",
    "route": "reuse_only",
    "differences": "Copied from X; quantity differs; remaining ability covered separately.",
    "rulings": "rulings.json",
    "review_map": "new_card.json",
    "unresolved": [],
    "clauses": [{"face": 0, "line": 1, "row_indices": [0], "tests": []}],
    "interactions": {
      "timing": "Checked against source and analogue",
      "simultaneous": "N/A: explained reason",
      "identity": "Checked generation and source ownership",
      "choices": "N/A: explained reason",
      "costs": "Checked exact costs",
      "targets": "Checked legality and rejection coverage",
      "tokens": "N/A: explained reason",
      "presentation": "Mapped every source line",
      "client": "N/A: existing unchanged contract"
    }
  }],
  "dependencies": []
}
```

Face numbers are zero-based; source line numbers are one-based. Each line needs a mapping to
one or more zero-based row indices for this card or dedicated tests. Dedicated references contain
`package`, `target`, `test` (full registered name) and `source` (Rust file). The early check confirms
the function exists in that source; final CardData still confirms exact registered, non-ignored
references. Rulings must be a saved list response, including `data: []` when none exist. Record all
additional sources/tokens/fixtures under `dependencies`.

```powershell
./scripts/authoring-batch.ps1 preflight --manifest build/my-drafts/assessment.json --exe tricerules/target/debug/card-author.exe --out build/my-drafts/preflight.json
./scripts/authoring-batch.ps1 check --manifest build/my-drafts/preflight.json
```

Preflight requires the assessed identities to equal the validated draft set, exact source names,
complete nonoverlapping map coverage, resolvable typed pointers, every source clause mapped to
evidence, and every interaction addressed. It refuses `engine_capability`/`unresolved` routes.
`new_composition` is allowed after explicit assessment and distinguishing tests. Readiness is
human-declared; explanatory prose is not machine-proven semantics. The output always records
`semantic_approval: false`. Source/rulings verification and independent review remain mandatory.

## Freeze explicit review scope and measure commands

```powershell
./scripts/authoring-batch.ps1 doctor --minimum-free-gib 8
./scripts/authoring-batch.ps1 freeze --path tricerules/tricerules-cards/data/new_card.ron --path tricerules/tricerules-cards/authoring/review-maps/new_card.json --evidence build/my-drafts/preflight.json --evidence build/my-drafts/focused.log --out build/my-review
./scripts/authoring-batch.ps1 check --manifest build/my-review/manifest.json
./scripts/authoring-batch.ps1 phase --name focused --out build/my-timing -- powershell.exe -NoProfile -File scripts/test-card-drafts.ps1 -BatchPath build/my-drafts/batch.json
./scripts/authoring-batch.ps1 timing --directory build/my-timing
```

Freeze requires explicit unique workspace files, includes tracked edits/new files/deletions,
copies scoped files and supplied evidence, and records base SHA and content hashes. It never
stages or touches unrelated files. Check proves freshness only for recorded inputs/artifacts,
not an unchanged entire repository. Supply all material evidence/dependencies and inspect the
frozen patch before assigning independent read-only review. No bundle is an approval.

Doctor reports free space and exact index-lock metadata without modifying either. Low space exits
nonzero; choose the threshold for the planned build. Lock presence or zero length does not prove
staleness. Recovery still requires process/quiescence checks and applicable standing authorization.

Measured phases capture command arguments, UTC start/end, elapsed command wall time, exact exit
code, full output and its hash. Failure output is printed in full; failed attempts remain in timing
totals. Summed commands can overlap and exclude investigation/review/dispatch time. Report those
separately and never label command time as model cost or active authoring effort.

After semantic review, promote the completed RON/map and independently reviewed rows/dedicated
scenarios. Run metadata preparation once, inspect it, then the full `verify.ps1 -Side Rust -CardData`
(or Both when contracts require it). Keep focused lint/format and cheap structural checks before
review/full gates. Reuse unchanged valid delivery evidence; preserve all required acceptance.
