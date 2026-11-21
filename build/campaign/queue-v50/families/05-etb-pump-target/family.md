# 05-etb-pump-target

Shape: Enters trigger with one fixed pump (and, for one card, indestructible) on a creature you control.

## Source and identity

| Card | Oracle ID | Cost | Type | Body | Oracle text |
|---|---|---|---|---|---|
| Haazda Officer | `35023c04-ae62-47bf-80a0-faa2d7a67788` | {2}{W} | Creature — Human Soldier | 3/2 | When this creature enters, target creature you control gets +1/+1 until end of turn. |
| Herald of the Fair | `6bb7d985-b033-46a2-9bbb-6f7ee62aaaff` | {2}{W} | Creature — Human | 3/2 | When this creature enters, target creature you control gets +1/+1 until end of turn. |
| Rage-Scarred Berserker | `658d54d4-e967-490c-b958-3743b3b1ef45` | {4}{B} | Creature — Minotaur Berserker | 5/4 | When this creature enters, target creature you control gets +1/+0 and gains indestructible until end of turn. (Damage and effects that say "destroy" don't destroy it.) |

Exact records and pinned rulings: `source/<oracle-id>.json` and `rulings/<oracle-id>.jsonl`.

## Reuse

`guardian_of_pilgrims` / `rubblebelt_boar` for entry pump, and `etb_target_pump_batch` for expiry and illegal-target coverage.

## Evidence

- Intended red: `build/verification-logs/` `q50 intended red 05-etb-pump-target` (two canary tests, exit 101 with the family definitions held out of the registry).
- Focused green: `build/verification-logs/20261120-163633-114-q50-scenario-green-v4.log` (41/41) and `build/verification-logs/20261120-163633-113-q50-registry-green-v4.log` (10/10), both exit 0.
- Review maps: `build/verification-logs/card-evidence-6e6ed57a-3ed0-4a08-9a24-329fdc3d6dd3` (34-map Preparation check, exit 0).
- Fingerprints and checklist: `build/verification-logs/card-data-1cede215-79bc-4497-8218-87a48af05507` (metadata Refresh and checklist validation exit 0).
- Scenario module: `tricerules/tricerules-core/tests/scenario/q50_cohorts_batch.rs`.
- Registry module: `tricerules/tricerules-cards/tests/q50_cohorts_batch.rs`.

## Assertions

- `q50_cohorts_batch::q50_etb_pump_canary_simplest_haazda_officer_pumps`
- `q50_cohorts_batch::q50_etb_pump_canary_hardest_rage_scarred_berserker_grants_indestructible`
- `q50_cohorts_batch::q50_etb_pump_cards_target_only_your_creatures_and_expire`
- `q50_cohorts_batch::q50_etb_triggered_cards_fizzle_stale_targets`
- `mechanic_registry::q50_cohorts_batch::q50_etb_pump_maps_definitions`
- `mechanic_registry::q50_cohorts_batch::q50_fingerprints_match_the_pinned_oracle_text`

## Delivery

- Independent read-only review: approved in hash-verified `build/campaign/queue-v50/freeze/batch-b.json` (initial review plus delta reviews; no remaining blocking findings).
- Batch gate: consolidated queue-v50 full gate `verify-03010474-87bb-4f4c-a841-cc732863c003` passed exit 0 (`Side Rust`, `-CardData`) covering `01-put-on-top`..`09-destroy-artifact-enchant`.
- Family commit `933a9f526`; shared evidence commit `0c2a0ed5e`.
- Acceptance: rules-data only; no GUI, networking, privacy or physical-identity surface.
