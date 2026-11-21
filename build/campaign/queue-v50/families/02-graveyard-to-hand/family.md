# 02-graveyard-to-hand

Shape: Single spell effect: return one filtered card from your graveyard to your hand.

## Source and identity

| Card | Oracle ID | Cost | Type | Body | Oracle text |
|---|---|---|---|---|---|
| Boggart Birth Rite | `030e9e3e-ce78-4c3b-bd83-969c07c70459` | {B} | Kindred Sorcery — Goblin | — | Return target Goblin card from your graveyard to your hand. |
| Argivian Find | `0b6c74a7-d7d3-4c05-84b1-bcb580bb476f` | {W} | Instant | — | Return target artifact or enchantment card from your graveyard to your hand. |
| Nature's Spiral | `565922d6-8152-411f-8d4d-dc7c8523a49b` | {1}{G} | Sorcery | — | Return target permanent card from your graveyard to your hand. |
| Déjà Vu | `7408b9c5-7266-4627-be4e-b691cf5c622c` | {2}{U} | Sorcery | — | Return target sorcery card from your graveyard to your hand. |

Exact records and pinned rulings: `source/<oracle-id>.json` and `rulings/<oracle-id>.jsonl`.

## Reuse

`sages_knowledge`, `recollect`, `disentomb` (same `MoveGraveyardCards(destination: Hand)` shape) plus `badlands_revival` for the permanent-card disjunction.

## Evidence

- Intended red: `build/verification-logs/` `q50 intended red 02-graveyard-to-hand` (two canary tests, exit 101 with the family definitions held out of the registry).
- Focused green: `build/verification-logs/20261120-163633-114-q50-scenario-green-v4.log` (41/41) and `build/verification-logs/20261120-163633-113-q50-registry-green-v4.log` (10/10), both exit 0.
- Review maps: `build/verification-logs/card-evidence-6e6ed57a-3ed0-4a08-9a24-329fdc3d6dd3` (34-map Preparation check, exit 0).
- Fingerprints and checklist: `build/verification-logs/card-data-1cede215-79bc-4497-8218-87a48af05507` (metadata Refresh and checklist validation exit 0).
- Scenario module: `tricerules/tricerules-core/tests/scenario/q50_cohorts_batch.rs`.
- Registry module: `tricerules/tricerules-cards/tests/q50_cohorts_batch.rs`.

## Assertions

- `q50_cohorts_batch::q50_graveyard_to_hand_canary_simplest_boggart_birth_rite_returns_a_goblin`
- `q50_cohorts_batch::q50_graveyard_to_hand_canary_hardest_natures_spiral_returns_a_permanent_card`
- `q50_cohorts_batch::q50_graveyard_to_hand_cards_return_only_their_filtered_cards`
- `q50_cohorts_batch::q50_graveyard_to_hand_cards_reject_nonmatching_cards`
- `q50_cohorts_batch::q50_graveyard_to_hand_boggart_returns_a_noncreature_goblin_card`
- `q50_cohorts_batch::q50_graveyard_to_hand_cards_reject_opponent_graveyards_and_fizzle_stale_targets`
- `mechanic_registry::q50_cohorts_batch::q50_graveyard_to_hand_maps_definitions`
- `mechanic_registry::q50_cohorts_batch::q50_fingerprints_match_the_pinned_oracle_text`

## Delivery

- Independent read-only review: approved in hash-verified `build/campaign/queue-v50/freeze/batch-a.json` (initial review plus delta reviews; no remaining blocking findings).
- Batch gate: consolidated queue-v50 full gate `verify-03010474-87bb-4f4c-a841-cc732863c003` passed exit 0 (`Side Rust`, `-CardData`) covering `01-put-on-top`..`09-destroy-artifact-enchant`.
- Family commit `db143f0a0`; shared evidence commit `0c2a0ed5e`.
- Acceptance: rules-data only; no GUI, networking, privacy or physical-identity surface.
