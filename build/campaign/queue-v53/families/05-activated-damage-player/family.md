# 05-activated-damage-player

Shape: A creature with a mana-only activated ability dealing a fixed amount of damage to one player or planeswalker target.

## Source and identity

| Card | Oracle ID | Cost | Type | P/T | Oracle text |
|---|---|---|---|---|---|
| Scalding Devil | `3bb1536f-06cc-433f-aeeb-afd528f9af03` | {1}{R} | Creature — Devil | 1/1 | {2}{R}: This creature deals 1 damage to target player or planeswalker. |
| Volcanic Rambler | `97f929ce-3eb3-4040-972f-0b21d5f2f61f` | {5}{R} | Creature — Elemental | 6/4 | {2}{R}: This creature deals 1 damage to target player or planeswalker. |
| Flamewave Invoker | `ee17706a-edc0-4301-8330-1da539679c7a` | {2}{R} | Creature — Goblin Mutant | 2/2 | {7}{R}: This creature deals 5 damage to target player or planeswalker. |

Exact records and pinned rulings: `build/campaign/queue-v53/families/05-activated-damage-player/source/<oracle-id>.json` and `rulings/<oracle-id>.jsonl`.

## Reuse

`chandra's_magmutt` for the player-or-planeswalker effect and `wyluli_wolf` for a mana-only activated creature ability. Rules: CR 602.2, 115.1 and 120.3.

## Evidence

- Intended red: `build/campaign/queue-v53/red-a.log` (0/7 exit 101).
- Focused scenario green: `build/campaign/queue-v53/green-a3.log` (8/8 exit 0).
- Focused registry green: `build/campaign/queue-v53/registry-a3.log` (4/4 exit 0).
- Review maps: `tricerules/tricerules-cards/authoring/review-maps/{scalding_devil,volcanic_rambler,flamewave_invoker}.json`.

## Assertions

- `q53_batch_a::q53_activated_damage_cards_deal_their_printed_damage`
- `q53_batch_a::q53_activated_damage_rejects_creature_targets`
- `mechanic_registry::q53_batch_a::q53_activated_damage_cards_map_definitions`
- `mechanic_registry::q53_batch_a::q53_fingerprints_match_the_pinned_oracle_text`

## Delivery

- Independent read-only review: approved after two frozen delta reviews; final bundle `build/campaign/queue-v53/freeze/batch-a-final2.json` (`patch_sha256 eb5caaeb…`).
- Batch gate: `build/verification-logs/verify-3e020448-ea22-4afb-9399-b6c17988ab07/summary.json` passed exit 0 (`Side Rust`, `-CardData -CardsXml build/campaign/source/cards-validation.xml`) covering 03-base-power-toughness..05-activated-damage-player.
- Acceptance: rules-data only; no GUI, networking, privacy or physical-identity surface.
