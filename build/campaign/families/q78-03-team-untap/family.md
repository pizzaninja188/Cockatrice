# 03-team-untap — Team untap with printed pump or draw riders

- Queue: `build/campaign/queue-v78` (expanded after queue-v77 was delivered).
- Cards: 4.
- Band: multi-clause composition (untap plus optional pump/draw).
- Status: **verified**; independent review APPROVED; batch gate pass.
- Pinned bulk: SHA-256 `a9a2fd3b550cc5cff4cec9bbc82aaf5fb8e3f781b4b95a896a0d94c15dce7853`.
- Pinned rulings: SHA-256 `3ee07dbf91347f7940b4067ab1c01e0d5097da8f6c27ef620c202ed4262aaf1f`.

## Typed shape and analogue

`UntapAll(players: Controller)` over the default creature filter, optionally preceded by
`PumpAll(filter: (controller: YouControl), ...)` and optionally followed by `Draw(1)`. Closest shipped
support: Vitalize (`UntapAll`), Rally the Forces (`PumpAll` over a controller scope), Zealous Display
(pump plus conditional untap), and ordinary `Draw`.

Differences: Mobilize is a sorcery with only the untap instruction; To Arms! adds a draw; Rallying
Roar and Rally to Battle pump and untap in one resolution, with the pump applying to already-untapped
creatures as the pinned rulings require.

## Cards

| Card | Oracle ID | Cost | Type | Oracle text | RON | Map |
|---|---|---|---|---|---|---|
| Mobilize | `7416c8db-ec33-4d92-866c-b7fdc69c8b41` | `{G}` | Sorcery | Untap all creatures you control. | `.../mobilize.ron` | `authoring/review-maps/mobilize.json` |
| To Arms! | `90c97fc4-597c-45cf-96c6-3ba9558ede0d` | `{1}{W}` | Instant | Untap all creatures you control. Draw a card. | `.../to_arms!.ron` | `.../to_arms!.json` |
| Rallying Roar | `40ee79dd-cf53-415c-8e3c-d1a125a735db` | `{2}{W}` | Instant | Creatures you control get +1/+1 until end of turn. Untap them. | `.../rallying_roar.ron` | `.../rallying_roar.json` |
| Rally to Battle | `9d8d88dd-c266-4b7d-a756-ac2d8976ca89` | `{3}{W}` | Instant | Creatures you control get +1/+3 until end of turn. Untap them. | `.../rally_to_battle.ron` | `.../rally_to_battle.json` |

## Rulings

- Rallying Roar, pinned 2017-09-29: untapped creatures you control still get +1/+1, and creatures you
  begin to control later in the turn do not get the pump. The same CR 611.2c/613.4c behavior applies
  to Rally to Battle.
- Rally to Battle, pinned 2019-01-25: already-untapped creatures still get +1/+3.
- Mobilize and To Arms! have no card-specific rulings. CR 701.26 applies the untap to each creature
  controlled at resolution; Mobilize keeps its printed sorcery timing.

## Scenario evidence

Module: `tricerules-core/tests/scenario/q78_team_untap.rs`.

- `q78_team_untap_canary_simplest_mobilize_untaps_only_your_creatures`
- `q78_team_untap_canary_hardest_rallying_roar_pumps_and_untaps_even_untapped_creatures`
- `q78_team_untap_cards_apply_only_their_printed_riders`
- `q78_team_untap_cards_do_not_affect_creatures_that_enter_later`
- `q78_team_untap_mobilize_keeps_sorcery_timing`
- `q78_team_untap_pump_does_not_apply_to_a_creature_controlled_later`
- `mechanic_registry::q78_team_untap::{...}_map_definitions`
- `mechanic_registry::q78_team_untap::{...}_fingerprints_match_the_pinned_oracle_text`

The scenarios assert exact mana, tapped state for both players' creatures, exact power/toughness
values for every pump value, the exact hand count for To Arms!, the opponent's creature remaining
tapped and unpumped, a creature entering later staying unpumped, a creature controlled later in the turn not being
retroactively pumped, the opponent's tapped creature staying tapped in every case, and Mobilize
being rejected during combat. There are no targets, tokens, counters, hidden choices or delayed effects.

## Presentation

Mobilize and both Rally cards have one normalized Oracle line. To Arms! has two: line 1 maps the
untap and line 2 maps the draw. `presentation-audit --check` reports 0 unresolved or invalid nodes.

## Open items

Independent review: initial bundle `q78-batch-a` (patch `8a13808a…`), corrected bundle
`q78-batch-a-v2` (patch `aac90a85…`) and final bundle `q78-batch-a-v3` (patch `b59f9d8c…`); the
final delta verdict was APPROVED. Full batch gate `verify-691906e1-74ff-48be-a2f1-4edb3e60a191`
exit 0 on the final product bytes; see `status.tsv` for the product and remote commits.
