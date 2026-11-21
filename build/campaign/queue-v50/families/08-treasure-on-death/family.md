# 08-treasure-on-death

Shape: Dies or enters-and-dies trigger creating exactly one Treasure token.

## Source and identity

| Card | Oracle ID | Cost | Type | Body | Oracle text |
|---|---|---|---|---|---|
| Dire Fleet Hoarder | `7d48ad95-d428-4e2d-b697-56907c3ce579` | {1}{B} | Creature — Human Pirate | 2/1 | When this creature dies, create a Treasure token. (It's an artifact with "{T}, Sacrifice this token: Add one mana of any color.") |
| Jewel-Eyed Cobra | `bbfee53c-30a6-4ed0-beef-5c254be75fc5` | {2}{G} | Creature — Snake | 3/1 | Deathtouch When this creature dies, create a Treasure token. (It's an artifact with "{T}, Sacrifice this token: Add one mana of any color.") |
| Patron of the Arts | `47f8df80-abc9-4d46-8078-8fbc23430259` | {2}{R} | Creature — Dragon Noble | 3/1 | When this creature enters or dies, create a Treasure token. (It's an artifact with "{T}, Sacrifice this token: Add one mana of any color.") |

Exact records and pinned rulings: `source/<oracle-id>.json` and `rulings/<oracle-id>.jsonl`.

## Reuse

`piggy_bank` for the dies Treasure trigger and `brazen_freebooter_batch` for the canonical `treasure` token; `ichor_wellspring` for the combined entry/dies trigger.

## Evidence

- Intended red: `build/verification-logs/` `q50 intended red 08-treasure-on-death` (two canary tests, exit 101 with the family definitions held out of the registry).
- Focused green: `build/verification-logs/20261120-163633-114-q50-scenario-green-v4.log` (41/41) and `build/verification-logs/20261120-163633-113-q50-registry-green-v4.log` (10/10), both exit 0.
- Review maps: `build/verification-logs/card-evidence-6e6ed57a-3ed0-4a08-9a24-329fdc3d6dd3` (34-map Preparation check, exit 0).
- Fingerprints and checklist: `build/verification-logs/card-data-1cede215-79bc-4497-8218-87a48af05507` (metadata Refresh and checklist validation exit 0).
- Scenario module: `tricerules/tricerules-core/tests/scenario/q50_cohorts_batch.rs`.
- Registry module: `tricerules/tricerules-cards/tests/q50_cohorts_batch.rs`.

## Assertions

- `q50_cohorts_batch::q50_treasure_canary_simplest_dire_fleet_hoarder_makes_a_treasure_on_death`
- `q50_cohorts_batch::q50_treasure_canary_hardest_patron_of_the_arts_makes_one_on_entry_and_one_on_death`
- `q50_cohorts_batch::q50_treasure_jewel_eyed_cobra_makes_a_treasure_on_death`
- `q50_cohorts_batch::q50_treasure_cards_credit_the_creatures_controller_and_do_not_trigger_on_exile`
- `mechanic_registry::q50_cohorts_batch::q50_treasure_maps_definitions`
- `mechanic_registry::q50_cohorts_batch::q50_fingerprints_match_the_pinned_oracle_text`

## Delivery

- Independent read-only review: approved in hash-verified `build/campaign/queue-v50/freeze/batch-c.json` (initial review plus delta reviews; no remaining blocking findings).
- Batch gate: consolidated queue-v50 full gate `verify-03010474-87bb-4f4c-a841-cc732863c003` passed exit 0 (`Side Rust`, `-CardData`) covering `01-put-on-top`..`09-destroy-artifact-enchant`.
- Family commit `a1d883392`; shared evidence commit `0c2a0ed5e`.
- Acceptance: rules-data only; no GUI, networking, privacy or physical-identity surface.
