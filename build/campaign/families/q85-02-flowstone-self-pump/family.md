# 02-flowstone-self-pump — Flowstone {R}: +1/-1 self pump plus one printed keyword

- Queue: `build/campaign/queue-v85`.
- Cards: 3.
- Band: multi-clause (3–5).
- Status: **verified**; independent review v1 CHANGES REQUIRED (the shared material finding about the
  zero-toughness evidence claim), v2 delta APPROVED; batch gate pass.
- Pinned bulk: SHA-256 `a9a2fd3b550cc5cff4cec9bbc82aaf5fb8e3f781b4b95a896a0d94c15dce7853`.
- Pinned rulings: SHA-256 `3ee07dbf91347f7940b4067ab1c01e0d5097da8f6c27ef620c202ed4262aaf1f`.

## Typed shape and analogues

One printed Keyword plus `activated_abilities: [{ costs: [Mana("{R}")], effect: [PumpTarget(power: 1,
toughness: -1, subject: Source)] }]`. Analogue: Flowstone Shambler, whose
`self_pump_negative_batch` suite already asserts the printed pump, source-only scope, stacking,
zero-toughness death, cleanup expiry, and missing-mana rejection.

## Cards

| Card | Oracle ID | Cost | Type | P/T | Keyword | RON | Map |
|---|---|---|---|---|---|---|---|
| Flowstone Kavu | `49475268-be95-4670-a5ca-2f2b6c5cf005` | `{2}{R}` | Creature — Kavu | 2/3 | Menace | `data/flowstone_kavu.ron` | `review-maps/flowstone_kavu.json` |
| Flowstone Wall | `5fa86b45-1d5b-42df-8811-6a10903f3039` | `{2}{R}` | Creature — Wall | 0/6 | Defender | `data/flowstone_wall.ron` | `review-maps/flowstone_wall.json` |
| Flowstone Mauler | `ea205569-b5a0-4c86-b825-67cd48d3b2e8` | `{4}{R}{R}` | Creature — Beast | 4/5 | Trample | `data/flowstone_mauler.ron` | `review-maps/flowstone_mauler.json` |

No card has a pinned ruling.

## Scenario evidence

Module: `tricerules-core/tests/scenario/q85_batch_a.rs`. Registry coverage:
`tricerules-cards/tests/q85_batch_a.rs`.

- Canary (simplest): Flowstone Kavu's printed Menace plus the exact +1/-1 self pump.
- Source-only scope: each card's opponent creature keeps its printed body while the source changes.
- Zero toughness: for each card the printed toughness sets the exact activation count. Flowstone Kavu
  (3) is alive at 2 and 1 then dies; Flowstone Wall (6) is alive at 5, 4, 3, 2 and 1 then dies on the
  sixth; Flowstone Mauler (5) is alive at 4, 3, 2 and 1 then dies on the fifth. CR 704.5f.
- Cleanup: the pump expires and the printed body returns.
- Illegal path: activation with no mana is rejected, changes no state, and applies no pump.
- Printed characteristics: body and keyword are asserted for each card.

## Presentation

`OracleLines([1])` selects the keyword line; `OracleLines([2])` selects the `{R}` pump line. Token use:
none.

## Focused evidence

- Intended red: `build/campaign/queue-v85/logs/red-v2.log` (18 failed, 0 passed with all ten RONs
  absent).
- Focused green (19 scenario tests):
  `build/verification-logs/20261125-100724-984-Rust-scenario-q85-batch-A-focused-green.log`.
- Registry green (4 tests):
  `build/verification-logs/20261125-100809-326-Rust-registry-q85-batch-A-focused-green.log`.
- Preflight: format, Clippy, card evidence and `git diff --check` all exit 0.

## Review and gate

- v1 `product-v1.patch` SHA-256 `8464df1c2a2e42cd0d9fc2f0fafcc406e9482478dd5be4e914deb0ec97fccba1`:
  CHANGES REQUIRED. The material finding was that this family's zero-toughness test claimed a
  survival branch that its own loop made unreachable; the test and the covers text were corrected so
  every positive toughness and the lethal activation are now asserted.
- v2 `product-v2.patch` SHA-256 `964aeb7a35d36f7957e517e29dc6245a618786af37e64f64720a9362468a9acd`:
  delta APPROVED.
- Batch queue-v85 batch A full gate covers 01-prevention-shield-clerics..03-deathtouch-grant
  (10 cards).

MTG rules surface: CR 602.1/602.2 (activation and printed cost), 611.2a/613.4c (until-end-of-turn
P/T modification), 704.5f (zero toughness), 514.2 (cleanup expiry), 702.111 (menace), 702.3
(defender), 702.19 (trample).

Acceptance: rules-data only; no GUI surface (N/A).
