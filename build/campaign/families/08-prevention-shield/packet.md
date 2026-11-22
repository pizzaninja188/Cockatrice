# Packet: 08-prevention-shield (8 cards)

Band: Simple single-primitive cohort, 8 cards.

Stable typed shape: Creature with a tap activated ability preventing the next N damage to any target.

| Oracle ID | Card | Mana | Type line | P/T | Oracle text |
|---|---|---|---|---|---|
| 4d979fbf-5009-47a1-8d77-b1ce80bbe90d | Sanctum Custodian | {2}{W} | Creature — Human Cleric | 1/2 | {T}: Prevent the next 2 damage that would be dealt to any target this turn. |
| 7695b61a-8d44-44b1-9783-ea3114111c3e | Master Healer | {4}{W} | Creature — Human Cleric | 1/4 | {T}: Prevent the next 4 damage that would be dealt to any target this turn. |
| 95a0ca48-d924-47f4-86ed-42c673ee778c | Samite Healer | {1}{W} | Creature — Human Cleric | 1/1 | {T}: Prevent the next 1 damage that would be dealt to any target this turn. |
| 9d405e5b-0cee-4c8f-b3ef-65765853b8a4 | Orim, Samite Healer | {1}{W}{W} | Legendary Creature — Human Cleric | 1/3 | {T}: Prevent the next 3 damage that would be dealt to any target this turn. |
| a5a1b4c9-474a-42b7-b412-c2d43a3353db | Clergy en-Vec | {1}{W} | Creature — Human Cleric | 1/1 | {T}: Prevent the next 1 damage that would be dealt to any target this turn. |
| c70dded8-658b-4250-8057-54894350455c | Loxodon Anchorite | {2}{W}{W} | Creature — Elephant Cleric | 2/3 | {T}: Prevent the next 2 damage that would be dealt to any target this turn. |
| e2117373-6f0a-41c8-ab6a-9014b9628e64 | Femeref Healer | {1}{W} | Creature — Human Cleric | 1/1 | {T}: Prevent the next 1 damage that would be dealt to any target this turn. |
| e3bd74f3-1f38-4162-a0be-c76ad63e2d9b | Kithkin Healer | {2}{W} | Creature — Kithkin Cleric | 2/2 | {T}: Prevent the next 1 damage that would be dealt to any target this turn. |

## Copy-from and differences

`healing_salve` mode 2 for `PreventNextDamage(amount, target: (kind: AnyTarget))`; the no-cost `{T}` activated shape comes from the tap-ability cohort. The effect is the same prevention shield used by the shipped spell.

Per-card differences: The prevention amount is 1, 2, 3 or 4; Orim is legendary and has a distinct type line. Every card is a vanilla body plus only this tap ability.

## Rules evidence

CR 614.1a/615 (prevention shields), 115.1 (any target), 602.2 and 302.6 (activating a tap ability). No card in this cohort has a pinned ruling.

## Canaries

- Simplest: Samite Healer ({1}{W}, 1/1, prevent 1).
- Hardest: Master Healer ({4}{W}, 1/4, prevent 4).
- Both are exercised by a focused canary scenario; every card is executed by the all-card loops, including the exact-cost, illegal-target/fizzle, scope and identity checks that apply to the family.

## Admission

All cards are Commander-legal normal-layout cards from the pinned bulk; exact source records and rulings are saved beside this packet. No candidate is blocked, excluded, already implemented, partial or a duplicate-name presentation source.

The pinned bulk SHA-256 is
`a9a2fd3b550cc5cff4cec9bbc82aaf5fb8e3f781b4b95a896a0d94c15dce7853` and the pinned
rulings SHA-256 is `3ee07dbf91347f7940b4067ab1c01e0d5097da8f6c27ef620c202ed4262aaf1f`.
