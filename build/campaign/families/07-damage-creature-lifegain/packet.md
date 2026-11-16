# Packet: 07-damage-creature-lifegain (6 cards)

Band: Multi-clause composition, 6 cards.

Stable typed shape: Spell dealing fixed damage to a chosen creature and gaining that much life.

| Oracle ID | Card | Mana | Type line | P/T | Oracle text |
|---|---|---|---|---|---|
| 06d77149-52b6-4d2c-bfb9-1c25f6b1b0bb | Pharika's Cure | {B}{B} | Instant |  | Pharika's Cure deals 2 damage to target creature and you gain 2 life. |
| 14363df6-ac0f-476a-bbd8-ce351de0babf | Essence Extraction | {1}{B}{B} | Instant |  | Essence Extraction deals 3 damage to target creature and you gain 3 life. |
| 1fd754fd-c053-4638-b379-46d903357b01 | Douse in Gloom | {2}{B} | Instant |  | Douse in Gloom deals 2 damage to target creature and you gain 2 life. |
| 38ea22cd-2c5d-4f66-a111-207aca4c67c3 | Vicious Hunger | {B}{B} | Sorcery |  | Vicious Hunger deals 2 damage to target creature and you gain 2 life. |
| bc59bf58-373d-4562-9a67-0721fcf2f098 | Last Kiss | {2}{B} | Instant |  | Last Kiss deals 2 damage to target creature and you gain 2 life. |
| ff27ff37-96c0-41af-8881-a078e884e67b | Sorin's Thirst | {B}{B} | Instant |  | Sorin's Thirst deals 2 damage to target creature and you gain 2 life. |

## Copy-from and differences

`winters_intervention` for the exact `DamageTarget(... kind: Creature)` plus `GainLife` sequence and targeting group.

Per-card differences: Damage and life are 2 or 3; Vicious Hunger is a sorcery, the other five are instants. All six target a creature only, never a player.

## Rules evidence

CR 115.1 (target), 120.3 (damage), 119.3 (life gain) and 608.2b (an illegal target fizzles the whole spell). The Pharika's Cure and Last Kiss rulings confirm both the fizzle rule and the prevented-damage life gain; the Essence Extraction and Sorin's Thirst rulings confirm the fizzle rule.

## Canaries

- Simplest: Pharika's Cure ({B}{B}, 2 damage).
- Hardest: Essence Extraction ({1}{B}{B}, 3 damage).
- Both are exercised by a focused canary scenario; every card is executed by the all-card loops, including the exact-cost, illegal-target/fizzle, scope and identity checks that apply to the family.

## Admission

All cards are Commander-legal normal-layout cards from the pinned bulk; exact source records and rulings are saved beside this packet. No candidate is blocked, excluded, already implemented, partial or a duplicate-name presentation source.

The pinned bulk SHA-256 is
`a9a2fd3b550cc5cff4cec9bbc82aaf5fb8e3f781b4b95a896a0d94c15dce7853` and the pinned
rulings SHA-256 is `3ee07dbf91347f7940b4067ab1c01e0d5097da8f6c27ef620c202ed4262aaf1f`.
