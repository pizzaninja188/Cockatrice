# Packet: 07-sac-creature-self-pump (4 cards)

Band: Simple single-primitive cohort, 4 cards.

Stable typed shape: Creature with `Sacrifice a creature: This creature gets +2/+2 until end of turn`.

| Oracle ID | Card | Mana | Type line | P/T | Oracle text |
|---|---|---|---|---|---|
| aa76cc50-5be0-4844-b3a4-fb33fca3cdd3 | Bloodthrone Vampire | {1}{B} | Creature — Vampire | 1/1 | Sacrifice a creature: This creature gets +2/+2 until end of turn. |
| 0dcefc00-9425-43e3-bd47-1e34fff8b0e2 | Nantuko Husk | {2}{B} | Creature — Zombie Insect | 2/2 | Sacrifice a creature: This creature gets +2/+2 until end of turn. |
| 748c2723-dc59-4aa7-9667-d4679448c8b6 | Phyrexian Ghoul | {2}{B} | Creature — Phyrexian Zombie | 2/2 | Sacrifice a creature: This creature gets +2/+2 until end of turn. |
| c14739b0-5c19-4873-b027-314225a10f7b | Vampire Aristocrat | {2}{B} | Creature — Vampire Rogue Noble | 2/2 | Sacrifice a creature: This creature gets +2/+2 until end of turn. |

## Copy-from and differences

Compose `frozen_shade`'s `PumpTarget(subject: Source)` with `vampiric_rites`' `SacrificePermanent(filter: (kind: Creature, controller: You))` cost; the only non-mana sacrifice-cost activation is the printed one.

## Rules evidence

CR 602.1a/602.2 (activated ability and cost), 701.21a (sacrifice as a cost), 611.2a (until-end-of-turn effect), and 514.2 (cleanup). The pinned rulings confirm that the source may sacrifice itself but then is not on the battlefield to receive the bonus.

## Canaries

Simplest: Bloodthrone Vampire. Hardest: Vampire Aristocrat.

## Admission

All cards are Commander-legal normal-layout cards from the pinned bulk; exact source records
and rulings are saved beside this packet. No candidate is blocked, excluded, already
implemented, partial or a duplicate-name presentation source.

The pinned bulk SHA-256 is
`a9a2fd3b550cc5cff4cec9bbc82aaf5fb8e3f781b4b95a896a0d94c15dce7853` and the pinned
rulings SHA-256 is `3ee07dbf91347f7940b4067ab1c01e0d5097da8f6c27ef620c202ed4262aaf1f`.
