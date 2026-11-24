# 01-gain-life-drain — Triggered drain on a life-gain event

- Queue: `build/campaign/queue-v79` (expanded after queue-v78 was delivered).
- Cards: 3.
- Band: multi-clause composition (3–5).
- Status: **verified**; independent review APPROVED; batch gate pass.
- Pinned bulk: SHA-256 `a9a2fd3b550cc5cff4cec9bbc82aaf5fb8e3f781b4b95a896a0d94c15dce7853`.
- Pinned rulings: SHA-256 `3ee07dbf91347f7940b4067ab1c01e0d5097da8f6c27ef620c202ed4262aaf1f`.

## Typed shape and analogue

Every card has one `WheneverPlayerGainsLife()` trigger (default `player: Controller`) whose effect is
`LoseLife(amount: Fixed(1), who: EachOpponent)`. Cliffhaven Vampire adds the printed flying keyword.
Dina, Soul Steeper adds one activated ability: `Mana("{1}")` plus
`SacrificePermanent(filter: (kind: Creature, controller: You, excluded_objects: [Source]))`, then a
source `PumpTarget` whose scale is
`CardResultCharacteristicSum(source: Payment, action: Sacrifice, players: All,
card_type: Creature, characteristic: Power)` (the payment cohort is the exact sacrificed object,
so `All` avoids the owner-vs-controller mismatch for a borrowed creature).

Closest shipped support: Marauding Blight-Priest and Pactdoll Terror use the same trigger/drain
pair; Bloodthirsty Aerialist combines a keyword with the same trigger; Greater Good is the shipped
flat-effect consumer of a sacrificed creature's last-known power.

Differences from the analogues: Cliffhaven's flying is a printed characteristic; Dina's pump is one
additional activated ability with a source-only subject. No new primitive is introduced.

## Cards

| Card | Oracle ID | Cost | Type | Oracle text | RON | Map |
|---|---|---|---|---|---|---|
| Epicure of Blood | `bdb319a6-a1cd-44f5-999d-af1bb55b2fc4` | `{4}{B}` | Creature — Vampire | Whenever you gain life, each opponent loses 1 life. | `tricerules/tricerules-cards/data/epicure_of_blood.ron` | `authoring/review-maps/epicure_of_blood.json` |
| Cliffhaven Vampire | `1915a311-209b-4628-8122-af3055a78fed` | `{2}{W}{B}` | Creature — Vampire Warrior Ally | Flying; whenever you gain life, each opponent loses 1 life. | `.../cliffhaven_vampire.ron` | `.../cliffhaven_vampire.json` |
| Dina, Soul Steeper | `2121bcb4-f5fe-447f-9112-c7e5b5c45baf` | `{B}{G}` | Legendary Creature — Dryad Druid | Whenever you gain life, each opponent loses 1 life. `{1}, Sacrifice another creature:` Dina gets +X/+0 until end of turn, where X is the sacrificed creature's power. | `.../dina_soul_steeper.ron` (id `dina,_soul_steeper`) | `.../dina_soul_steeper.json` |

## Rulings

Epicure's 2019-07-12 rulings and Cliffhaven's 2016-01-22 rulings confirm one trigger per
life-gaining event regardless of the amount, a fixed 1-life loss per opponent, and that a single
lifelink source dealing damage to several recipients is still one event. Dina's 2021-04-16 rulings
confirm the same event count, the fixed 1-life loss, and that X is the sacrificed creature's power
while it was on the battlefield, not its power in the graveyard. Two-Headed Giant teammate life gain
does not trigger these cards; the engine's two-player scenario does not model teams, and the
event-time player scope is exercised through the default controller filter.

## Scenario evidence

Module: `tricerules-core/tests/scenario/q79_gain_life_drain.rs` (registered in `tests/scenario.rs`).

- `q79_gain_life_drain_canary_simplest_epicure_drains_once_for_one_event`
- `q79_gain_life_drain_canary_hardest_dina_sacrifices_another_creature_and_pumps`
- `q79_gain_life_drain_cards_all_drain_each_opponent_once_per_event`
- `q79_gain_life_drain_two_events_trigger_twice`
- `q79_gain_life_drain_dina_rejects_sacrificing_itself`
- `q79_gain_life_drain_dina_keeps_last_known_power_after_the_sacrifice`
- `q79_gain_life_drain_dina_reads_a_controlled_but_not_owned_sacrifice`
- `q79_gain_life_drain_does_not_trigger_on_an_opponent_gaining_life`
- `mechanic_registry::q79_gain_life_drain::q79_gain_life_drain_map_definitions`
- `mechanic_registry::q79_gain_life_drain::q79_gain_life_drain_fingerprints_match_the_pinned_oracle_text`

The canaries assert the controller gains exactly the printed 7 life, each opponent loses exactly 1
per event, the source remains, and Dina's pump reads the sacrificed 3-power creature. The all-cards
row checks the same event for each printed card. The two-event row proves the second, separate
life-gain event drains again. The rejection row proves Dina cannot sacrifice herself and the
rejected activation spends no mana. The foreign-control row proves the payment cohort reads the exact
sacrificed object even when its owner is the opponent. The last two rows prove only Dina is pumped
and the bystander keeps its printed power, and that an opponent's life gain does not trigger the
default controller-scoped watcher.

## Presentation

Epicure and Cliffhaven map their keyword and trigger lines; Dina maps its trigger and activated
lines. The metadata refresh added exactly 10 fingerprint rows across the batch (3 in this family)
and flipped the corresponding `CARDS.md` entries; no unrelated fingerprint drift was produced.
`presentation-audit --check` and the review-map batch report 0 unresolved or invalid nodes.

## Open items

Independent review: initial bundle `q79-batch-a` patch `6d8e293c…` (changes required: Dina
sacrifice-payment owner scope and missing evidence); corrected bundle `q79-batch-a-v2` patch
`57c1d846…` (approved for semantics); evidence bundle `q79-batch-a-v3` patch `f88d3795…`
(approved, v2 carries forward). Full batch gate
`verify-d9f48957-0477-404b-9cba-4820d02bbb65` exit 0 on the final product bytes; see `status.tsv`
for the product and remote commits.
