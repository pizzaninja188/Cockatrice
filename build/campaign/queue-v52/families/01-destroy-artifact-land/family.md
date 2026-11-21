# 01-destroy-artifact-land

Shape: One-shot sorcery destroying exactly one artifact or land, with only a supported rider (regeneration prevention, controller life loss, or cycling).

## Source and identity

| Card | Oracle ID | Cost | Type | Oracle text |
|---|---|---|---|---|
| Demolish | `08c6d2fd-d9ae-4f95-bb9d-7e16b1039814` | {3}{R} | Sorcery | Destroy target artifact or land. |
| Sunder from Within | `09f75f28-09b9-4a6e-a1b0-671208d274d3` | {2}{R}{R} | Sorcery — Arcane | Destroy target artifact or land. |
| Pillage | `0b137853-7cb9-424b-8285-12938991eafb` | {1}{R}{R} | Sorcery | Destroy target artifact or land. It can't be regenerated. |
| Victorious Destruction | `fa5d6c02-53b2-4144-87cb-e223b529ba7d` | {4}{R} | Sorcery | Destroy target artifact or land. Its controller loses 1 life. |
| Violent Impact | `d48d9e93-72a2-48f2-8164-8032615ee217` | {3}{R} | Sorcery | Destroy target artifact or land. Cycling {2} ({2}, Discard this card: Draw a card.) |

Exact records and pinned rulings: `source/<oracle-id>.json` and `rulings/<oracle-id>.jsonl`.

## Reuse

`naturalize` / `return_to_the_earth` for the `Destroy(subject: Chosen(any_of ...))` target union; `pongify`/`flesh_to_dust` for `DestroyPreventingRegeneration`; `inevitable_defeat` for `LoseLife(who: ControllerOfTargetGroup)`; `barkhide_mauler` for hand-zone cycling.

## Evidence

- Historical red: `build/verification-logs/20261121-q52-intended-red.log` (6/6 tests exit 101 with `MissingCard` while the five definitions are held out).
- Focused green: `build/verification-logs/20261121-q52-scenario-green.log` (6/6) and `build/verification-logs/20261121-q52-registry-green.log` (2/2), both exit 0.
- Review maps: `build/campaign/queue-v52/map-list.json`.
- Scenario module: `tricerules/tricerules-core/tests/scenario/q52_cohorts_batch.rs`.
- Registry module: `tricerules/tricerules-cards/tests/q52_cohorts_batch.rs`.

## Assertions

- `q52_cohorts_batch::q52_canary_simplest_demolish_destroys_an_artifact`
- `q52_cohorts_batch::q52_canary_hardest_victorious_destruction_destroys_a_land_and_drains`
- `q52_cohorts_batch::q52_cards_accept_each_branch_and_reject_other_types`
- `q52_cohorts_batch::q52_cards_fizzle_a_departed_target`
- `q52_cohorts_batch::q52_pillage_prevents_regeneration`
- `q52_cohorts_batch::q52_violent_impact_cycles_for_two_and_draws`
- `mechanic_registry::q52_cohorts_batch::q52_maps_definitions_and_target_unions`
- `mechanic_registry::q52_cohorts_batch::q52_fingerprints_match_the_pinned_oracle_text`

## Delivery

- Independent read-only review: approved in hash-verified `build/campaign/queue-v52/freeze/batch-d.json` after initial and delta reviews; no remaining material findings.
- Batch gate: `verify-06a92503-bedd-4c7a-bb41-b45c68878ea2` passed exit 0 (`Side Rust`, `-CardData`) covering `01-destroy-artifact-land`.
- Family commit `defdacc6b`; remote `fde229a9b`.
- Acceptance: rules-data only; no GUI, networking, privacy or physical-identity surface.
