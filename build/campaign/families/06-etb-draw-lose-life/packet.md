# Packet: 06-etb-draw-lose-life (3 cards)

Band: Multi-clause composition, 3 cards.

Stable typed shape: Creature with one entry trigger: draw a card, then lose 1 life.

| Oracle ID | Card | Mana | Type line | P/T | Oracle text |
|---|---|---|---|---|---|
| ea2dce18-195e-4681-a687-f9819edaf9fc | Dusk Legion Zealot | {1}{B} | Creature — Vampire Soldier | 1/1 | When this creature enters, you draw a card and you lose 1 life. |
| e409c9be-0c9a-43c3-adb4-8c47afc1d551 | Phyrexian Rager | {2}{B} | Creature — Phyrexian Horror | 2/2 | When this creature enters, you draw a card and you lose 1 life. |
| 475aaf7c-b0f8-40f3-ad91-3e7d414de933 | Tithebearer Giant | {5}{B} | Creature — Giant Warrior | 4/5 | When this creature enters, you draw a card and you lose 1 life. |

## Copy-from and differences

audacious_thief for the ordered Draw then fixed LoseLife pair; shroudstomper is a different multi-clause entry example, not this pair.

The pair is on the entry trigger and the life loss is exactly 1; no other text.

## Rules evidence

CR 603.6a, 121.1 (draw), 119.3 (life loss), 608.2c (effects in written order).

## Canaries

- Simplest: Dusk Legion Zealot ({1}{B} 1/1).
- Hardest: Tithebearer Giant ({5}{B} 4/5).

## Admission

All cards are Commander-legal normal-layout cards from the pinned bulk; exact
source records and rulings are saved beside this packet. No candidate is blocked,
excluded, already implemented, partial or a duplicate-name presentation source.

The pinned bulk SHA-256 is
`a9a2fd3b550cc5cff4cec9bbc82aaf5fb8e3f781b4b95a896a0d94c15dce7853` and the pinned
rulings SHA-256 is `3ee07dbf91347f7940b4067ab1c01e0d5097da8f6c27ef620c202ed4262aaf1f`.
