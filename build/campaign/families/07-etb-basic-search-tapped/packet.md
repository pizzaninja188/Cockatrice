# Packet: 07-etb-basic-search-tapped (3 cards)

Band: Multi-clause composition, 3 cards.

Stable typed shape: Creature with one entry trigger: may search for a basic land, put it onto the battlefield tapped, then shuffle.

| Oracle ID | Card | Mana | Type line | P/T | Oracle text |
|---|---|---|---|---|---|
| 4ce2357f-93e6-40ca-beca-8f4e15adc464 | Farhaven Elf | {2}{G} | Creature — Elf Druid | 1/1 | When this creature enters, you may search your library for a basic land card, put it onto the battlefield tapped, then shuffle. |
| ed8b2887-15fa-4e62-83ac-55973ac6f76c | Ondu Giant | {3}{G} | Creature — Giant Druid | 2/4 | When this creature enters, you may search your library for a basic land card, put it onto the battlefield tapped, then shuffle. |
| ff16f889-9203-4874-919b-42e540cadcbc | Wild Wanderer | {3}{G} | Creature — Elf Druid | 3/2 | When this creature enters, you may search your library for a basic land card, put it onto the battlefield tapped, then shuffle. |

## Copy-from and differences

larval_scoutlander optional-search program and wood_elves battlefield search; sylvan_ranger optional private search.

The search is optional, finds any basic land, and the land enters tapped; nothing goes to hand.

## Rules evidence

CR 603.6a, 701.23 (search; a filtered search may fail to find), 701.24 (shuffle).

## Canaries

- Simplest: Farhaven Elf ({2}{G} 1/1).
- Hardest: Ondu Giant ({3}{G} 2/4).

## Admission

All cards are Commander-legal normal-layout cards from the pinned bulk; exact
source records and rulings are saved beside this packet. No candidate is blocked,
excluded, already implemented, partial or a duplicate-name presentation source.

The pinned bulk SHA-256 is
`a9a2fd3b550cc5cff4cec9bbc82aaf5fb8e3f781b4b95a896a0d94c15dce7853` and the pinned
rulings SHA-256 is `3ee07dbf91347f7940b4067ab1c01e0d5097da8f6c27ef620c202ed4262aaf1f`.
