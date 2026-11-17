# Packet: 09-sliver-anthem-controlled (5 cards)

Band: Simple single-primitive cohort, 5 cards.

Stable typed shape: Sliver with a controller-scoped `AnthemPt` for the Sliver subtype, including itself.

| Oracle ID | Card | Mana | Type line | P/T | Oracle text |
|---|---|---|---|---|---|
| d513e5df-31b0-4756-bf47-403460d0068e | Battle Sliver | {4}{R} | Creature — Sliver | 3/3 | Sliver creatures you control get +2/+0. |
| 9b33059c-c862-4452-8609-14a0e2551fb3 | Cleaving Sliver | {3}{R} | Creature — Sliver | 2/2 | Sliver creatures you control get +2/+0. |
| 52e8ec03-23ed-43a0-bce2-ad0cd5c4d737 | Megantic Sliver | {5}{G} | Creature — Sliver | 3/3 | Sliver creatures you control get +3/+3. |
| 6b589e70-9c1d-48dd-a597-479959e692bf | Predatory Sliver | {1}{G} | Creature — Sliver | 1/1 | Sliver creatures you control get +1/+1. |
| ae58e71c-b617-404f-a5dc-5e6a487f2675 | Steelform Sliver | {2}{W} | Creature — Sliver | 2/2 | Sliver creatures you control get +0/+1. |

## Copy-from and differences

Compose `corsair_captain`'s controller-scoped subtype anthem with the shipped Sliver anthems; `exclude_self` stays false because these print no "other".

## Rules evidence

CR 604.1/611.3a (continuous static ability), 613.4c (layer 7c), and 109.2 (a subtype description and the controller scope). The pinned rulings confirm cumulativity and that a creature that stops being a Sliver stops being affected.

## Canaries

Simplest: Predatory Sliver. Hardest: Megantic Sliver.

## Admission

All cards are Commander-legal normal-layout cards from the pinned bulk; exact source records
and rulings are saved beside this packet. No candidate is blocked, excluded, already
implemented, partial or a duplicate-name presentation source.

The pinned bulk SHA-256 is
`a9a2fd3b550cc5cff4cec9bbc82aaf5fb8e3f781b4b95a896a0d94c15dce7853` and the pinned
rulings SHA-256 is `3ee07dbf91347f7940b4067ab1c01e0d5097da8f6c27ef620c202ed4262aaf1f`.
