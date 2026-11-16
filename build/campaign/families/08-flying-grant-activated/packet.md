# Packet: 08-flying-grant-activated (5 cards)

Band: Parameter family, 5 cards.

Stable typed shape: Creature with one mana-only activated ability granting itself flying until end of turn.

| Oracle ID | Card | Mana | Type line | P/T | Oracle text |
|---|---|---|---|---|---|
| 1e50af57-e49d-4b94-a370-44846d9f6b33 | Cobalt Golem | {4} | Artifact Creature — Golem | 2/3 | {1}{U}: This creature gains flying until end of turn. |
| 298a71d4-6dc1-444e-9057-0d25aef7fb64 | Kor Sky Climber | {2}{W} | Creature — Kor Soldier Ally | 3/2 | {1}{W}: This creature gains flying until end of turn. |
| 1b5c2ba0-3df7-4394-a3f1-8e8330d7ac81 | Leaping Master | {1}{R} | Creature — Human Monk | 2/1 | {2}{W}: This creature gains flying until end of turn. |
| c2f2fd39-4a21-489b-893e-4258f8d2aa63 | Roofstalker Wight | {1}{B} | Creature — Zombie | 2/1 | {1}{U}: This creature gains flying until end of turn. |
| f53f33ed-5c8c-4835-bc5f-65a8acfa3f5b | Steeple Creeper | {2}{G} | Creature — Frog Snake | 4/2 | {3}{U}: This creature gains flying until end of turn. |

## Copy-from and differences

`dukhara_peafowl` and the reviewed `blue_gain_flying_batch` cohort for `GrantKeywords(subject: Source, keywords: [Flying])` on a mana-cost ability.

The activation cost and the printed body differ per card, including two non-blue activation colors; the grant is always exactly flying.

## Rules evidence

CR 602.1-2 (activated ability and cost), CR 702.9a-b (flying evasion), CR 611.2a (until-end-of-turn grant), CR 509.1b/509.1h (a grant after blocking does not undo the block) and CR 514.2 (cleanup ends it). The pinned Leaping Master ruling restates the post-block case.

## Canaries

Simplest: Cobalt Golem (`{4}` 2/3; `{1}{U}`). Hardest: Steeple Creeper (`{2}{G}` 4/2; `{3}{U}`) for the largest activation cost.

## Admission

All cards are Commander-legal normal-layout cards from the pinned bulk; exact source records and rulings are saved beside this packet. No candidate is blocked, excluded, already implemented, partial or a duplicate-name presentation source.

The pinned bulk SHA-256 is
`a9a2fd3b550cc5cff4cec9bbc82aaf5fb8e3f781b4b95a896a0d94c15dce7853` and the pinned
rulings SHA-256 is `3ee07dbf91347f7940b4067ab1c01e0d5097da8f6c27ef620c202ed4262aaf1f`.
