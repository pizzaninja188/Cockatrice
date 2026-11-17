# Packet: 08-etb-pump-flying (3 cards)

Band: Multi-clause composition, 3 cards.

Stable typed shape: Flying creature whose entry trigger gives one chosen creature `+N/+N` and flying until end of turn.

| Oracle ID | Card | Mana | Type line | P/T | Oracle text |
|---|---|---|---|---|---|
| 0ce31ff1-6cfe-4b34-bd02-7d727934e754 | Welkin Guide | {4}{W} | Creature — Bird Cleric | 2/2 | Flying / When this creature enters, target creature gets +2/+2 and gains flying until end of turn. |
| 9a17452d-3adb-42ec-99f8-0083740cefa0 | Arborea Pegasus | {3}{W} | Creature — Pegasus | 2/3 | Flying / When this creature enters, target creature gets +1/+1 and gains flying until end of turn. |
| f36e316d-c0ad-414d-8cf0-b1c9ea779e03 | Battleflight Eagle | {4}{W} | Creature — Bird | 2/2 | Flying / When this creature enters, target creature gets +2/+2 and gains flying until end of turn. |

## Copy-from and differences

Compose `due_diligence`'s ordered `PumpTarget` + `GrantKeywords` with `gale_swooper`'s entry target; only the printed pump amount and body differ.

## Rules evidence

CR 702.9a-b (flying), 603.6a (enters-the-battlefield trigger), 115.1/608.2b (target), 611.2a (until-end-of-turn duration), 613.1f (layer-6 keyword grant) and 613.4c (layer-7c pump). Battleflight Eagle's pinned ruling confirms the ability is mandatory and may target the source itself.

## Canaries

Simplest: Welkin Guide (`welkin_guide`). Hardest: Battleflight Eagle (`battleflight_eagle`).

## Admission

All cards are Commander-legal normal-layout cards from the pinned bulk; exact source records
and rulings are saved beside this packet. No candidate is blocked, excluded, already
implemented, partial or a duplicate-name presentation source.

The pinned bulk SHA-256 is
`a9a2fd3b550cc5cff4cec9bbc82aaf5fb8e3f781b4b95a896a0d94c15dce7853` and the pinned
rulings SHA-256 is `3ee07dbf91347f7940b4067ab1c01e0d5097da8f6c27ef620c202ed4262aaf1f`.
