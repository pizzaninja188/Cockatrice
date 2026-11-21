# 03-base-power-toughness

Shape: One-shot instant setting a target creature's base power and toughness to a fixed printed value; Zhalfirin Shapecraft also draws one.

## Source and identity

| Card | Oracle ID | Cost | Type | Oracle text |
|---|---|---|---|---|
| Diminish | `3e554ab4-8179-4d3e-84ba-9a67b51cff7f` | {U} | Instant | Target creature has base power and toughness 1/1 until end of turn. |
| Square Up | `013a465b-7034-45fe-bf62-c3d273131ec5` | {1}{G/U} | Instant | Target creature has base power and toughness 4/4 until end of turn. |
| Zhalfirin Shapecraft | `cf459f47-e565-4bbb-89e4-ed54da8adae1` | {1}{U} | Instant | Target creature has base power and toughness 4/3 until end of turn. Draw a card. |

Exact records and pinned rulings: `build/campaign/queue-v53/families/03-base-power-toughness/source/<oracle-id>.json` and `rulings/<oracle-id>.jsonl`.

## Reuse

`quandrix_charm` mode 3 and `ghost_vacuum` entry modifier for `SetBasePowerToughness`; `defibrillating_current` for a single-target spell with a following effect. Rules: CR 611.2d/613.4b and 115.1.

## Evidence

- Intended red: `build/campaign/queue-v53/red-a.log` (0/7 exit 101, `MissingCard` with the nine batch A definitions held out).
- Focused scenario green: `build/campaign/queue-v53/green-a3.log` (8/8 exit 0).
- Focused registry green: `build/campaign/queue-v53/registry-a3.log` (4/4 exit 0).
- Review maps: `tricerules/tricerules-cards/authoring/review-maps/{diminish,square_up,zhalfirin_shapecraft}.json`.
- Scenario module: `tricerules/tricerules-core/tests/scenario/q53_batch_a.rs`.
- Registry module: `tricerules/tricerules-cards/tests/q53_batch_a.rs`.

## Assertions

- `q53_batch_a::q53_canary_simplest_diminish_sets_one_one`
- `q53_batch_a::q53_canary_hardest_zhalfirin_shapecraft_sets_four_three_and_draws`
- `q53_batch_a::q53_base_power_toughness_cards_apply_their_printed_values`
- `q53_batch_a::q53_base_power_toughness_fizzles_a_departed_target`
- `mechanic_registry::q53_batch_a::q53_base_power_toughness_cards_map_definitions`
- `mechanic_registry::q53_batch_a::q53_fingerprints_match_the_pinned_oracle_text`

## Delivery

- Independent read-only review: approved after two frozen delta reviews; final bundle `build/campaign/queue-v53/freeze/batch-a-final2.json` (`patch_sha256 eb5caaeb…`).
- Batch gate: `build/verification-logs/verify-3e020448-ea22-4afb-9399-b6c17988ab07/summary.json` passed exit 0 (`Side Rust`, `-CardData -CardsXml build/campaign/source/cards-validation.xml`) covering 03-base-power-toughness..05-activated-damage-player.
- Acceptance: rules-data only; no GUI, networking, privacy or physical-identity surface.
