# Packet: 02-equipment-haste (3 cards)

Band: Simple single-primitive cohort, 3 cards.

Stable typed shape: Equipment whose attached modifier grants a fixed pump and haste, with equip {1}.

| Oracle ID | Card | Mana | Type line | P/T | Oracle text |
|---|---|---|---|---|---|
| 6f6601bc-cb12-4054-9027-e4a0ebdae1a3 | Boots of Speed | {R} | Artifact — Equipment |  | Equipped creature gets +1/+0 and has haste. / Equip {1} ({1}: Attach to target creature you control. Equip only as a sorcery.) |
| 8c2d2888-38be-41f9-aaf2-8337e87d2aed | Crystal Slipper | {1}{R} | Artifact — Equipment |  | Equipped creature gets +1/+0 and has haste. / Equip {1} ({1}: Attach to target creature you control. Equip only as a sorcery.) |
| bd4ad383-8c40-4dda-bc9c-ba8a001d6882 | Strider Harness | {3} | Artifact — Equipment |  | Equipped creature gets +1/+1 and has haste. / Equip {1} ({1}: Attach to target creature you control. Equip only as a sorcery.) |

## Copy-from and differences

`accorders_shield` for `AttachedModifier` with a keyword and `Torch Gauntlet`/`Dueling Rapier` for the Equipment/equip shape. No entry trigger and no flash.

Per-card differences: Boots of Speed and Crystal Slipper grant +1/+0; Strider Harness grants +1/+1. All three cost {1} to equip and have no other ability.

## Rules evidence

CR 301.5/702.6 (Equipment and equip), 613.4c (attached modifier), 702.10 (haste) and 704.5n (an Equipment unattaches when its creature leaves). The Crystal Slipper and Strider Harness rulings confirm that haste must still be present at the declare-attackers step.

## Canaries

- Simplest: Boots of Speed ({R}, +1/+0).
- Hardest: Strider Harness ({3}, +1/+1).
- Both are exercised by a focused canary scenario; every card is executed by the all-card loops, including the exact-cost, illegal-target/fizzle, scope and identity checks that apply to the family.

## Admission

All cards are Commander-legal normal-layout cards from the pinned bulk; exact source records and rulings are saved beside this packet. No candidate is blocked, excluded, already implemented, partial or a duplicate-name presentation source.

The pinned bulk SHA-256 is
`a9a2fd3b550cc5cff4cec9bbc82aaf5fb8e3f781b4b95a896a0d94c15dce7853` and the pinned
rulings SHA-256 is `3ee07dbf91347f7940b4067ab1c01e0d5097da8f6c27ef620c202ed4262aaf1f`.
