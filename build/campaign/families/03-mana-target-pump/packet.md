# Packet: 03-mana-target-pump (4 cards)

Band: Multi-clause composition, 4 cards.

Stable typed shape: Permanent with a mana-cost activated ability pumping a chosen target creature.

| Oracle ID | Card | Mana | Type line | P/T | Oracle text |
|---|---|---|---|---|---|
| 0439df39-0324-4110-b3f4-4a32393021de | Ghitu War Cry | {2}{R} | Enchantment |  | {R}: Target creature gets +1/+0 until end of turn. |
| fada1102-bbb7-4c90-a72e-6c595c08a55d | Captive Flame | {2}{R} | Enchantment |  | {R}: Target creature gets +1/+0 until end of turn. |
| 37d74df9-cf87-4b82-842a-3941064078fb | Sacred Armory | {2} | Artifact |  | {2}: Target creature gets +1/+0 until end of turn. |
| fcf92abb-6c06-42d2-a3d1-f44ec997d56d | Ursapine | {3}{G}{G} | Creature — Beast | 3/3 | {G}: Target creature gets +1/+1 until end of turn. |

## Copy-from and differences

`nylea_god_of_the_hunt` for `PumpTarget(... subject: Chosen((kind: Creature)))` in an activated ability; the enchantment/artifact permanent shapes come from `ghitu_war_cry`'s closest shipped artifacts and enchantments.

Per-card differences: Ghitu War Cry and Captive Flame are identical {2}{R} enchantments with {R}: +1/+0; Sacred Armory is a colorless artifact with {2}: +1/+0; Ursapine is a 3/3 creature with {G}: +1/+1. All four target any creature, including an opponent's.

## Rules evidence

CR 602.2 (activate), 601.2c/115.1 (target chosen at activation), 611.2c/613.4c (until-end-of-turn modifier), and 514.2 (the modifier ends during cleanup). No card in this cohort has a pinned ruling.

## Canaries

- Simplest: Sacred Armory ({2} artifact, {2} activation).
- Hardest: Ursapine ({3}{G}{G} creature, {G} activation).
- Both are exercised by a focused canary scenario; every card is executed by the all-card loops, including the exact-cost, illegal-target/fizzle, scope and identity checks that apply to the family.

## Admission

All cards are Commander-legal normal-layout cards from the pinned bulk; exact source records and rulings are saved beside this packet. No candidate is blocked, excluded, already implemented, partial or a duplicate-name presentation source.

The pinned bulk SHA-256 is
`a9a2fd3b550cc5cff4cec9bbc82aaf5fb8e3f781b4b95a896a0d94c15dce7853` and the pinned
rulings SHA-256 is `3ee07dbf91347f7940b4067ab1c01e0d5097da8f6c27ef620c202ed4262aaf1f`.
