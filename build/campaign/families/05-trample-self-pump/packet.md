# Packet: 05-trample-self-pump (3 cards)

Band: Multi-clause composition, 3 cards.

Stable typed shape: Trample creature with a mana-cost activated self-pump.

| Oracle ID | Card | Mana | Type line | P/T | Oracle text |
|---|---|---|---|---|---|
| d8a3a89d-9d27-40ae-a1aa-d55cc6a79b0b | Almighty Brushwagg | {G} | Creature — Brushwagg | 1/1 | Trample / {3}{G}: This creature gets +3/+3 until end of turn. |
| 3a799843-1304-4abf-b8d7-db616628ee3f | Feral Ridgewolf | {2}{R} | Creature — Wolf | 1/2 | Trample / {1}{R}: This creature gets +2/+0 until end of turn. |
| 9debaed0-b605-4778-9977-216d194ecdeb | Sun-Collared Raptor | {1}{R} | Creature — Dinosaur | 1/2 | Trample / {2}{R}: This creature gets +3/+0 until end of turn. |

## Copy-from and differences

`nightwing_shade` for the activated self-pump and `valley_mightcaller`/trample cohort for the keyword.

Per-card differences: Feral Ridgewolf and Sun-Collared Raptor pump power only; Almighty Brushwagg pumps both. Activation costs and bonuses are card-specific and must not be collapsed.

## Rules evidence

CR 702.19 (trample), 602.2 (activate), 611.2c/613.4c (pump) and 510.1 (assigning combat damage). No card in this cohort has a pinned ruling.

## Canaries

- Simplest: Feral Ridgewolf ({2}{R}, 1/2, {1}{R}: +2/+0).
- Hardest: Almighty Brushwagg ({G}, 1/1, {3}{G}: +3/+3).
- Both are exercised by a focused canary scenario; every card is executed by the all-card loops, including the exact-cost, illegal-target/fizzle, scope and identity checks that apply to the family.

## Admission

All cards are Commander-legal normal-layout cards from the pinned bulk; exact source records and rulings are saved beside this packet. No candidate is blocked, excluded, already implemented, partial or a duplicate-name presentation source.

The pinned bulk SHA-256 is
`a9a2fd3b550cc5cff4cec9bbc82aaf5fb8e3f781b4b95a896a0d94c15dce7853` and the pinned
rulings SHA-256 is `3ee07dbf91347f7940b4067ab1c01e0d5097da8f6c27ef620c202ed4262aaf1f`.
