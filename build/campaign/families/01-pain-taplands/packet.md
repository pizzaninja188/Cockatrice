# Packet: 01-pain-taplands (5 cards)

Band: Multi-clause composition, 5 cards.

Stable typed shape: Entered-tapped land with `{T}: Add {C}` and `{T}: Add {X} or {Y}` plus 1 self-damage.

| Oracle ID | Card | Mana | Type line | P/T | Oracle text |
|---|---|---|---|---|---|
| 117a5fae-7fc4-4e24-b646-5727ea392fa7 | Skyshroud Forest |  | Land | — | This land enters tapped. / {T}: Add {C}. / {T}: Add {G} or {U}. This land deals 1 damage to you. |
| 2896f01d-003b-4d68-9d0a-64990ba59cbe | Scabland |  | Land | — | This land enters tapped. / {T}: Add {C}. / {T}: Add {R} or {W}. This land deals 1 damage to you. |
| 603dc263-270c-4a16-9b75-41f36fd7dfde | Pine Barrens |  | Land | — | This land enters tapped. / {T}: Add {C}. / {T}: Add {B} or {G}. This land deals 1 damage to you. |
| 7a951bd7-4f7d-44ea-9c2f-fe2f6b2f5289 | Salt Flats |  | Land | — | This land enters tapped. / {T}: Add {C}. / {T}: Add {W} or {B}. This land deals 1 damage to you. |
| c737d27b-db14-4bd4-8f16-bcbd4401c47b | Caldera Lake |  | Land | — | This land enters tapped. / {T}: Add {C}. / {T}: Add {U} or {R}. This land deals 1 damage to you. |

## Copy-from and differences

Copy `adarkar_wastes`'s two mana abilities and add `secluded_steppe`'s unconditional `EntersTapped(affected: Self_)`; only the printed name and the two produced colors differ.

## Rules evidence

CR 614.1d/614.12 (the enters-tapped replacement effect), 605.1a/605.3a-b (mana ability and immediate resolution), 120.3a (damage causes life loss) and 118.3 (tap cost). The cycle has no pinned ruling.

## Canaries

Simplest: Skyshroud Forest (`skyshroud_forest`). Hardest: Caldera Lake (`caldera_lake`).

## Admission

All cards are Commander-legal normal-layout cards from the pinned bulk; exact source records
and rulings are saved beside this packet. No candidate is blocked, excluded, already
implemented, partial or a duplicate-name presentation source.

The pinned bulk SHA-256 is
`a9a2fd3b550cc5cff4cec9bbc82aaf5fb8e3f781b4b95a896a0d94c15dce7853` and the pinned
rulings SHA-256 is `3ee07dbf91347f7940b4067ab1c01e0d5097da8f6c27ef620c202ed4262aaf1f`.
