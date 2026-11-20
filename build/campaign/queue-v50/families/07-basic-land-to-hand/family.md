# 07-basic-land-to-hand

Shape: Enters trigger with an optional private search for a basic land or printed land subtype into hand.

## Source and identity

| Card | Oracle ID | Cost | Type | Body | Oracle text |
|---|---|---|---|---|---|
| Gatecreeper Vine | `0f539127-535f-4e0d-abaa-e884521098d2` | {1}{G} | Creature — Plant | 0/2 | Defender When this creature enters, you may search your library for a basic land card or a Gate card, reveal it, put it into your hand, then shuffle. |
| District Guide | `1eb4c001-1115-4b99-bbbf-703eb551a381` | {2}{G} | Creature — Elf Scout | 2/2 | When this creature enters, you may search your library for a basic land card or Gate card, reveal it, put it into your hand, then shuffle. |
| Farfinder | `7d306de8-a93b-429b-9436-0501be97dc8d` | {3} | Creature — Fox | 1/1 | Vigilance When this creature enters, you may search your library for a basic land card, reveal it, put it into your hand, then shuffle. |
| Pilgrim's Eye | `66155010-cc2c-449a-85d5-92c95aade514` | {3} | Artifact Creature — Thopter | 1/1 | Flying When this creature enters, you may search your library for a basic land card, reveal it, put it into your hand, then shuffle. |

Exact records and pinned rulings: `source/<oracle-id>.json` and `rulings/<oracle-id>.jsonl`.

## Reuse

`sylvan_ranger` / `borderland_ranger` for the optional private `Choose` + `SearchLibrary` program.

## Evidence

- Intended red: `build/verification-logs/` `q50 intended red 07-basic-land-to-hand` (two canary tests, exit 101 with the family definitions held out of the registry).
- Focused green: `build/verification-logs/20261120-163633-114-q50-scenario-green-v4.log` (41/41) and `build/verification-logs/20261120-163633-113-q50-registry-green-v4.log` (10/10), both exit 0.
- Review maps: `build/verification-logs/card-evidence-6e6ed57a-3ed0-4a08-9a24-329fdc3d6dd3` (34-map Preparation check, exit 0).
- Fingerprints and checklist: `build/verification-logs/card-data-1cede215-79bc-4497-8218-87a48af05507` (metadata Refresh and checklist validation exit 0).
- Scenario module: `tricerules/tricerules-core/tests/scenario/q50_cohorts_batch.rs`.
- Registry module: `tricerules/tricerules-cards/tests/q50_cohorts_batch.rs`.

## Assertions

- `q50_cohorts_batch::q50_basic_land_search_canary_simplest_farfinder_finds_a_basic_land`
- `q50_cohorts_batch::q50_basic_land_search_canary_hardest_district_guide_finds_a_gate`
- `q50_cohorts_batch::q50_basic_land_search_cards_offer_only_their_printed_filters`
- `q50_cohorts_batch::q50_basic_land_search_cards_may_decline_the_search`
- `mechanic_registry::q50_cohorts_batch::q50_basic_land_maps_definitions`
- `mechanic_registry::q50_cohorts_batch::q50_fingerprints_match_the_pinned_oracle_text`

## Delivery

- Independent read-only review: pending.
- Batch gate: pending.
- Acceptance: rules-data only; no GUI, networking, privacy or physical-identity surface.
