# 04-etb-shrink

Shape: Enters trigger with one fixed negative P/T modification on an opponent's creature.

## Source and identity

| Card | Oracle ID | Cost | Type | Body | Oracle text |
|---|---|---|---|---|---|
| Eyeblight Assassin | `09cacd93-5c20-4993-9ee2-a756204796f7` | {2}{B} | Creature — Elf Assassin | 2/2 | When this creature enters, target creature an opponent controls gets -1/-1 until end of turn. |
| Black Dragon | `ad804712-18ba-49d7-9af2-4361edaa6cf8` | {5}{B}{B} | Creature — Dragon | 4/4 | Flying Acid Breath — When this creature enters, target creature an opponent controls gets -3/-3 until end of turn. |
| Faerie Duelist | `ccbf7141-69c9-407d-b68e-c1c63ddef213` | {1}{U} | Creature — Faerie Rogue | 1/2 | Flash Flying When this creature enters, target creature an opponent controls gets -2/-0 until end of turn. |
| Brinebarrow Intruder | `eba73313-5ec5-4004-8040-658861b71a8f` | {U} | Creature — Human Rogue | 1/2 | Flash When this creature enters, target creature an opponent controls gets -2/-0 until end of turn. |

Exact records and pinned rulings: `source/<oracle-id>.json` and `rulings/<oracle-id>.jsonl`.

## Reuse

`blister_beetle`, `keening_banshee`, `brinebarrow_intruder`-family tests (same `WhenSelfEntersBattlefield` + negative `PumpTarget`).

## Evidence

- Intended red: `build/verification-logs/` `q50 intended red 04-etb-shrink` (two canary tests, exit 101 with the family definitions held out of the registry).
- Focused green: `build/verification-logs/20261120-163633-114-q50-scenario-green-v4.log` (41/41) and `build/verification-logs/20261120-163633-113-q50-registry-green-v4.log` (10/10), both exit 0.
- Review maps: `build/verification-logs/card-evidence-6e6ed57a-3ed0-4a08-9a24-329fdc3d6dd3` (34-map Preparation check, exit 0).
- Fingerprints and checklist: `build/verification-logs/card-data-1cede215-79bc-4497-8218-87a48af05507` (metadata Refresh and checklist validation exit 0).
- Scenario module: `tricerules/tricerules-core/tests/scenario/q50_cohorts_batch.rs`.
- Registry module: `tricerules/tricerules-cards/tests/q50_cohorts_batch.rs`.

## Assertions

- `q50_cohorts_batch::q50_etb_shrink_canary_simplest_eyeblight_assassin_shrinks`
- `q50_cohorts_batch::q50_etb_shrink_canary_hardest_black_dragon_flying_shrinks_three`
- `q50_cohorts_batch::q50_etb_shrink_cards_target_only_opposing_creatures_and_expire`
- `q50_cohorts_batch::q50_etb_triggered_cards_fizzle_stale_targets`
- `mechanic_registry::q50_cohorts_batch::q50_etb_shrink_maps_definitions`
- `mechanic_registry::q50_cohorts_batch::q50_fingerprints_match_the_pinned_oracle_text`

## Delivery

- Independent read-only review: approved in hash-verified `build/campaign/queue-v50/freeze/batch-b.json` (initial review plus delta reviews; no remaining blocking findings).
- Batch gate: consolidated queue-v50 full gate `verify-03010474-87bb-4f4c-a841-cc732863c003` passed exit 0 (`Side Rust`, `-CardData`) covering `01-put-on-top`..`09-destroy-artifact-enchant`.
- Family commit `d8dca4c9b`; shared evidence commit `0c2a0ed5e`.
- Acceptance: rules-data only; no GUI, networking, privacy or physical-identity surface.
