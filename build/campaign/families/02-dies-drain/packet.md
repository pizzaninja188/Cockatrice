# Packet: 02-dies-drain (5 cards)

Band: Multi-clause composition, 5 cards.

Stable typed shape: Creature with one dies trigger: each opponent loses N life and you gain N life.

| Oracle ID | Card | Mana | Type line | P/T | Oracle text |
|---|---|---|---|---|---|
| 0c766a86-9c70-465e-874b-9d9f3b4eb090 | Brotherhood Patriarch | {3}{B} | Creature — Human Assassin | 4/1 | When this creature dies, each opponent loses 2 life and you gain 2 life. |
| 15323e45-8260-43dc-b72b-efd661a58d75 | Spirit of Malevolence | {1}{B} | Creature — Spirit | 2/1 | When this creature dies, each opponent loses 1 life and you gain 1 life. |
| 7c3ff360-461a-44f3-a8f2-eb061b149cf5 | Symbiote Spawn | {2}{B} | Creature — Symbiote Villain | 3/2 | When this creature dies, each opponent loses 2 life and you gain 2 life. |
| e2bc064e-4739-4eb1-ba44-1fd92b2829d7 | Etched Familiar | {2}{B} | Artifact Creature — Phyrexian Fox | 3/2 | When this creature dies, each opponent loses 2 life and you gain 2 life. |
| f71bbaf2-02ea-4fc0-9a39-49d6689b36cd | Nocturnal Feeder | {2}{B} | Creature — Vampire Rogue | 2/1 | Flying / When this creature dies, each opponent loses 2 life and you gain 2 life. |

## Copy-from and differences

vampire_neonate for the fixed `LoseLife(EachOpponent)` plus fixed `GainLife` pair; shroudstomper for the same ordered pair on an entry trigger.

N is 1 or 2 and the gain is the fixed printed amount; Nocturnal Feeder also has flying. Kokusho is not an analogue: it gains life equal to the total lost, which is a different wording and a different primitive.

## Rules evidence

CR 700.4 (dies), 603.6c, 119.3 (life loss), 119.3 (life gain), 608.2c.

## Canaries

- Simplest: Spirit of Malevolence (1 life).
- Hardest: Nocturnal Feeder (flying plus 2 life).

## Admission

All cards are Commander-legal normal-layout cards from the pinned bulk; exact
source records and rulings are saved beside this packet. No candidate is blocked,
excluded, already implemented, partial or a duplicate-name presentation source.

The pinned bulk SHA-256 is
`a9a2fd3b550cc5cff4cec9bbc82aaf5fb8e3f781b4b95a896a0d94c15dce7853` and the pinned
rulings SHA-256 is `3ee07dbf91347f7940b4067ab1c01e0d5097da8f6c27ef620c202ed4262aaf1f`.
