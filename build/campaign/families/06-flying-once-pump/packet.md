# Packet: 06-flying-once-pump (4 cards)

Band: Multi-clause composition, 4 cards.

Stable typed shape: Flying creature with `{M}: This creature gets +1/+0 until end of turn. Activate only once each turn.`

| Oracle ID | Card | Mana | Type line | P/T | Oracle text |
|---|---|---|---|---|---|
| a82d9c9b-ec17-4f51-a6bf-5984ca4433e5 | Azimaet Drake | {2}{U} | Creature — Drake | 1/3 | Flying / {U}: This creature gets +1/+0 until end of turn. Activate only once each turn. |
| bd55bcc4-f0c5-4c09-8b3b-2e25e9fbd9ee | Drake Hatchling | {2}{U} | Creature — Drake | 1/3 | Flying / {U}: This creature gets +1/+0 until end of turn. Activate only once each turn. |
| bfbc12ec-ecc0-4e23-a9d1-d13babd99a46 | Spitting Drake | {3}{R} | Creature — Drake | 2/2 | Flying / {R}: This creature gets +1/+0 until end of turn. Activate only once each turn. |
| fd3bcc9b-7d84-478e-aef5-2e44610107c7 | Fire Drake | {1}{R}{R} | Creature — Drake | 1/2 | Flying / {R}: This creature gets +1/+0 until end of turn. Activate only once each turn. |

## Copy-from and differences

Copy `darkthicket_wolf`'s `PumpTarget(subject: Source)` with `ActivationLimit::PerTurn`; add the printed flying keyword.

## Rules evidence

CR 702.9a-b (flying), 602.1a/602.2 (activated ability and printed cost), 602.5b (once each turn), 611.2c/613.4c (pump) and 514.2 (cleanup). The cycle has no pinned ruling.

## Canaries

Simplest: Azimaet Drake (`azimaet_drake`). Hardest: Fire Drake (`fire_drake`).

## Admission

All cards are Commander-legal normal-layout cards from the pinned bulk; exact source records
and rulings are saved beside this packet. No candidate is blocked, excluded, already
implemented, partial or a duplicate-name presentation source.

The pinned bulk SHA-256 is
`a9a2fd3b550cc5cff4cec9bbc82aaf5fb8e3f781b4b95a896a0d94c15dce7853` and the pinned
rulings SHA-256 is `3ee07dbf91347f7940b4067ab1c01e0d5097da8f6c27ef620c202ed4262aaf1f`.
