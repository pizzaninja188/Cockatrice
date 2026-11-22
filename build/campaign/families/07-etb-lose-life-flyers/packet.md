# Packet: 07-etb-lose-life-flyers (4 cards)

Band: Multi-clause composition, 4 cards.

Stable typed shape: Flying creature with a mandatory entry trigger: its controller loses the printed amount of life.

| Oracle ID | Card | Mana | Type line | P/T | Oracle text |
|---|---|---|---|---|---|
| 25c04be3-f2e0-4f45-a1ca-9e75ffffa9b5 | Foul Imp | {B}{B} | Creature — Imp | 2/2 | Flying / When this creature enters, you lose 2 life. |
| 93f37df7-e1c4-457b-bc22-00c49b0c4f79 | Necrogen Scudder | {2}{B} | Creature — Phyrexian Horror | 3/3 | Flying / When this creature enters, you lose 3 life. |
| bd73ab86-0ac9-4ce0-be41-f4ad257e74f6 | Dread Reaper | {3}{B}{B}{B} | Creature — Horror | 6/5 | Flying / When this creature enters, you lose 5 life. |
| dbd1fc9c-6316-4c89-a971-f2e08ca291a4 | Vampiric Spirit | {2}{B}{B} | Creature — Spirit | 4/3 | Flying / When this creature enters, you lose 4 life. |

## Copy-from and differences

Copy `dusk_legion_zealot`'s `WhenSelfEntersBattlefield` + `LoseLife(amount: Fixed(N), who: Controller)` and add the printed flying keyword; only the printed amount and body differ.

## Rules evidence

CR 702.9a-b (flying), 603.6a (enters-the-battlefield trigger), 603.3d/608.2c (stack and resolution order), 119.3 (effect life loss adjusts the total), 120.3a (damage is a distinct life-loss cause) and 119.4 (paying life is a cost). Foul Imp's pinned ruling confirms the spell can be cast below the lost amount.

## Canaries

Simplest: Foul Imp (`foul_imp`). Hardest: Vampiric Spirit (`vampiric_spirit`).

## Admission

All cards are Commander-legal normal-layout cards from the pinned bulk; exact source records
and rulings are saved beside this packet. No candidate is blocked, excluded, already
implemented, partial or a duplicate-name presentation source.

The pinned bulk SHA-256 is
`a9a2fd3b550cc5cff4cec9bbc82aaf5fb8e3f781b4b95a896a0d94c15dce7853` and the pinned
rulings SHA-256 is `3ee07dbf91347f7940b4067ab1c01e0d5097da8f6c27ef620c202ed4262aaf1f`.
