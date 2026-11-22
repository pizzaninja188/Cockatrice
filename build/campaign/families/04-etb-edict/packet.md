# Packet: 04-etb-edict (3 cards)

Band: Multi-clause composition, 3 cards.

Stable typed shape: Creature with one entry trigger: each player sacrifices a creature of their choice.

| Oracle ID | Card | Mana | Type line | P/T | Oracle text |
|---|---|---|---|---|---|
| 4b1bf05e-753e-4350-a913-894cf3cecc0c | Fleshbag Marauder | {2}{B} | Creature — Zombie Warrior | 3/1 | When this creature enters, each player sacrifices a creature of their choice. |
| c3c45d50-9038-41df-bb2f-9bc40071845b | Merciless Executioner | {2}{B} | Creature — Orc Warrior | 3/1 | When this creature enters, each player sacrifices a creature of their choice. |
| 0cf742c1-feb5-4108-9976-37fa58528212 | Slum Reaper | {3}{B} | Creature — Horror | 4/2 | When this creature enters, each player sacrifices a creature of their choice. |

## Copy-from and differences

portal_to_phyrexia and professor_onyx ChooseAndSacrifice frames; the primitive documents Fleshbag Marauder's EachPlayer/1/Creature shape.

Recipient is EachPlayer rather than EachOpponent, count 1, creature only. Each player chooses their own sacrificed creature (CR 101.4).

## Rules evidence

CR 603.6a (entry trigger), 101.4/101.4b and 701.21a (each player chooses, simultaneous sacrifice).

## Canaries

- Simplest: Fleshbag Marauder ({2}{B} 3/1).
- Hardest: Slum Reaper ({3}{B} 4/2).

## Admission

All cards are Commander-legal normal-layout cards from the pinned bulk; exact
source records and rulings are saved beside this packet. No candidate is blocked,
excluded, already implemented, partial or a duplicate-name presentation source.

The pinned bulk SHA-256 is
`a9a2fd3b550cc5cff4cec9bbc82aaf5fb8e3f781b4b95a896a0d94c15dce7853` and the pinned
rulings SHA-256 is `3ee07dbf91347f7940b4067ab1c01e0d5097da8f6c27ef620c202ed4262aaf1f`.
