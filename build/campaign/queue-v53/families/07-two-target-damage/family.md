# 07-two-target-damage

Shape: One-shot damage spells with two independent targets: a creature and a player-or-planeswalker. The second target still takes damage when the creature target is illegal at resolution.

## Source and identity

| Card | Oracle ID | Cost | Type | Oracle text |
|---|---|---|---|---|
| Lunge | `5e642112-9592-475f-ba03-d71544270cfa` | {2}{R} | Instant | Lunge deals 2 damage to target creature and 2 damage to target player or planeswalker. |
| Hungry Flames | `a2c00788-14e7-4ab5-8c71-2a176621a177` | {2}{R} | Instant | Hungry Flames deals 3 damage to target creature and 2 damage to target player or planeswalker. |
| Shower of Sparks | `a43500ed-4aa6-49de-8a6e-a1781301b108` | {R} | Instant | Shower of Sparks deals 1 damage to target creature and 1 damage to target player or planeswalker. |

Exact records and pinned rulings: `source/<oracle-id>.json` and `rulings/<oracle-id>.jsonl`.

## Reuse

`schismotivate`/`leeching_bite` for two independent target groups and `chandra's_magmutt` for the player-or-planeswalker damage target. Rules: CR 115.1/115.3, 120.3 and 608.2b.

## Evidence

- Intended red/green: pending batch B execution.
- Review maps: `tricerules/tricerules-cards/authoring/review-maps/*.json` after promotion.

## Delivery

- Independent read-only review: pending.
- Batch gate: pending.
- Acceptance: rules-data only; no GUI, networking, privacy or physical-identity surface.
