# Packet: 05-etb-cant-block (3 cards)

Band: Multi-clause composition, 3 cards.

Stable typed shape: Creature with one entry trigger: target creature can't block this turn.

| Oracle ID | Card | Mana | Type line | P/T | Oracle text |
|---|---|---|---|---|---|
| 543071da-fc89-4274-a2ae-d5d755d1f2a9 | Crossway Vampire | {1}{R}{R} | Creature — Vampire | 3/2 | When this creature enters, target creature can't block this turn. |
| 29863706-fa4e-49ed-9f40-69ae503abba3 | Goblin Shortcutter | {1}{R} | Creature — Goblin Scout | 2/1 | When this creature enters, target creature can't block this turn. |
| 1733777d-87fe-4975-b099-1ff803a721e8 | Unstoppable Ogre | {2}{R} | Artifact Creature — Ogre Warrior | 4/1 | When this creature enters, target creature can't block this turn. |

## Copy-from and differences

summit_intimidator and zog,_triceraton_castaway for WhenSelfEntersBattlefield plus ApplyCombatRestriction(Chosen, cant_block).

No other ability; Unstoppable Ogre is an artifact creature. The restriction expires at cleanup.

## Rules evidence

CR 603.6a, 509.1b (a declared block is unaffected), 611.2a and 514.2 (until-end-of-turn expiry).

## Canaries

- Simplest: Goblin Shortcutter ({1}{R} 2/1).
- Hardest: Crossway Vampire ({1}{R}{R} 3/2).

## Admission

All cards are Commander-legal normal-layout cards from the pinned bulk; exact
source records and rulings are saved beside this packet. No candidate is blocked,
excluded, already implemented, partial or a duplicate-name presentation source.

The pinned bulk SHA-256 is
`a9a2fd3b550cc5cff4cec9bbc82aaf5fb8e3f781b4b95a896a0d94c15dce7853` and the pinned
rulings SHA-256 is `3ee07dbf91347f7940b4067ab1c01e0d5097da8f6c27ef620c202ed4262aaf1f`.
