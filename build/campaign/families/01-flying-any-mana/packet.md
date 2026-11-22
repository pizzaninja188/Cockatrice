# Packet: 01-flying-any-mana (3 cards)

Band: Multi-clause composition, 3 cards.

Stable typed shape: Flying creature with a tap mana ability adding one mana of any color.

| Oracle ID | Card | Mana | Type line | P/T | Oracle text |
|---|---|---|---|---|---|
| d3a0b660-358c-41bd-9cd2-41fbf3491b1a | Birds of Paradise | {G} | Creature — Bird | 0/1 | Flying / {T}: Add one mana of any color. |
| 143d7a32-c15d-4a1f-ad81-cf1e6aa92bf3 | Lotus Guardian | {7} | Artifact Creature — Dragon | 4/4 | Flying / {T}: Add one mana of any color. |
| 3a940dfa-a026-4969-8981-ef90cdfbe9ef | Ornithopter of Paradise | {2} | Artifact Creature — Thopter | 0/2 | Flying / {T}: Add one mana of any color. |

## Copy-from and differences

`lifespring_druid` for the five-option `{T}` mana ability; `welkin_tern` and the flying cohort for the printed Flying keyword. The mana ability has no target and resolves immediately (CR 605.1a/605.3b); the creature still obeys summoning sickness for its `{T}` cost (CR 302.6).

Per-card differences: The three cards differ only in mana cost, body and type line; every one offers exactly the five colors and no other ability.

## Rules evidence

CR 702.9 (flying), 605.1a/605.3b (tap mana ability), 106.1a (five colors) and 302.6/602.5a (untapped, not summoning-sick). No card in this cohort has a pinned ruling.

## Canaries

- Simplest: Birds of Paradise ({G}, 0/1).
- Hardest: Lotus Guardian ({7}, 4/4 artifact creature).
- Both are exercised by a focused canary scenario; every card is executed by the all-card loops, including the exact-cost, illegal-target/fizzle, scope and identity checks that apply to the family.

## Admission

All cards are Commander-legal normal-layout cards from the pinned bulk; exact source records and rulings are saved beside this packet. No candidate is blocked, excluded, already implemented, partial or a duplicate-name presentation source.

The pinned bulk SHA-256 is
`a9a2fd3b550cc5cff4cec9bbc82aaf5fb8e3f781b4b95a896a0d94c15dce7853` and the pinned
rulings SHA-256 is `3ee07dbf91347f7940b4067ab1c01e0d5097da8f6c27ef620c202ed4262aaf1f`.
