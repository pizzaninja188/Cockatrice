# 03-tap-stun-spells

Shape: Single tap-and-stun spell: tap one creature, put the printed stun counters on it, then draw, scry, or nothing.

## Source and identity

| Card | Oracle ID | Cost | Type | Body | Oracle text |
|---|---|---|---|---|---|
| Cryogenic Stasis | `846ba66c-a4eb-4d00-a0f1-d9f9524b9695` | {1}{U} | Instant | — | Tap target creature and put a stun counter on it. (If a permanent with a stun counter would become untapped, remove one from it instead.) Draw a card. |
| Impede Momentum | `c0239ff1-6ae6-431a-806e-7321d8272a1d` | {1}{U} | Sorcery | — | Tap target creature and put three stun counters on it. (If a permanent with a stun counter would become untapped, remove one from it instead.) Scry 1. (Look at the top card of your library. You may put that card on the bottom.) |
| Tranquilize | `1133633a-b717-42cb-b477-4e18584d876f` | {1}{U} | Sorcery | — | Tap target creature an opponent controls and put three stun counters on it. (If a permanent with a stun counter would become untapped, remove one from it instead.) |
| Freeze in Place | `8bf90bb0-f9c1-49c8-905e-32747e921e92` | {1}{U} | Sorcery | — | Tap target creature an opponent controls and put three stun counters on it. Scry 2. (If a permanent with a stun counter would become untapped, remove one from it instead.) |

Exact records and pinned rulings: `source/<oracle-id>.json` and `rulings/<oracle-id>.jsonl`.

## Reuse

`alchemax_slayer-bots` / `cryogen_relic` for tap plus stun counters, and `crippling_chill` / `aggressive_urge` for a targeted spell with a non-targeted follow-up.

## Evidence

- Intended red: `build/verification-logs/` `q50 intended red 03-tap-stun-spells` (two canary tests, exit 101 with the family definitions held out of the registry).
- Focused green: `build/verification-logs/20261120-163633-114-q50-scenario-green-v4.log` (41/41) and `build/verification-logs/20261120-163633-113-q50-registry-green-v4.log` (10/10), both exit 0.
- Review maps: `build/verification-logs/card-evidence-6e6ed57a-3ed0-4a08-9a24-329fdc3d6dd3` (34-map Preparation check, exit 0).
- Fingerprints and checklist: `build/verification-logs/card-data-1cede215-79bc-4497-8218-87a48af05507` (metadata Refresh and checklist validation exit 0).
- Scenario module: `tricerules/tricerules-core/tests/scenario/q50_cohorts_batch.rs`.
- Registry module: `tricerules/tricerules-cards/tests/q50_cohorts_batch.rs`.

## Assertions

- `q50_cohorts_batch::q50_tap_stun_canary_simplest_cryogenic_stasis_taps_and_draws`
- `q50_cohorts_batch::q50_tap_stun_canary_hardest_freeze_in_place_scries_two`
- `q50_cohorts_batch::q50_tap_stun_cards_tap_and_place_their_printed_counters`
- `q50_cohorts_batch::q50_tap_stun_cards_still_place_counters_on_an_already_tapped_creature`
- `q50_cohorts_batch::q50_tap_stun_cards_replace_untaps_with_counter_removal`
- `q50_cohorts_batch::q50_tap_stun_cards_scope_opponent_only_targets_and_reject_noncreatures`
- `q50_cohorts_batch::q50_tap_stun_freeze_in_place_fizzles_without_scrying_when_the_target_leaves`
- `mechanic_registry::q50_cohorts_batch::q50_tap_stun_maps_definitions`
- `mechanic_registry::q50_cohorts_batch::q50_fingerprints_match_the_pinned_oracle_text`

## Delivery

- Independent read-only review: approved in hash-verified `build/campaign/queue-v50/freeze/batch-a.json` (initial review plus delta reviews; no remaining blocking findings).
- Batch gate: consolidated queue-v50 full gate `verify-03010474-87bb-4f4c-a841-cc732863c003` passed exit 0 (`Side Rust`, `-CardData`) covering `01-put-on-top`..`09-destroy-artifact-enchant`.
- Family commit `d0ebf0420`; shared evidence commit `0c2a0ed5e`.
- Acceptance: rules-data only; no GUI, networking, privacy or physical-identity surface.
