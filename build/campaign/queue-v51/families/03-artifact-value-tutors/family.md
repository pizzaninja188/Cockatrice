# 03-artifact-value-tutors

Shape: Enters trigger with an optional private artifact-card search bounded to a printed mana-value band.

## Source and identity

| Card | Oracle ID | Cost | Type | Body | Oracle text |
|---|---|---|---|---|---|
| Trinket Mage | `ffc95093-24a2-4616-a44a-24788e8df9c8` | {2}{U} | Creature — Human Wizard | 2/2 | When this creature enters, you may search your library for an artifact card with mana value 1 or less, reveal that card, put it into your hand, then shuffle. |
| Tribute Mage | `c2b51295-8820-425f-a517-55231123c7de` | {2}{U} | Creature — Human Wizard | 2/2 | When this creature enters, you may search your library for an artifact card with mana value 2, reveal that card, put it into your hand, then shuffle. |
| Trophy Mage | `08afc0d7-192f-4ab6-b6a0-c4265cf5e225` | {2}{U} | Creature — Human Wizard | 2/2 | When this creature enters, you may search your library for an artifact card with mana value 3, reveal it, put it into your hand, then shuffle. |
| Transit Mage | `991efbf1-299d-4892-aeab-8ff9827f7168` | {2}{U} | Creature — Human Wizard | 2/2 | When this creature enters, you may search your library for an artifact card with mana value 4 or 5, reveal it, put it into your hand, then shuffle. |

Exact records and pinned rulings: `source/<oracle-id>.json` and `rulings/<oracle-id>.jsonl`.

## Reuse

`goblin_engineer` for the optional `SearchLibrary` trigger and `sylvan_ranger`/`borderland_ranger` for the private search flow.

## Evidence

- Historical red: `build/verification-logs/20261120-q51-intended-red.log` (the original 51-card, 34-test scenario run, including both canaries per family, exited 101 with `MissingCard` while the definitions and 9 token templates were held out of `data/`; family 07 was ejected afterwards).
- Focused green: `build/verification-logs/20261120-q51-scenario-green4.log` (40/40) and `build/verification-logs/20261120-q51-registry-green4.log` (9/9), both exit 0.
- Review maps: `build/campaign/queue-v51/map-list.json` and `build/verification-logs/card-evidence-f69e990e-c0e2-4ee8-8a8d-da4cc83beba4` (Preparation evidence and executable-reference checks).
- Scenario module: `tricerules/tricerules-core/tests/scenario/q51_cohorts_batch.rs`.
- Registry module: `tricerules/tricerules-cards/tests/q51_cohorts_batch.rs`.

## Assertions

- `q51_cohorts_batch::q51_artifact_tutors_canary_simplest_trinket_mage_finds_a_one_drop`
- `q51_cohorts_batch::q51_artifact_tutors_canary_hardest_transit_mage_finds_four_or_five`
- `q51_cohorts_batch::q51_artifact_tutors_offer_only_matching_artifacts`
- `q51_cohorts_batch::q51_artifact_tutors_may_decline_without_shuffling`
- `q51_cohorts_batch::q51_artifact_tutors_reveal_shuffle_and_shuffle_on_failure_to_find`
- `mechanic_registry::q51_cohorts_batch::q51_artifact_value_tutors_map_bounded_searches`

## Delivery

- Independent read-only review: approved in hash-verified `build/campaign/queue-v51/freeze/batch-a.json` after initial and delta reviews; no remaining material findings.
- Batch gate: `verify-8bd9732e-bf56-4343-92ef-cec4c77dcc07` passed exit 0 (`Side Rust`, `-CardData`) covering `01-token-spells..03-artifact-value-tutors`.
- Family commit `c21ed13e5`; remote `4ab4e4c99`.
- Acceptance: rules-data only; no GUI, networking, privacy or physical-identity surface.
