# Registry conformance coverage

`conformance.rs` checks execution coverage separately from best-effort zone integrity.
It does not replace semantic card scenarios and does not claim to exercise every mode,
alternate casting method, triggered/static ability, X value, or interaction.

## Outcomes and baseline

`baseline.tsv` is sorted by `(card ID, face index, action)`, with four tab-separated columns.
`cast` means a hand cast, `land` means a land play; `ability:N` identifies a face's printed activated ability.
Every registry face and printed activated ability has exactly one row:

- `exercised`: the initial command was accepted, and its stack and all resulting pending
  choices/triggers finished within 256 commands. Immediate mana abilities may finish without
  creating a stack item.
- `unsupported: reason`: the shared fixture cannot prepare the action. This is a fixture gap,
  not a claim that the card is unsupported by the game. Reasons identify missing targets,
  resources/conditions, nonbattlefield sources, or specialized cost/mode selection.
- `intentional: face unavailable from hand`: the registry's face-availability API excludes that
  face from hand casting. Its printed activated abilities are still enumerated independently.

Once execution begins, rejected commands, unhandled choices, missing published offers, and
budget exhaustion **fail** with case/seed/actor/command/pending-state diagnostics; they cannot
be recorded as exercised or silently added as baseline exclusions. Coverage losses, improvements,
new cases, removed cases, duplicate rows, and changed reasons all require baseline review.

Run the normal gate from the repository root:

```powershell
./scripts/run-quiet-command.ps1 -Label 'Conformance' -WorkingDirectory tricerules `
  -Executable cargo -ArgumentList @('test','--quiet','-p','tricerules-core','--test','conformance')
```

For a proposed baseline update, run `report_registry_execution` with
`-- --ignored --nocapture` through the same runner and inspect its retained log. The `COVERAGE`
rows are candidate data only: review every changed outcome and missing-fixture reason before
editing the baseline. Neither this reporting test nor ordinary tests write the baseline.
Do not accept a coverage loss to conceal an engine regression.

## Fixture boundary

Each case starts a fresh ordinary two-player game with seed 221. Existing scenario helpers
place a creature, artifact, basic lands, hand fodder, and a graveyard creature for each player,
with ample mana. Fixture construction directly establishes test state; subsequent actions and
choices use engine commands. Battlefield ability fixtures set the intended face explicitly;
they do not claim to cover that face's entry/transform sequence.

Annul and Flashfreeze fixtures additionally cast a matching artifact or red creature spell
before exercising the counterspell. Get Out uses the existing modal/permanent fixture;
its separate semantic scenarios cover both modes and one/two owned targets.

The driver selects from engine-published target groups, selectable modes, cost candidates,
resolution candidates/slots/branches, and trigger ordering. Payments use engine previews and
are previewed again before submission. The preview's canonical `remaining_cost` alternatives
are already computed by the engine; the fixture chooses the first funded alternative.
It never reads Oracle text to decide legality.

The baseline includes common modal/grouped targeting, discard/sacrifice/composite costs,
hand/library choices, branching, and triggered choices. Dedicated driver checks also exercise
trigger ordering and resolution mana payments. Specialized combat conditions, nonbattlefield
activations, counters/aggregate costs, required cast-cost groups and linked Spree modes remain
explicit gaps. Execution coverage is not semantic correctness or manual GUI acceptance.

`integrity.rs` retains the old command-attempt fixture as a separate safety sweep. Its actually
completed cases must also complete under the strict fixtures. Zone assertions count physical
cards, including parked stack spells, without double-counting continuation snapshots of cards
that already moved. This preserves the old useful invariant without preserving its old claim.

No production rules, protocol, relay, Qt, freeform, or card-data behavior changes here. Full
completion verification is `scripts/verify.ps1 -Side Rust`; manual UI checks are N/A.
