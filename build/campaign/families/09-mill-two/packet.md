# Packet: 09-mill-two (3 cards)

Band: Multi-clause composition, 3 cards.

Stable typed shape: `{M}, {T}: Target player mills two cards.`

| Oracle ID | Card | Mana | Type line | P/T | Oracle text |
|---|---|---|---|---|---|
| 3212e47a-5492-4c50-9d4a-6ea562f1a6e1 | Millstone | {2} | Artifact | — | {2}, {T}: Target player mills two cards. |
| 04cc5292-a485-4bb8-a143-9364229d480f | Vedalken Entrancer | {3}{U} | Creature — Vedalken Wizard | 1/4 | {U}, {T}: Target player mills two cards. |
| 590d524f-278d-4cf4-ae3f-efcced4517b1 | Merfolk Mesmerist | {1}{U} | Creature — Merfolk Wizard | 1/2 | {U}, {T}: Target player mills two cards. |

## Copy-from and differences

Copy `codex_shredder`'s activated `MillTargetPlayer` + `Tap` shape; set `count: 2` and add the printed mana cost. Only the printed mana cost, mill count and artifact-versus-creature identity differ.

## Rules evidence

CR 701.17a-b (mill), 602.1a/602.2 (activated ability and printed cost), 115.1/608.2b (target player), 118.3 (tap cost) and 121.1 (mill is not a draw). Millstone's pinned ruling confirms that a short library is milled as far as possible. The supported engine puts multiple milled cards into the graveyard in library order; no in-scope card observes graveyard order, so the owner's CR 404.3 order choice is deferred.

## Canaries

Simplest: Millstone (`millstone`). Hardest: Merfolk Mesmerist (`merfolk_mesmerist`).

## Admission

All cards are Commander-legal normal-layout cards from the pinned bulk; exact source records
and rulings are saved beside this packet. No candidate is blocked, excluded, already
implemented, partial or a duplicate-name presentation source.

The pinned bulk SHA-256 is
`a9a2fd3b550cc5cff4cec9bbc82aaf5fb8e3f781b4b95a896a0d94c15dce7853` and the pinned
rulings SHA-256 is `3ee07dbf91347f7940b4067ab1c01e0d5097da8f6c27ef620c202ed4262aaf1f`.
