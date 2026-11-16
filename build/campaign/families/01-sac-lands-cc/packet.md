# Packet: 01-sac-lands-cc (10 cards)

Band: Simple single-primitive cohort, 10 cards.

Stable typed shape: Land with `EntersTapped` and two `{T}` mana abilities: one for a single colored mana, one costing `{T}, Sacrifice this land` for two colored mana.

| Oracle ID | Card | Mana | Type line | P/T | Oracle text |
|---|---|---|---|---|---|
| 2160849a-184f-4a49-8931-fd021e16f7cb | Ancient Spring | — | Land |  | This land enters tapped. / {T}: Add {U}. / {T}, Sacrifice this land: Add {W}{B}. |
| cbef6ad5-717f-4b02-9c62-e5deba407ad1 | Dwarven Ruins | — | Land |  | This land enters tapped. / {T}: Add {R}. / {T}, Sacrifice this land: Add {R}{R}. |
| 4d4b9512-d29d-4b99-9249-d37e1199a1f5 | Ebon Stronghold | — | Land |  | This land enters tapped. / {T}: Add {B}. / {T}, Sacrifice this land: Add {B}{B}. |
| 83f9d84a-a011-4fa9-a3a4-51eb39159c54 | Geothermal Crevice | — | Land |  | This land enters tapped. / {T}: Add {R}. / {T}, Sacrifice this land: Add {B}{G}. |
| 18cbb47a-85b1-48f6-a024-8c3bbffa0d87 | Havenwood Battleground | — | Land |  | This land enters tapped. / {T}: Add {G}. / {T}, Sacrifice this land: Add {G}{G}. |
| e0c3c87b-83e3-4cff-b05f-78a192e2bba4 | Irrigation Ditch | — | Land |  | This land enters tapped. / {T}: Add {W}. / {T}, Sacrifice this land: Add {G}{U}. |
| ad7b610c-e276-4c71-9b53-1ccb6d2dafd2 | Ruins of Trokair | — | Land |  | This land enters tapped. / {T}: Add {W}. / {T}, Sacrifice this land: Add {W}{W}. |
| bc2906cd-9e0c-4aa8-b656-fb1045d810ac | Sulfur Vent | — | Land |  | This land enters tapped. / {T}: Add {B}. / {T}, Sacrifice this land: Add {U}{R}. |
| 5578cf33-62f5-456c-a58c-f744a25df79b | Svyelunite Temple | — | Land |  | This land enters tapped. / {T}: Add {U}. / {T}, Sacrifice this land: Add {U}{U}. |
| af0ad159-8264-4526-9a95-eddd32c0a13f | Tinder Farm | — | Land |  | This land enters tapped. / {T}: Add {G}. / {T}, Sacrifice this land: Add {R}{W}. |

## Copy-from and differences

`sun-blessed_peak` (enters tapped plus two tap abilities) and `chromatic_star`/`signets` for a `SacrificeSelf` mana ability and a fixed multi-color `ProduceMana` option.

The printed single-mana color and the sacrificed pair differ per card; five cards sacrifice for two mana of one color and five for a two-color pair. All ten print exactly one colorless-free option each.

## Rules evidence

CR 614.1d and 614.12 (enters tapped is a replacement effect), CR 605.1a/605.3a-b (mana abilities resolve immediately), CR 118.3 and 701.21a (sacrifice as a cost), CR 106.1a-b (the produced mana's colors and types). No card in this cohort has a pinned ruling.

## Canaries

Simplest: Dwarven Ruins (`{T}: Add {R}`; sacrifice for `{R}{R}`). Hardest: Ancient Spring (`{T}: Add {U}`; sacrifice for `{W}{B}`).

## Admission

All cards are Commander-legal normal-layout cards from the pinned bulk; exact source records and rulings are saved beside this packet. No candidate is blocked, excluded, already implemented, partial or a duplicate-name presentation source.

The pinned bulk SHA-256 is
`a9a2fd3b550cc5cff4cec9bbc82aaf5fb8e3f781b4b95a896a0d94c15dce7853` and the pinned
rulings SHA-256 is `3ee07dbf91347f7940b4067ab1c01e0d5097da8f6c27ef620c202ed4262aaf1f`.
