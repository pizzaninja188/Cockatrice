# 06-etb-counter-another

Shape: Enters trigger with one +1/+1 counter on another typed creature you control.

## Source and identity

| Card | Oracle ID | Cost | Type | Body | Oracle text |
|---|---|---|---|---|---|
| Stromkirk Mentor | `17cbeec4-b180-48e9-9156-42271f918a9d` | {3}{B} | Creature — Vampire Soldier | 4/2 | When this creature enters, put a +1/+1 counter on another target Vampire you control. |
| Sanguine Glorifier | `391f1331-7577-4bf4-a9ed-2f55ded4cbd8` | {3}{W} | Creature — Vampire Cleric | 3/3 | When this creature enters, put a +1/+1 counter on another target Vampire you control. |
| Jade Bearer | `72ddbc9b-f584-433f-9d2d-6e4c374129c4` | {G} | Creature — Merfolk Shaman | 1/1 | When this creature enters, put a +1/+1 counter on another target Merfolk you control. |

Exact records and pinned rulings: `source/<oracle-id>.json` and `rulings/<oracle-id>.jsonl`.

## Reuse

`kujar_seedsculptor` / `q43_etb_counter_target_batch` for entry counter placement; the printed subtype and `another` exclusion are the only differences.

## Evidence

- Intended red: `build/verification-logs/` `q50 intended red 06-etb-counter-another` (two canary tests, exit 101 with the family definitions held out of the registry).
- Focused green: `build/verification-logs/20261120-163633-114-q50-scenario-green-v4.log` (41/41) and `build/verification-logs/20261120-163633-113-q50-registry-green-v4.log` (10/10), both exit 0.
- Review maps: `build/verification-logs/card-evidence-6e6ed57a-3ed0-4a08-9a24-329fdc3d6dd3` (34-map Preparation check, exit 0).
- Fingerprints and checklist: `build/verification-logs/card-data-1cede215-79bc-4497-8218-87a48af05507` (metadata Refresh and checklist validation exit 0).
- Scenario module: `tricerules/tricerules-core/tests/scenario/q50_cohorts_batch.rs`.
- Registry module: `tricerules/tricerules-cards/tests/q50_cohorts_batch.rs`.

## Assertions

- `q50_cohorts_batch::q50_etb_counter_canary_simplest_jade_bearer_adds_a_counter`
- `q50_cohorts_batch::q50_etb_counter_canary_hardest_stromkirk_mentor_excludes_source`
- `q50_cohorts_batch::q50_etb_counter_cards_require_another_typed_creature_you_control`
- `q50_cohorts_batch::q50_etb_triggered_cards_fizzle_stale_targets`
- `mechanic_registry::q50_cohorts_batch::q50_etb_counter_maps_definitions`
- `mechanic_registry::q50_cohorts_batch::q50_fingerprints_match_the_pinned_oracle_text`

## Delivery

- Independent read-only review: pending.
- Batch gate: pending.
- Acceptance: rules-data only; no GUI, networking, privacy or physical-identity surface.
