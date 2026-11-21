# 02-splicer-cycle

Shape: Enters trigger creating one 3/3 Golem, plus a Golem-only static lord or activated regenerate.

## Source and identity

| Card | Oracle ID | Cost | Type | Body | Oracle text |
|---|---|---|---|---|---|
| Wing Splicer | `7fbf6afc-8a16-447b-bdef-b7d0b640f76c` | {3}{U} | Creature — Phyrexian Human Artificer | 1/1 | When this creature enters, create a 3/3 colorless Phyrexian Golem artifact creature token. Golem creatures you control have flying. |
| Sensor Splicer | `9676da93-1e17-4f6c-b610-a242b78c71ab` | {4}{W} | Creature — Phyrexian Artificer | 1/1 | When this creature enters, create a 3/3 colorless Phyrexian Golem artifact creature token. Golem creatures you control have vigilance. |
| Master Splicer | `9e0c5919-aed6-4d98-85c8-2658be78bdf0` | {3}{W} | Creature — Phyrexian Human Artificer | 1/1 | When this creature enters, create a 3/3 colorless Phyrexian Golem artifact creature token. Golems you control get +1/+1. |
| Blade Splicer | `194166ad-0179-42a3-86b9-ba7f322ec576` | {2}{W} | Creature — Phyrexian Human Artificer | 1/1 | When this creature enters, create a 3/3 colorless Phyrexian Golem artifact creature token. Golems you control have first strike. |
| Vital Splicer | `e1899133-635d-46ff-b90d-6f8d46a1ffe1` | {3}{G} | Creature — Phyrexian Human Artificer | 1/1 | When this creature enters, create a 3/3 colorless Phyrexian Golem artifact creature token. {1}: Regenerate target Golem you control. |

Exact records and pinned rulings: `source/<oracle-id>.json` and `rulings/<oracle-id>.jsonl`.

## Reuse

`goblin_chieftain` for `AnthemKeyword`, `glorious_anthem` for `AnthemPt`, `cudgel_troll` for activated `Regenerate`, and `raise_the_alarm` for `CreateTokens`.

## Evidence

- Historical red: `build/verification-logs/20261120-q51-intended-red.log` (the original 51-card, 34-test scenario run, including both canaries per family, exited 101 with `MissingCard` while the definitions and 9 token templates were held out of `data/`; family 07 was ejected afterwards).
- Focused green: `build/verification-logs/20261120-q51-scenario-green4.log` (40/40) and `build/verification-logs/20261120-q51-registry-green4.log` (9/9), both exit 0.
- Review maps: `build/campaign/queue-v51/map-list.json` and `build/verification-logs/card-evidence-f69e990e-c0e2-4ee8-8a8d-da4cc83beba4` (Preparation evidence and executable-reference checks).
- Scenario module: `tricerules/tricerules-core/tests/scenario/q51_cohorts_batch.rs`.
- Registry module: `tricerules/tricerules-cards/tests/q51_cohorts_batch.rs`.

## Assertions

- `q51_cohorts_batch::q51_splicers_canary_simplest_wing_splicer_grants_flying`
- `q51_cohorts_batch::q51_splicers_canary_hardest_vital_splicer_regenerates_a_golem`
- `q51_cohorts_batch::q51_splicers_create_one_golem_and_apply_the_printed_lord`
- `q51_cohorts_batch::q51_splicers_do_not_modify_other_creatures`
- `q51_cohorts_batch::q51_splicers_scope_their_lord_to_your_golems`
- `q51_cohorts_batch::q51_vital_splicer_rejects_non_golem_and_opposing_golem_targets`
- `mechanic_registry::q51_cohorts_batch::q51_splicers_map_entry_and_lord_profiles`

## Delivery

- Independent read-only review: approved in hash-verified `build/campaign/queue-v51/freeze/batch-a.json` after initial and delta reviews; no remaining material findings.
- Batch gate: pending.
- Acceptance: rules-data only; no GUI, networking, privacy or physical-identity surface.
