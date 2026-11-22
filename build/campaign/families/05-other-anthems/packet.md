# Packet: 05-other-anthems (3 cards)

Band: Simple single-primitive cohort, 3 cards.

Stable typed shape: Controller-scoped layer-7c `AnthemPt` with `exclude_self`.

| Oracle ID | Card | Mana | Type line | P/T | Oracle text |
|---|---|---|---|---|---|
| 1b1214e9-1581-4008-b3d4-8b7af6b092fd | Veteran Armorer | {1}{W} | Creature — Human Soldier | 2/2 | Other creatures you control get +0/+1. |
| 2cc439e8-d112-44e6-bc5a-6e99333c519a | Benalish Marshal | {W}{W}{W} | Creature — Human Knight | 3/3 | Other creatures you control get +1/+1. |
| c0856fea-cbdc-427b-ae00-bfd9311f284f | Blessed Orator | {3}{W} | Creature — Human Cleric | 1/4 | Other creatures you control get +0/+1. |

## Copy-from and differences

Copy `corsair_captain`'s controller-scoped `AnthemPt` filter and clear the subtype, because these cards pump every other creature you control.

## Rules evidence

CR 604.2/611.3b (the continuous effect is active while the source remains on the battlefield), 613.4c (layer 7c) and 109.5 ("you" is the source's controller); the printed "other" excludes the source. Benalish Marshal's pinned ruling confirms that marked damage can become lethal when the anthem leaves.

## Canaries

Simplest: Veteran Armorer (`veteran_armorer`). Hardest: Blessed Orator (`blessed_orator`).

## Admission

All cards are Commander-legal normal-layout cards from the pinned bulk; exact source records
and rulings are saved beside this packet. No candidate is blocked, excluded, already
implemented, partial or a duplicate-name presentation source.

The pinned bulk SHA-256 is
`a9a2fd3b550cc5cff4cec9bbc82aaf5fb8e3f781b4b95a896a0d94c15dce7853` and the pinned
rulings SHA-256 is `3ee07dbf91347f7940b4067ab1c01e0d5097da8f6c27ef620c202ed4262aaf1f`.
