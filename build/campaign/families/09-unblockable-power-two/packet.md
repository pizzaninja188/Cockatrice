# Packet: 09-unblockable-power-two (5 cards)

Band: Simple single-primitive cohort, 5 cards.

Stable typed shape: Creature with `{T}:` `ApplyCombatRestriction(scope: Chosen((kind: Creature, power: Some(AtMost(2)))), restriction: (cant_be_blocked: true))`.

| Oracle ID | Card | Mana | Type line | P/T | Oracle text |
|---|---|---|---|---|---|
| c4020178-71d9-4363-a0bd-b9a7eecd5d02 | Crafty Pathmage | {2}{U} | Creature — Human Wizard | 1/1 | {T}: Target creature with power 2 or less can't be blocked this turn. |
| db130ff4-7525-421f-b7e7-200921ecbdec | Dwarven Nomad | {2}{R} | Creature — Dwarf Nomad | 1/1 | {T}: Target creature with power 2 or less can't be blocked this turn. |
| cfc553cd-3b4c-47a9-bffb-e5790befb32c | Dwarven Warriors | {2}{R} | Creature — Dwarf Warrior | 1/1 | {T}: Target creature with power 2 or less can't be blocked this turn. |
| 1005bc16-8a6a-476a-a647-4ed40d2e96de | Goblin Tunneler | {1}{R} | Creature — Goblin Rogue | 1/1 | {T}: Target creature with power 2 or less can't be blocked this turn. |
| c0247a19-9c8f-422c-88cf-9f298c3a8c96 | Pathmaker Initiate | {1}{R} | Creature — Human Wizard | 2/1 | {T}: Target creature with power 2 or less can't be blocked this turn. |

## Copy-from and differences

`ragged_playmate` for the exact chosen-creature power-2-or-less can't-be-blocked restriction, with the tap-only cost of these cards.

The printed bodies and mana costs differ; every card uses only `{T}` as the activation cost.

## Rules evidence

CR 115.1 (targeting), CR 509.1a-b (evasion checked as blockers are declared), CR 608.2b (a target whose power is above 2 on resolution is illegal and the ability does not resolve) and CR 611.2a. The pinned Pathmaker Initiate and Goblin Tunneler rulings restate the resolution and post-resolution cases.

## Canaries

Simplest: Goblin Tunneler (`{1}{R}` 1/1). Hardest: Dwarven Warriors (`{2}{R}` 1/1) for the same tap ability with a different body.

## Admission

All cards are Commander-legal normal-layout cards from the pinned bulk; exact source records and rulings are saved beside this packet. No candidate is blocked, excluded, already implemented, partial or a duplicate-name presentation source.

The pinned bulk SHA-256 is
`a9a2fd3b550cc5cff4cec9bbc82aaf5fb8e3f781b4b95a896a0d94c15dce7853` and the pinned
rulings SHA-256 is `3ee07dbf91347f7940b4067ab1c01e0d5097da8f6c27ef620c202ed4262aaf1f`.
