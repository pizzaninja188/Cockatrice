# Packet: 02-slow-lands (5 cards)

Band: Simple single-primitive cohort, 5 cards.

Stable typed shape: Land whose `EntersTapped` condition counts at most one other land you control, then a two-option `{T}` mana ability.

| Oracle ID | Card | Mana | Type line | P/T | Oracle text |
|---|---|---|---|---|---|
| f0ec8681-da50-466b-8cdd-1dc710deccd9 | Deserted Beach | — | Land |  | This land enters tapped unless you control two or more other lands. / {T}: Add {W} or {U}. |
| e2a37967-4212-4553-9f77-bcb613405807 | Haunted Ridge | — | Land |  | This land enters tapped unless you control two or more other lands. / {T}: Add {B} or {R}. |
| 709d2f10-1585-48c3-9058-ddd5f62f0452 | Overgrown Farmland | — | Land |  | This land enters tapped unless you control two or more other lands. / {T}: Add {G} or {W}. |
| 185c70c1-8403-4ae5-b45d-3679d4ee092a | Rockfall Vale | — | Land |  | This land enters tapped unless you control two or more other lands. / {T}: Add {R} or {G}. |
| 5f42b67f-87fd-4f98-a0e8-0c8313f4bbc8 | Shipwreck Marsh | — | Land |  | This land enters tapped unless you control two or more other lands. / {T}: Add {U} or {B}. |

## Copy-from and differences

Copy `deathcap_glade` exactly; only the printed name and the two produced colors differ.

## Rules evidence

CR 614.1d/614.12 (enters-tapped replacement effect), 605.1a/605.3b (mana ability), 109.2 (a land on the battlefield is not an "other land" for itself), and 305.6 (land mana abilities). The pinned ruling states that lands entering at the same time are not counted.

## Canaries

Simplest: Deserted Beach. Hardest: Shipwreck Marsh.

## Admission

All cards are Commander-legal normal-layout cards from the pinned bulk; exact source records
and rulings are saved beside this packet. No candidate is blocked, excluded, already
implemented, partial or a duplicate-name presentation source.

The pinned bulk SHA-256 is
`a9a2fd3b550cc5cff4cec9bbc82aaf5fb8e3f781b4b95a896a0d94c15dce7853` and the pinned
rulings SHA-256 is `3ee07dbf91347f7940b4067ab1c01e0d5097da8f6c27ef620c202ed4262aaf1f`.
