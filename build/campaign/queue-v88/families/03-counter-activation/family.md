# 03-counter-activation — tied activated +1/+1 counters

- Queue: `build/campaign/queue-v88`.
- Cards: 3.
- Band: multi-clause composition (3).
- Status: **verified**; independent review approved and the queue-v88 batch gate passed
  exit 0.
- Pinned bulk: SHA-256 `1cee09ee1ac70977c46171f4abf5cdfe170ab43bc32232af9138404e27524ce6`.
- Pinned rulings: SHA-256 `84f4c793e6d4f1256206b915da007111e1a445fe7cad99147802086454e8a95d`.

## Typed shape and analogue

Each card prints either `{3}{G}: Put a +1/+1 counter on this creature.` or
`{4}{W}: Put a +1/+1 counter on this creature.` — one activated ability with a mana cost and `effect: [PutCounters(counter: PlusOnePlusOne, count: 1, subject: Source)]`, shipped
on Toadstool Admirer (`data/generated/t/toadstool_admirer.ron`, reviewed under
`issue_343_generated_eight_clause_batch`).

The family varies the printed activation cost and one optional keyword, so the exact cost mapping
is what the tests prove rather than a single hard-coded amount. Notably `Unholy Officiant` charges
`{4}{W}`, not `{4}{G}`, so a copy-paste from the green analogue is rejected by the cost check.

## Cards

| Card | Oracle ID | Cost | Type | P/T | Extra | Activation | Line | RON | Map |
|---|---|---|---|---|---|---|---|---|---|
| Jungle Delver | `2f299e69-d82b-48c9-b7f5-cf07cd9dced2` | `{G}` | Creature — Merfolk Warrior | 1/1 | — | `{3}{G}` | 1 | `data/jungle_delver.ron` | `review-maps/jungle_delver.json` |
| Unholy Officiant | `41c06726-0360-4488-8dc3-ec257386a372` | `{W}` | Creature — Vampire Cleric | 1/2 | Vigilance | `{4}{W}` | 2 | `data/unholy_officiant.ron` | `review-maps/unholy_officiant.json` |
| Verdant Automaton | `d3af1c69-42ae-4d29-a9f1-72f862d26303` | `{2}` | Artifact Creature — Construct | 1/2 | — | `{3}{G}` | 1 | `data/verdant_automaton.ron` | `review-maps/verdant_automaton.json` |

## Pinned rulings

None of the three has a pinned ruling. The output is the ordinary counter-placement effect
(CR 122.1/122.6), and the shipped Toadstool Admirer assertions already cover the same typed
operation.

Packets: `build/campaign/queue-v88/families/03-counter-activation/packets/`.

## Canaries

- Simplest: Jungle Delver — one pip, no keyword, so the whole activation is `{3}{G}` and one
  counter.
- Hardest: Unholy Officiant — the largest printed cost in the family and the only white pip plus a
  printed keyword. Its cost is not the green analogue's, so it is the case that catches a
  copy-pasted activation.

## Semantic evidence

- `scenario q88_batch_a::q88_counter_canary_simplest_adds_one_counter`
- `scenario q88_batch_a::q88_counter_canary_hardest_charges_its_exact_cost_and_keeps_its_keyword`
- `scenario q88_batch_a::q88_counter_cards_add_one_counter_each_activation_and_survive_a_failed_one`
- `scenario q88_batch_a::q88_counter_cards_require_their_controller`
- `mechanic_registry::q88_batch_a::q88_counter_cards_map_definitions`
- `mechanic_registry::q88_batch_a::q88_batch_a_fingerprints_match_the_pinned_oracle_text`

## Focused evidence

- Intended red: `build/campaign/queue-v88/logs/red-q88.log` (produced before the timing and cost
  list assertions were hardened).
- Registry green: `build/campaign/queue-v88/logs/green-q88-registry.log` (4 passed, shared).
- Scenario green: `build/campaign/queue-v88/logs/green-q88-scenario.log` (12 passed, shared).
- `cargo fmt --all -- --check` exit 0; `cargo clippy -D warnings` exit 0; `git diff --check`
  exit 0 (`build/campaign/queue-v88/logs/preflight-q88.log`).
- Non-vacuity probes: copying the green `{3}{G}` cost onto Unholy Officiant fails the registry and
  the scenario (there is no green mana in its pool); adding a `Tap` cost to Jungle Delver fails the
  registry cost list, and the canaries assert the source stays untapped.

## Conformance and scope

Pure data plus tests, maps and metadata. No engine change. Affected side: Rust and CardData.

## Final gate

PASS, exit 0. Queue-v88 consolidated batch gate `verify-1175655f-71da-4886-a7bb-f74e3eadc5af`
on the reviewed tree, covering all three families (16 cards): environment, diff check, CardData,
the card-data dependency boundary, all five package format checks, Clippy and the full Rust suite
(including all 16 q88 tests).
Summary: `build/verification-logs/verify-1175655f-71da-4886-a7bb-f74e3eadc5af/summary.json`.

Commit `f7880fa10`; remote `f7880fa10d41238f4922e5e753668a8712124a3a` verified by
`git fetch` and `git rev-parse`.

## Blockers and gaps

No card in this family is blocked. No partial admission.

## MTG applicability

CR 602.1/602.2 (activation and printed cost), CR 107.4a/118.3a (mana payment at activation),
CR 122.1a (a +1/+1 counter modifies the permanent's power and toughness) and
CR 702.20a-b (vigilance on Unholy Officiant). No Comprehensive Rules deviation.
