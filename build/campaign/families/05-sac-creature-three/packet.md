# Packet: 05-sac-creature-three (5 cards)

Band: Simple single-primitive cohort, 5 cards.

Stable typed shape: Artifact Creature — Golem with `{1}, Sacrifice this creature: Add <three fixed mana>`.

| Oracle ID | Card | Mana | Type line | P/T | Oracle text |
|---|---|---|---|---|---|
| 6223b1c0-bfe0-490c-b2d4-28537b05f571 | Crosis's Attendant | {5} | Artifact Creature — Golem | 3/3 | {1}, Sacrifice this creature: Add {U}{B}{R}. |
| c0ad16d7-2550-47e0-8581-0e22b27cb0d0 | Darigaaz's Attendant | {5} | Artifact Creature — Golem | 3/3 | {1}, Sacrifice this creature: Add {B}{R}{G}. |
| 91d36a9d-3a3f-4cb7-837d-b1e8f6730512 | Dromar's Attendant | {5} | Artifact Creature — Golem | 3/3 | {1}, Sacrifice this creature: Add {W}{U}{B}. |
| 781542b7-155b-4a66-aa52-eec0ebb29bb2 | Rith's Attendant | {5} | Artifact Creature — Golem | 3/3 | {1}, Sacrifice this creature: Add {R}{G}{W}. |
| ff7d927f-0a01-4243-ad00-c65686ee86bb | Treva's Attendant | {5} | Artifact Creature — Golem | 3/3 | {1}, Sacrifice this creature: Add {G}{W}{U}. |

## Copy-from and differences

`chromatic_star` for `Mana + SacrificeSelf` producing mana and `azorius_signet` for a fixed multi-color `ProduceMana` option. The same card body is the only printed difference.

The three produced colors differ per card (all five shard triples are covered).

## Rules evidence

CR 605.1a-b (mana ability), CR 118.3 and 701.21a (sacrifice as a cost), CR 106.1a-b (one mana per symbol) and CR 301.1 (artifact creatures). No card in this cohort has a pinned ruling.

## Canaries

Simplest: Treva's Attendant (`{G}{W}{U}`). Hardest: Crosis's Attendant (`{U}{B}{R}`) for the remaining color order.

## Admission

All cards are Commander-legal normal-layout cards from the pinned bulk; exact source records and rulings are saved beside this packet. No candidate is blocked, excluded, already implemented, partial or a duplicate-name presentation source.

The pinned bulk SHA-256 is
`a9a2fd3b550cc5cff4cec9bbc82aaf5fb8e3f781b4b95a896a0d94c15dce7853` and the pinned
rulings SHA-256 is `3ee07dbf91347f7940b4067ab1c01e0d5097da8f6c27ef620c202ed4262aaf1f`.
