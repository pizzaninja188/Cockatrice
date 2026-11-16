# Packet: 01-dies-counter-target (5 cards)

Band: Multi-clause composition, 5 cards.

Stable typed shape: Creature with one dies trigger putting one +1/+1 counter on a chosen creature you control.

| Oracle ID | Card | Mana | Type line | P/T | Oracle text |
|---|---|---|---|---|---|
| 43d44a7f-829a-4f4d-9f71-2f4384ddd803 | Spinal Centipede | {2}{B} | Creature — Insect | 3/2 | When this creature dies, put a +1/+1 counter on target creature you control. |
| ad62cd58-fe6f-4844-b3dc-5ba21327fc43 | Guul Draz Mucklord | {2}{B} | Creature — Crocodile | 2/3 | When this creature dies, put a +1/+1 counter on target creature you control. |
| bc948e19-816b-4c13-a36b-73b3e0472dca | Sparring Construct | {1} | Artifact Creature — Construct | 1/1 | When this creature dies, put a +1/+1 counter on target creature you control. |
| d17d085f-aeda-4b23-ab80-729d070ae813 | Lawless Broker | {2}{B} | Creature — Aetherborn Rogue | 3/2 | When this creature dies, put a +1/+1 counter on target creature you control. |
| d464b54c-3b02-4c4e-99ef-1ad75f336c65 | Goblin Assault Team | {3}{R} | Creature — Goblin Warrior | 4/1 | Haste / When this creature dies, put a +1/+1 counter on target creature you control. |

## Copy-from and differences

steadfast_sentry for the dies trigger and targeted counter; spawn_of_mayhem/put-counter fixtures for the chosen-creature scope.

No vigilance; four cards have no other ability, and Goblin Assault Team adds haste. The counter target is any creature you control, including a different creature.

## Rules evidence

CR 700.4 (dies), 603.6c (leaves-the-battlefield trigger), 122.1 (counters), 608.2c (the controller follows the instruction).

## Canaries

- Simplest: Sparring Construct ({1} 1/1 artifact creature).
- Hardest: Goblin Assault Team (haste plus the dies trigger).

## Admission

All cards are Commander-legal normal-layout cards from the pinned bulk; exact
source records and rulings are saved beside this packet. No candidate is blocked,
excluded, already implemented, partial or a duplicate-name presentation source.

The pinned bulk SHA-256 is
`a9a2fd3b550cc5cff4cec9bbc82aaf5fb8e3f781b4b95a896a0d94c15dce7853` and the pinned
rulings SHA-256 is `3ee07dbf91347f7940b4067ab1c01e0d5097da8f6c27ef620c202ed4262aaf1f`.
