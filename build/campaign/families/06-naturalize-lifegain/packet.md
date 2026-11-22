# Packet: 06-naturalize-lifegain (6 cards)

Band: Multi-clause composition, 6 cards.

Stable typed shape: Spell destroying a chosen artifact or enchantment and gaining a fixed amount of life.

| Oracle ID | Card | Mana | Type line | P/T | Oracle text |
|---|---|---|---|---|---|
| 05b5c0fb-84b2-4a32-bf66-179c4dccb3df | Natural End | {2}{G} | Instant |  | Destroy target artifact or enchantment. You gain 3 life. |
| 14945b80-10ec-4205-98df-cc3e119ca57c | Springsage Ritual | {3}{G} | Instant |  | Destroy target artifact or enchantment. You gain 4 life. |
| f041b37e-25af-4aff-b7e3-f07a4d5c6f9a | Appetite for the Unnatural | {2}{G} | Instant |  | Destroy target artifact or enchantment. You gain 2 life. |
| 87bd0fdb-c5b9-46ea-9858-1a870960351f | Invoke the Divine | {2}{W} | Instant |  | Destroy target artifact or enchantment. You gain 4 life. |
| c804ac8e-7427-43c6-b496-3ee6de192f03 | Sanctify | {1}{W} | Sorcery |  | Destroy target artifact or enchantment. You gain 3 life. |
| aff51c27-fb61-41b3-a7ea-7092cd280c63 | Solemn Offering | {2}{W} | Sorcery |  | Destroy target artifact or enchantment. You gain 4 life. |

## Copy-from and differences

`thorins_last_stand` mode 2 for the exact `Destroy(... permanent_types: [Artifact, Enchantment])` plus `GainLife` sequence; `disenchant` for the targeting shape.

Per-card differences: The life amount is 2, 3 or 4 and the colors differ; Sanctify and Solemn Offering are sorceries, the other four are instants.

## Rules evidence

CR 115.1 (target), 701.8 (destroy), 119.3 (life gain) and 608.2b (an illegal target makes the whole spell not resolve, so no life is gained). The Natural End, Springsage Ritual, Invoke the Divine and Solemn Offering rulings state the same fizzle rule; indestructible targets are still legal and still gain life.

## Canaries

- Simplest: Appetite for the Unnatural ({2}{G}, gain 2).
- Hardest: Solemn Offering ({2}{W} sorcery, gain 4).
- Both are exercised by a focused canary scenario; every card is executed by the all-card loops, including the exact-cost, illegal-target/fizzle, scope and identity checks that apply to the family.

## Admission

All cards are Commander-legal normal-layout cards from the pinned bulk; exact source records and rulings are saved beside this packet. No candidate is blocked, excluded, already implemented, partial or a duplicate-name presentation source.

The pinned bulk SHA-256 is
`a9a2fd3b550cc5cff4cec9bbc82aaf5fb8e3f781b4b95a896a0d94c15dce7853` and the pinned
rulings SHA-256 is `3ee07dbf91347f7940b4067ab1c01e0d5097da8f6c27ef620c202ed4262aaf1f`.
