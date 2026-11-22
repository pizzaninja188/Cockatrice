# Packet: 07-becomes-target-sacrifice (5 cards)

Band: Simple single-primitive cohort, 5 cards.

Stable typed shape: Creature with `WheneverSelfBecomesTarget(source: SpellOrAbility, source_controller: AnyPlayer)` and `Sacrifice(subject: Source)`.

| Oracle ID | Card | Mana | Type line | P/T | Oracle text |
|---|---|---|---|---|---|
| 54a22814-c0d5-4637-9aa9-a6bc96401914 | Frost Walker | {1}{U} | Creature — Elemental | 4/1 | When this creature becomes the target of a spell or ability, sacrifice it. |
| 780da643-8d14-4f1a-a298-5b7454e9c4df | Phantasmal Bear | {U} | Creature — Bear Illusion | 2/2 | When this creature becomes the target of a spell or ability, sacrifice it. |
| 13ca7c14-5db2-4d17-912a-fdeabcdcd9f8 | Phantom Beast | {3}{U} | Creature — Illusion Beast | 4/5 | When this creature becomes the target of a spell or ability, sacrifice it. |
| 4178ae56-2449-4617-aff8-f59d8fb96c66 | Skulking Fugitive | {2}{B} | Creature — Horror Mercenary | 3/4 | When this creature becomes the target of a spell or ability, sacrifice it. |
| 05a7ca83-e820-433f-b9e9-151e817d3708 | Tar Pit Warrior | {2}{B} | Creature — Cyclops Warrior | 3/4 | When this creature becomes the target of a spell or ability, sacrifice it. |

## Copy-from and differences

The shipped Illusion token `illusion_u_2_2_spell_target_sacrifice` for the exact trigger/effect pair; the card bodies differ.

Oracle says "a spell or ability" with no controller restriction, so the trigger is `AnyPlayer`, unlike the shipped token's spell-only trigger.

## Rules evidence

CR 603.2 (trigger on becoming a target), CR 603.3b (the trigger resolves before the targeting spell or ability), CR 701.21a (sacrifice) and CR 608.2b (a spell or ability whose only target has left is countered on resolution). The pinned Frost Walker and Phantasmal Bear rulings restate this ordering.

## Canaries

Simplest: Phantasmal Bear (`{U}`, 2/2). Hardest: Phantom Beast (`{3}{U}`, 4/5) for the largest body.

## Admission

All cards are Commander-legal normal-layout cards from the pinned bulk; exact source records and rulings are saved beside this packet. No candidate is blocked, excluded, already implemented, partial or a duplicate-name presentation source.

The pinned bulk SHA-256 is
`a9a2fd3b550cc5cff4cec9bbc82aaf5fb8e3f781b4b95a896a0d94c15dce7853` and the pinned
rulings SHA-256 is `3ee07dbf91347f7940b4067ab1c01e0d5097da8f6c27ef620c202ed4262aaf1f`.
