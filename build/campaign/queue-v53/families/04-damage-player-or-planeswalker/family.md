# 04-damage-player-or-planeswalker

Shape: One-shot sorcery or instant dealing a fixed amount of damage to one player or planeswalker target.

## Source and identity

| Card | Oracle ID | Cost | Type | Oracle text |
|---|---|---|---|---|
| Lava Spike | `2837888c-bfa8-4955-9334-0605ad409f7e` | {R} | Sorcery — Arcane | Lava Spike deals 3 damage to target player or planeswalker. |
| Lava Axe | `387b6b07-a283-412d-94c3-f7f1dc76e858` | {4}{R} | Sorcery | Lava Axe deals 5 damage to target player or planeswalker. |
| Sarkhan's Catharsis | `787935ab-26a6-4b4a-9c14-e106160e951d` | {4}{R} | Instant | Sarkhan's Catharsis deals 5 damage to target player or planeswalker. |

Exact records and pinned rulings: `build/campaign/queue-v53/families/04-damage-player-or-planeswalker/source/<oracle-id>.json` and `rulings/<oracle-id>.jsonl`.

## Reuse

`chandra's_magmutt` for the player-or-planeswalker `DamageTarget` union and `obliterating_bolt` for the disjunctive permanent branch. Rules: CR 115.1, 120.3 and 608.2b.

## Evidence

- Intended red: `build/campaign/queue-v53/red-a.log` (0/7 exit 101).
- Focused scenario green: `build/campaign/queue-v53/green-a3.log` (8/8 exit 0).
- Focused registry green: `build/campaign/queue-v53/registry-a3.log` (4/4 exit 0).
- Review maps: `tricerules/tricerules-cards/authoring/review-maps/{lava_spike,lava_axe,sarkhans_catharsis}.json`.

## Assertions

- `q53_batch_a::q53_player_damage_cards_deal_their_printed_damage`
- `q53_batch_a::q53_player_damage_cards_reject_creature_targets`
- `mechanic_registry::q53_batch_a::q53_player_damage_cards_map_definitions`
- `mechanic_registry::q53_batch_a::q53_fingerprints_match_the_pinned_oracle_text`

## Delivery

- Independent read-only review: approved after two frozen delta reviews; final bundle `build/campaign/queue-v53/freeze/batch-a-final2.json` (`patch_sha256 eb5caaeb…`).
- Batch gate: `build/verification-logs/verify-3e020448-ea22-4afb-9399-b6c17988ab07/summary.json` passed exit 0 (`Side Rust`, `-CardData -CardsXml build/campaign/source/cards-validation.xml`) covering 03-base-power-toughness..05-activated-damage-player.
- Acceptance: rules-data only; no GUI, networking, privacy or physical-identity surface.
