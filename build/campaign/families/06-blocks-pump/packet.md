# Packet: 06-blocks-pump (5 cards)

Band: Simple single-primitive cohort, 5 cards.

Stable typed shape: Creature with `WheneverSelfBlocksCreature(attacker: ())` and `PumpTarget(subject: Source)` until end of turn.

| Oracle ID | Card | Mana | Type line | P/T | Oracle text |
|---|---|---|---|---|---|
| 86002526-ed3d-47e5-9f2a-55c4fa220ce8 | Folk of An-Havva | {G} | Creature — Human | 1/1 | Whenever this creature blocks, it gets +2/+0 until end of turn. |
| a4109138-5d69-4510-a228-7c1f88c0c8c0 | Giant Badger | {1}{G}{G} | Creature — Badger | 2/2 | Whenever this creature blocks, it gets +2/+2 until end of turn. |
| e31510d5-b74e-4a8d-94e8-8bd098cac3c7 | Royal Trooper | {2}{W} | Creature — Human Soldier | 2/2 | Whenever this creature blocks, it gets +2/+2 until end of turn. |
| 0b6625cb-4177-45bd-ba99-2af43dfd6f46 | Shu Defender | {2}{W} | Creature — Human Soldier | 2/2 | Whenever this creature blocks, it gets +0/+2 until end of turn. |
| ec1ac4df-b031-43b4-a2de-ca58a1599053 | Town Sentry | {2}{W} | Creature — Human Soldier | 2/2 | Whenever this creature blocks, it gets +0/+2 until end of turn. |

## Copy-from and differences

`ezuris_archers` for the block trigger and self pump (it filters the attacker to flying) and `high-rise_sawjack` for the same reviewed trigger shape.

Two cards pump +2/+0, two pump +0/+2 and one pumps +2/+2. The attacker filter is empty because the Oracle line has no attacker restriction.

## Rules evidence

CR 509.3b (one occurrence per attacker-blocker edge; each card can block one attacker), CR 611.2a and 514.2 (until-end-of-turn duration) and CR 613.4c (layer 7c pump of the source). No card in this cohort has a pinned ruling.

## Canaries

Simplest: Folk of An-Havva (`{G}`, +2/+0). Hardest: Giant Badger (`{1}{G}{G}`, +2/+2).

## Admission

All cards are Commander-legal normal-layout cards from the pinned bulk; exact source records and rulings are saved beside this packet. No candidate is blocked, excluded, already implemented, partial or a duplicate-name presentation source.

The pinned bulk SHA-256 is
`a9a2fd3b550cc5cff4cec9bbc82aaf5fb8e3f781b4b95a896a0d94c15dce7853` and the pinned
rulings SHA-256 is `3ee07dbf91347f7940b4067ab1c01e0d5097da8f6c27ef620c202ed4262aaf1f`.
