# 01-put-on-top

Shape: Single placement spell: put one matching permanent on top of its owner's library.

## Source and identity

| Card | Oracle ID | Cost | Type | Body | Oracle text |
|---|---|---|---|---|---|
| Repel | `6e072b3a-7a83-4d77-bae3-72de0e646b81` | {3}{U} | Instant | — | Put target creature on top of its owner's library. |
| Grasp of Phantoms | `0d96e4bc-ff4e-4f37-89b6-8a4e3c1069a0` | {3}{U} | Sorcery | — | Put target creature on top of its owner's library. Flashback {7}{U} (You may cast this card from your graveyard for its flashback cost. Then exile it.) |
| Run Aground | `6ef9bf18-6b09-4398-a502-6867d999ab0c` | {3}{U} | Instant | — | Put target artifact or creature on top of its owner's library. |
| Banishment Decree | `567b676b-54f3-44c7-8654-4d2b0262ea12` | {3}{W}{W} | Instant | — | Put target artifact, creature, or enchantment on top of its owner's library. |
| Temporal Eddy | `3a35e772-2547-41a3-a2a2-125b48c23aa5` | {2}{U}{U} | Sorcery | — | Put target creature or land on top of its owner's library. |
| Fallow Earth | `05a4acfd-3bf3-4ec2-a28a-bc43ba7c6fbf` | {2}{G} | Sorcery | — | Put target land on top of its owner's library. |

Exact records and pinned rulings: `source/<oracle-id>.json` and `rulings/<oracle-id>.jsonl`.

## Reuse

`time_ebb`, `griptide`, `excommunicate` (same `PutInOwnersLibrary(placement: Top)` program); `damage_flashback_batch` for the flashback alternative cost.

## Evidence

- Intended red: `build/verification-logs/` `q50 intended red 01-put-on-top` (two canary tests, exit 101 with the family definitions held out of the registry).
- Focused green: `build/verification-logs/20261120-163633-114-q50-scenario-green-v4.log` (41/41) and `build/verification-logs/20261120-163633-113-q50-registry-green-v4.log` (10/10), both exit 0.
- Review maps: `build/verification-logs/card-evidence-6e6ed57a-3ed0-4a08-9a24-329fdc3d6dd3` (34-map Preparation check, exit 0).
- Fingerprints and checklist: `build/verification-logs/card-data-1cede215-79bc-4497-8218-87a48af05507` (metadata Refresh and checklist validation exit 0).
- Scenario module: `tricerules/tricerules-core/tests/scenario/q50_cohorts_batch.rs`.
- Registry module: `tricerules/tricerules-cards/tests/q50_cohorts_batch.rs`.

## Assertions

- `q50_cohorts_batch::q50_put_top_canary_simplest_repel_puts_the_target_on_top`
- `q50_cohorts_batch::q50_put_top_canary_hardest_banishment_decree_accepts_three_card_types`
- `q50_cohorts_batch::q50_put_top_cards_put_only_their_matching_permanents_on_top`
- `q50_cohorts_batch::q50_put_top_cards_accept_each_disjunctive_branch`
- `q50_cohorts_batch::q50_put_top_cards_fizzle_when_the_target_leaves`
- `q50_cohorts_batch::q50_put_top_grasp_of_phantoms_flashes_back_and_exiles`
- `mechanic_registry::q50_cohorts_batch::q50_put_top_maps_definitions`
- `mechanic_registry::q50_cohorts_batch::q50_fingerprints_match_the_pinned_oracle_text`

## Delivery

- Independent read-only review: approved in hash-verified `build/campaign/queue-v50/freeze/batch-a.json` (initial review plus delta reviews; no remaining blocking findings).
- Batch gate: consolidated queue-v50 full gate `verify-03010474-87bb-4f4c-a841-cc732863c003` passed exit 0 (`Side Rust`, `-CardData`) covering `01-put-on-top`..`09-destroy-artifact-enchant`.
- Family commit `8a1a0054c`; shared evidence commit `0c2a0ed5e`.
- Acceptance: rules-data only; no GUI, networking, privacy or physical-identity surface.
