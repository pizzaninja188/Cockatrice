# Packet: 04-tri-mana-obelisks (7 cards)

Band: Parameter family, 7 cards.

Stable typed shape: Permanent with one `{T}` mana ability producing one mana of any of three printed colors via `ProduceMana(options: [...])`.

| Oracle ID | Card | Mana | Type line | P/T | Oracle text |
|---|---|---|---|---|---|
| 8d7933c4-4c74-492f-a9cd-50b339a4b29e | Druid of the Anima | {1}{G} | Creature — Elf Druid | 1/1 | {T}: Add {R}, {G}, or {W}. |
| 3f96dfd3-975a-4595-95c1-d2f04a86cb37 | Obelisk of Bant | {3} | Artifact |  | {T}: Add {G}, {W}, or {U}. |
| 9da963f6-734b-4d3c-a3be-dd5cca2c7b19 | Obelisk of Esper | {3} | Artifact |  | {T}: Add {W}, {U}, or {B}. |
| e193ae18-91a0-4870-b8a1-f8dedf744708 | Obelisk of Grixis | {3} | Artifact |  | {T}: Add {U}, {B}, or {R}. |
| eb0be178-55f9-4743-8128-34bc86f134ec | Obelisk of Jund | {3} | Artifact |  | {T}: Add {B}, {R}, or {G}. |
| 02e81bb8-ba80-4483-9b9f-973dcae0fc9d | Obelisk of Naya | {3} | Artifact |  | {T}: Add {R}, {G}, or {W}. |
| 9c382817-5c62-44ff-8f29-d283e65712a1 | Urborg Elf | {1}{G} | Creature — Elf Druid | 1/1 | {T}: Add {B}, {G}, or {U}. |

## Copy-from and differences

`arcane_sanctum` (three single-color `ProduceMana` options) and `azorius_signet` (artifact body with a mana ability).

Two cards are 1/1 Elf Druid creatures costing `{1}{G}`; five are `{3}` artifacts. The printed three-color set differs (shards and wedges).

## Rules evidence

CR 605.1a-b (mana ability), CR 106.1a-b (one mana of the chosen color), CR 305.6 (lands have no mana cost; these cards are not lands) and CR 202.1 (mana cost). No card in this cohort has a pinned ruling.

## Canaries

Simplest: Obelisk of Bant (`{T}: Add {G}, {W}, or {U}`). Hardest: Druid of the Anima (creature body plus `{1}{G}` cost).

## Admission

All cards are Commander-legal normal-layout cards from the pinned bulk; exact source records and rulings are saved beside this packet. No candidate is blocked, excluded, already implemented, partial or a duplicate-name presentation source.

The pinned bulk SHA-256 is
`a9a2fd3b550cc5cff4cec9bbc82aaf5fb8e3f781b4b95a896a0d94c15dce7853` and the pinned
rulings SHA-256 is `3ee07dbf91347f7940b4067ab1c01e0d5097da8f6c27ef620c202ed4262aaf1f`.
