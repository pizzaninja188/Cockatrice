# Packet: 04-haste-self-pump (3 cards)

Band: Multi-clause composition, 3 cards.

Stable typed shape: Haste creature with a mana-cost activated self-pump.

| Oracle ID | Card | Mana | Type line | P/T | Oracle text |
|---|---|---|---|---|---|
| 18bcf4c5-7abc-4ca5-b805-4bbebf4e433c | Crypt Ripper | {2}{B}{B} | Creature — Shade | 2/2 | Haste / {B}: This creature gets +1/+1 until end of turn. |
| 86b33d25-6123-439c-9026-a4e835b48562 | Furnace Spirit | {2}{R} | Creature — Spirit | 1/1 | Haste / {R}: This creature gets +1/+0 until end of turn. |
| 50bc464e-1c8d-455c-a870-ddbbc4618343 | Pyre Charger | {R}{R} | Creature — Elemental Warrior | 1/1 | Haste / {R}: This creature gets +1/+0 until end of turn. |

## Copy-from and differences

`nightwing_shade` for `PumpTarget(... subject: Source)` in an activated ability and `flamekin_brawler`/the self-pump cohort for the exact pump values; the haste keyword comes from `raging_goblin`-style bodies.

Per-card differences: Crypt Ripper pumps toughness as well as power; Furnace Spirit and Pyre Charger pump power only. All three are summoning-sick on entry and must survive to the next turn before attacking without another haste source.

## Rules evidence

CR 702.10 (haste), 602.2 (activate), 611.2c/613.4c (until-end-of-turn pump) and 514.2 (cleanup). No card in this cohort has a pinned ruling.

## Canaries

- Simplest: Pyre Charger ({R}{R}, 1/1, {R}: +1/+0).
- Hardest: Crypt Ripper ({2}{B}{B}, 2/2, {B}: +1/+1).
- Both are exercised by a focused canary scenario; every card is executed by the all-card loops, including the exact-cost, illegal-target/fizzle, scope and identity checks that apply to the family.

## Admission

All cards are Commander-legal normal-layout cards from the pinned bulk; exact source records and rulings are saved beside this packet. No candidate is blocked, excluded, already implemented, partial or a duplicate-name presentation source.

The pinned bulk SHA-256 is
`a9a2fd3b550cc5cff4cec9bbc82aaf5fb8e3f781b4b95a896a0d94c15dce7853` and the pinned
rulings SHA-256 is `3ee07dbf91347f7940b4067ab1c01e0d5097da8f6c27ef620c202ed4262aaf1f`.
