# 06-etb-each-discard

Shape: Enters trigger making each player discard one card; one card also discards to regenerate.

## Source and identity

| Card | Oracle ID | Cost | Type | Body | Oracle text |
|---|---|---|---|---|---|
| Miasmic Mummy | `b5b049d4-242a-48ec-85d1-1ef66c8cfa4c` | {1}{B} | Creature — Zombie Jackal | 2/2 | When this creature enters, each player discards a card. |
| Sibsig Icebreakers | `4880c976-b417-48e4-98a2-9e5a8da0d758` | {2}{B} | Creature — Zombie | 2/3 | When this creature enters, each player discards a card. |
| Earsplitting Rats | `6c14ecbf-0097-4fd8-97a3-a32b532c21e0` | {3}{B} | Creature — Rat | 2/1 | When this creature enters, each player discards a card. Discard a card: Regenerate this creature. |

Exact records and pinned rulings: `source/<oracle-id>.json` and `rulings/<oracle-id>.jsonl`.

## Reuse

`fanatic_of_the_harrowing` for the player-set `Discard(who: EachPlayer)` continuation and `cudgel_troll` for activated regenerate.

## Evidence

- Historical red: `build/verification-logs/20261120-q51-intended-red.log` (the original 51-card, 34-test scenario run, including both canaries per family, exited 101 with `MissingCard` while the definitions and 9 token templates were held out of `data/`; family 07 was ejected afterwards).
- Focused green: `build/verification-logs/20261120-q51-scenario-green4.log` (40/40) and `build/verification-logs/20261120-q51-registry-green4.log` (9/9), both exit 0.
- Review maps: `build/campaign/queue-v51/map-list.json` and `build/verification-logs/card-evidence-f69e990e-c0e2-4ee8-8a8d-da4cc83beba4` (Preparation evidence and executable-reference checks).
- Scenario module: `tricerules/tricerules-core/tests/scenario/q51_cohorts_batch.rs`.
- Registry module: `tricerules/tricerules-cards/tests/q51_cohorts_batch.rs`.

## Assertions

- `q51_cohorts_batch::q51_discard_canary_simplest_miasmic_mummy_makes_each_player_discard`
- `q51_cohorts_batch::q51_discard_canary_hardest_earsplitting_rats_regenerates_after_discarding`
- `q51_cohorts_batch::q51_discard_cards_use_only_their_printed_cost`
- `q51_cohorts_batch::q51_discard_choices_are_private_exact_one_and_owner_graveyarded`
- `q51_cohorts_batch::q51_earsplitting_rats_shield_prevents_destruction`
- `mechanic_registry::q51_cohorts_batch::q51_each_player_discard_maps_the_shared_discard`

## Delivery

- Independent read-only review: approved in hash-verified `build/campaign/queue-v51/freeze/batch-b.json` after initial and delta reviews; no remaining material findings.
- Batch gate: pending.
- Acceptance: rules-data only; no GUI, networking, privacy or physical-identity surface.
