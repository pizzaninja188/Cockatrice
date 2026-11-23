# Registry conformance coverage

`conformance.rs` checks execution coverage separately from best-effort zone integrity.
It does not replace semantic card scenarios. It does not claim to exercise every mode,
alternate casting method, triggered or static ability, X value, or interaction.

## Outcomes and baseline

`baseline.tsv` has four tab-separated columns and is sorted by `(card ID, face index, action)`.
In the action column:
- `cast` is a hand cast.
- `land` is a land play.
- `ability:N` is the face's Nth printed activated ability.

Every registry face and every printed activated ability is evaluated.

### Outcomes

- `exercised`: the initial command was accepted. Its stack, and every pending choice and trigger
  it caused, finished within 256 commands. An immediate mana ability may finish without
  creating a stack item.
- `intentional: face unavailable from hand`: the registry's face-availability API excludes that
  face from hand casting. Its printed activated abilities are still enumerated independently.
- `intentional: <reason>; covered by <scenario test>`: a reviewed case that no fixture profile
  can prepare. It must have an entry in the `INTENTIONAL` table in `conformance.rs`. The table is
  checked:
  - The named `fn` must exist under `tests/scenario/`.
  - An entry that a profile can prepare fails as stale.

  The table is currently empty.

There is no `unsupported` outcome. A case that no profile can prepare, and that has no reviewed
exclusion, fails the test. The baseline parser rejects `unsupported:` rows. Extend a profile or
the driver instead.

### Failures and baseline rows

Once execution begins, any of these **fails** the test, with case, profile, seed, actor,
command and pending-state diagnostics:
- a rejected command
- an unhandled choice
- a missing published offer
- budget exhaustion

These can never be recorded as an outcome.

New `exercised` cases need no row. Keep existing rows, because they preserve change and removal
detection. These need review:
- new exclusions
- changed or removed rows
- duplicate rows
- changed reasons

The baseline doesn't track the removal of an implicit successful case. Actual-card semantic
tests and registry review stay responsible for which cards belong in the registry.

### Running the gate

Run the normal gate from the repository root:

```powershell
./scripts/run-quiet-command.ps1 -Label 'Conformance' -WorkingDirectory tricerules `
  -Executable cargo -ArgumentList @('test','--quiet','-p','tricerules-core','--test','conformance')
```

Run `report_registry_execution` only when an exclusion or an existing reviewed outcome changes.
Pass `-- --ignored --nocapture` through the same runner, and inspect its retained log.
- The `COVERAGE` rows are candidate data only. Review every changed outcome before editing the
  baseline.
- Neither this reporting test nor any ordinary test writes the baseline.
- Never accept a coverage loss that conceals an engine regression.

## Fixture profiles

Each case starts a fresh ordinary two-player game with seed 221. Profiles are tried in order on a
fresh engine each time. A case runs under the first profile that prepares its initial command,
and a later profile never retries an execution failure.

1. **Base.** For each player, the existing scenario helpers place:
   - a creature, an artifact and basic lands on the battlefield
   - a card in hand
   - a creature in the graveyard
   - ample mana

   Every case is tried here first, so cases that already passed keep their original board.
2. **Rich.** Base, plus vanilla or near-vanilla registry cards.
   - **Player 0's battlefield:** the other basic land types, a flier, a Soldier with +1/+1
     counters, a legendary creature, a Golem, a Vehicle, a tapped Goblin, Food and Treasure
     tokens.
   - **Opponent's battlefield:** a flier, a tapped black creature with a stun counter, an
     enchantment and a nonbasic land.
   - **Both graveyards:** typed cards, plus two extra copies of the case card in player 0's
     graveyard.
   - **Turn history,** made by real commands: a creature is sacrificed, and an instant is cast
     and resolved.
3. **Stack variants.** Rich, then one sorcery-speed spell cast on an empty stack, with any
   instants cast on top while the caster holds priority. There are three variants:
   - a green creature with a blue one-mana instant on top
   - a black four-mana sorcery with a red instant on top
   - a colorless artifact
4. **Combat.** Rich, advanced to declare blockers. Two creatures attack and one of them is
   blocked, all chosen from engine-published attack assignments and block pairs.

Fixture construction establishes test state directly. Every later action and choice uses an
engine command.

### Ability sources

An ability source starts in the zone its definition names (`ActivatedAbilityDef.source_zone`):
hand, graveyard or battlefield. Battlefield sources set the intended face explicitly. They don't
claim to cover that face's entry or transform sequence. Because direct relocation skips entry, a
battlefield source is seeded with:
- its fixed self enters-with-counters amounts
- enough of each counter kind that its own source-paid removal cost names

Planeswalkers are seeded with enough loyalty for the ability under test.

### The choice driver

The driver chooses from engine-published data:
- target groups and selectable modes
- Spree and Tiered linked cast costs
- required cast-cost groups
- cost candidates, including aggregate-minimum (Crew) and counter-removal choices
- resolution candidates, slots and branches
- trigger ordering

It never reads Oracle text to decide legality.

Some activations go through the engine's staged activation window, and the driver commits the
window with its published candidate:
- hand activations that return a tapped creature
- aggregate counter-removal activations, which announce their Phyrexian pips

Payments use engine previews and are previewed again before submission. The driver takes the
first funded alternative of the preview's canonical `remaining_cost`. Generic mana comes from
colorless first, then from whatever colored mana remains.

### Remaining card-named fixtures

A few card-named fixture additions remain in `authoring_fixture::game()` and `fixture_with()`:
- Luxior's planeswalker
- War Room's commander
- the Primeval's, Inventors' Fair, Trash for Treasure, Kuldotha, Metalwork, Fanatic and
  artifact-return resources
- Maze of Ith's attacker

The shared scenario tests in `scenario/authoring_actions.rs` also use these resources, and they
keep those cases exercised on the Base profile. Execution coverage is not semantic correctness
or manual GUI acceptance.

## Integrity

`integrity.rs` keeps the old command-attempt fixture as a separate safety sweep. Cases it
actually completes must also complete under the strict fixtures.

Zone assertions count physical cards, including parked stack spells. They don't count:
- tokens
- spell copies (CR 707.10), which have no backing object
- continuation snapshots of cards that already moved
- a parked spell that is still on the stack

This keeps the old useful invariant without keeping its old claim.

No production rules, protocol, relay, Qt, freeform or card-data behavior changes here. Full
completion verification is `scripts/verify.ps1 -Side Rust`. Manual UI checks are N/A.
