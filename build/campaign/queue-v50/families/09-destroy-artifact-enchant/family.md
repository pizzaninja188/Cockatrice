# 09-destroy-artifact-enchant

Shape: Single destroy spell with a fixed artifact/enchantment (and one flying-creature) target union.

## Source and identity

| Card | Oracle ID | Cost | Type | Body | Oracle text |
|---|---|---|---|---|---|
| Naturalize | `bdb3ca68-ec1f-4e16-81cc-d23f8f52c728` | {1}{G} | Instant | — | Destroy target artifact or enchantment. |
| Nature's Chant | `46e4ace2-e8bf-4b72-81de-286215886a10` | {1}{G/W} | Instant | — | Destroy target artifact or enchantment. |
| Return to the Earth | `722f2965-a1ec-4b97-a4bd-b99364f58b1e` | {3}{G} | Instant | — | Destroy target artifact, enchantment, or creature with flying. |

Exact records and pinned rulings: `source/<oracle-id>.json` and `rulings/<oracle-id>.jsonl`.

## Reuse

`broken_wings` for the three-branch target union and `destroy_artifact_batch` for destruction, ownership and stale-target coverage.

## Evidence

- Intended red: `build/verification-logs/` `q50 intended red 09-destroy-artifact-enchant` (two canary tests, exit 101 with the family definitions held out of the registry).
- Focused green: `build/verification-logs/20261120-163633-114-q50-scenario-green-v4.log` (41/41) and `build/verification-logs/20261120-163633-113-q50-registry-green-v4.log` (10/10), both exit 0.
- Review maps: `build/verification-logs/card-evidence-6e6ed57a-3ed0-4a08-9a24-329fdc3d6dd3` (34-map Preparation check, exit 0).
- Fingerprints and checklist: `build/verification-logs/card-data-1cede215-79bc-4497-8218-87a48af05507` (metadata Refresh and checklist validation exit 0).
- Scenario module: `tricerules/tricerules-core/tests/scenario/q50_cohorts_batch.rs`.
- Registry module: `tricerules/tricerules-cards/tests/q50_cohorts_batch.rs`.

## Assertions

- `q50_cohorts_batch::q50_destroy_canary_simplest_naturalize_destroys_an_artifact`
- `q50_cohorts_batch::q50_destroy_canary_hardest_return_to_the_earth_destroys_a_flying_creature`
- `q50_cohorts_batch::q50_destroy_cards_accept_only_their_printed_target_types`
- `q50_cohorts_batch::q50_destroy_cards_fizzle_when_the_target_already_left`
- `mechanic_registry::q50_cohorts_batch::q50_destroy_maps_definitions`
- `mechanic_registry::q50_cohorts_batch::q50_fingerprints_match_the_pinned_oracle_text`

## Delivery

- Independent read-only review: pending.
- Batch gate: pending.
- Acceptance: rules-data only; no GUI, networking, privacy or physical-identity surface.
