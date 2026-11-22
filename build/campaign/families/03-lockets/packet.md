# Packet: 03-lockets (10 cards)

Band: Parameter family, 10 cards.

Stable typed shape: `{3}` artifact with `{T}: Add {X} or {Y}` and four hybrid symbols, `{T}, Sacrifice this artifact: Draw two cards`.

| Oracle ID | Card | Mana | Type line | P/T | Oracle text |
|---|---|---|---|---|---|
| ca13dda2-fbb0-4b6b-9126-3c8df04dee2b | Azorius Locket | {3} | Artifact | — | {T}: Add {W} or {U}. / {W/U}{W/U}{W/U}{W/U}, {T}, Sacrifice this artifact: Draw two cards. |
| b044b4fd-d504-4a11-80bf-eb8fb0b6e21e | Boros Locket | {3} | Artifact | — | {T}: Add {R} or {W}. / {R/W}{R/W}{R/W}{R/W}, {T}, Sacrifice this artifact: Draw two cards. |
| 3ae4b679-2e3c-4f60-bcf4-b279917cdc54 | Dimir Locket | {3} | Artifact | — | {T}: Add {U} or {B}. / {U/B}{U/B}{U/B}{U/B}, {T}, Sacrifice this artifact: Draw two cards. |
| dba6e24e-7875-46b9-b51d-4cd71568ecfa | Golgari Locket | {3} | Artifact | — | {T}: Add {B} or {G}. / {B/G}{B/G}{B/G}{B/G}, {T}, Sacrifice this artifact: Draw two cards. |
| 7018ccd0-96e6-458f-b412-197edcb2bafd | Gruul Locket | {3} | Artifact | — | {T}: Add {R} or {G}. / {R/G}{R/G}{R/G}{R/G}, {T}, Sacrifice this artifact: Draw two cards. |
| 7ee07546-2c2e-4679-b671-7ea509b1d3ea | Izzet Locket | {3} | Artifact | — | {T}: Add {U} or {R}. / {U/R}{U/R}{U/R}{U/R}, {T}, Sacrifice this artifact: Draw two cards. |
| 8aaf248e-66cf-447b-b062-9aa2ab1ec864 | Orzhov Locket | {3} | Artifact | — | {T}: Add {W} or {B}. / {W/B}{W/B}{W/B}{W/B}, {T}, Sacrifice this artifact: Draw two cards. |
| ab784412-0722-4501-9aa0-fd68d307f6fd | Rakdos Locket | {3} | Artifact | — | {T}: Add {B} or {R}. / {B/R}{B/R}{B/R}{B/R}, {T}, Sacrifice this artifact: Draw two cards. |
| 9ecd98d5-8d66-4cb0-bada-23dfa0faacc0 | Selesnya Locket | {3} | Artifact | — | {T}: Add {G} or {W}. / {G/W}{G/W}{G/W}{G/W}, {T}, Sacrifice this artifact: Draw two cards. |
| c567fed8-df00-4579-901e-8754079105bf | Simic Locket | {3} | Artifact | — | {T}: Add {G} or {U}. / {G/U}{G/U}{G/U}{G/U}, {T}, Sacrifice this artifact: Draw two cards. |

## Copy-from and differences

Copy `mind_stone`'s sacrifice-draw ability with `Draw(count: 2)` and the shipped hybrid payment contract (`unmake`, `hovel_hurler`); only the two produced colors and the hybrid pair differ.

## Rules evidence

CR 107.4e (each hybrid symbol may be paid with either half), 605.1a/605.3a-b (artifact mana ability and immediate resolution), 118.3 and 701.21a (tap and sacrifice costs) and 121.1 (draw two). The cycle has no pinned ruling.

## Canaries

Simplest: Azorius Locket (`azorius_locket`). Hardest: Simic Locket (`simic_locket`).

## Admission

All cards are Commander-legal normal-layout cards from the pinned bulk; exact source records
and rulings are saved beside this packet. No candidate is blocked, excluded, already
implemented, partial or a duplicate-name presentation source.

The pinned bulk SHA-256 is
`a9a2fd3b550cc5cff4cec9bbc82aaf5fb8e3f781b4b95a896a0d94c15dce7853` and the pinned
rulings SHA-256 is `3ee07dbf91347f7940b4067ab1c01e0d5097da8f6c27ef620c202ed4262aaf1f`.
