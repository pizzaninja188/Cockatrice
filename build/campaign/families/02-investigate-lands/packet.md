# Packet: 02-investigate-lands (10 cards)

Band: Parameter family, 10 cards.

Stable typed shape: Land with `EntersTapped`, a dual-color `{T}` mana ability and `{4}, {T}: Investigate`.

| Oracle ID | Card | Mana | Type line | P/T | Oracle text |
|---|---|---|---|---|---|
| cb91e842-9f06-4863-a328-2cabe1bcfe27 | Ballroom | — | Land |  | This land enters tapped. / {T}: Add {W} or {B}. / {4}, {T}: Investigate. (Create a Clue token. It's an artifact with "{2}, Sacrifice this token: Draw a card.") |
| a4fc174e-7fa6-41a8-ae03-255f226840f9 | Billiard Room | — | Land |  | This land enters tapped. / {T}: Add {B} or {R}. / {4}, {T}: Investigate. (Create a Clue token. It's an artifact with "{2}, Sacrifice this token: Draw a card.") |
| 8f88b0bf-81bd-4223-9dad-d8c49ff4b87a | Conservatory | — | Land |  | This land enters tapped. / {T}: Add {G} or {W}. / {4}, {T}: Investigate. (Create a Clue token. It's an artifact with "{2}, Sacrifice this token: Draw a card.") |
| 0880461e-8943-443b-90e7-ff84eef46550 | Dining Room | — | Land |  | This land enters tapped. / {T}: Add {R} or {G}. / {4}, {T}: Investigate. (Create a Clue token. It's an artifact with "{2}, Sacrifice this token: Draw a card.") |
| 949e455d-6a9e-491a-892f-826cc8be0fd9 | Hall | — | Land |  | This land enters tapped. / {T}: Add {R} or {W}. / {4}, {T}: Investigate. (Create a Clue token. It's an artifact with "{2}, Sacrifice this token: Draw a card.") |
| 47f71408-5509-4651-9121-fd0867adae00 | Kitchen | — | Land |  | This land enters tapped. / {T}: Add {G} or {U}. / {4}, {T}: Investigate. (Create a Clue token. It's an artifact with "{2}, Sacrifice this token: Draw a card.") |
| 33a7d29a-ebc0-4606-91b6-7381e2a16016 | Library | — | Land |  | This land enters tapped. / {T}: Add {U} or {R}. / {4}, {T}: Investigate. (Create a Clue token. It's an artifact with "{2}, Sacrifice this token: Draw a card.") |
| 98a22879-d0b2-441f-bcf6-a67b4552b73c | Lounge | — | Land |  | This land enters tapped. / {T}: Add {B} or {G}. / {4}, {T}: Investigate. (Create a Clue token. It's an artifact with "{2}, Sacrifice this token: Draw a card.") |
| c6a46fc2-fc8f-4dcd-bca7-682abfbf303d | Secret Passage | — | Land |  | This land enters tapped. / {T}: Add {U} or {B}. / {4}, {T}: Investigate. (Create a Clue token. It's an artifact with "{2}, Sacrifice this token: Draw a card.") |
| 7ffadb3f-0b88-416f-b1a1-876383c22720 | Study | — | Land |  | This land enters tapped. / {T}: Add {W} or {U}. / {4}, {T}: Investigate. (Create a Clue token. It's an artifact with "{2}, Sacrifice this token: Draw a card.") |

## Copy-from and differences

`sun-blessed_peak` for the entered-tapped dual land shell and `callous_inspector`/`air_nomad_legacy` for investigate implemented as `CreateTokens(token: "clue")`.

The two printed mana colors differ per card; every card creates exactly one Clue token and prints the same reminder text.

## Rules evidence

CR 614.1d/614.12 (enters tapped), CR 605.1a/605.3a-b (mana ability resolves immediately), CR 701.16a (investigate creates one Clue artifact token) and CR 111.10f (Clue token characteristics). The pinned rulings for this cycle restate Clue artifact-type and sacrifice facts; none changes the typed shape.

## Canaries

Simplest: Study (`{T}: Add {W} or {U}`). Hardest: Secret Passage (`{T}: Add {U} or {B}`), exercised with the same investigate evidence.

## Admission

All cards are Commander-legal normal-layout cards from the pinned bulk; exact source records and rulings are saved beside this packet. No candidate is blocked, excluded, already implemented, partial or a duplicate-name presentation source.

The pinned bulk SHA-256 is
`a9a2fd3b550cc5cff4cec9bbc82aaf5fb8e3f781b4b95a896a0d94c15dce7853` and the pinned
rulings SHA-256 is `3ee07dbf91347f7940b4067ab1c01e0d5097da8f6c27ef620c202ed4262aaf1f`.
