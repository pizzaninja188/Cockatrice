# Packet: 03-sacrifice-draw (4 cards)

Band: Parameter family, 4 cards.

Stable typed shape: Spell with an additional creature-sacrifice cost and a fixed draw of two or three.

| Oracle ID | Card | Mana | Type line | P/T | Oracle text |
|---|---|---|---|---|---|
| c1cf9d74-e700-456a-9e5a-1c4df22db268 | Blood Divination | {3}{B} | Sorcery | — | As an additional cost to cast this spell, sacrifice a creature. / Draw three cards. |
| 46f78c42-85d1-4cd4-9fc3-d5fdec04a4c8 | Vivisection | {3}{U} | Sorcery | — | As an additional cost to cast this spell, sacrifice a creature. / Draw three cards. |
| 6a125750-2b8c-4f9d-8173-ac8d14c91ddb | Altar's Reap | {1}{B} | Instant | — | As an additional cost to cast this spell, sacrifice a creature. / Draw two cards. |
| 17c7878f-891f-40a6-80a7-47fcc7b8e29b | Skulltap | {1}{B} | Sorcery | — | As an additional cost to cast this spell, sacrifice a creature. / Draw two cards. |

## Copy-from and differences

village_rites and additional_spell_costs for the sacrifice additional cost; divination/rowdy_research draw values.

Draw count is two or three; Blood Divination and Vivisection are sorceries, Altar's Reap is an instant; no other text.

## Rules evidence

CR 118.8 and 601.2f-h (additional cost paid before casting), 701.21 (sacrifice), 121.1 (draw).

## Canaries

- Simplest: Altar's Reap ({1}{B} instant, draw two).
- Hardest: Vivisection ({3}{U} sorcery, draw three).

## Admission

All cards are Commander-legal normal-layout cards from the pinned bulk; exact
source records and rulings are saved beside this packet. No candidate is blocked,
excluded, already implemented, partial or a duplicate-name presentation source.

The pinned bulk SHA-256 is
`a9a2fd3b550cc5cff4cec9bbc82aaf5fb8e3f781b4b95a896a0d94c15dce7853` and the pinned
rulings SHA-256 is `3ee07dbf91347f7940b4067ab1c01e0d5097da8f6c27ef620c202ed4262aaf1f`.
