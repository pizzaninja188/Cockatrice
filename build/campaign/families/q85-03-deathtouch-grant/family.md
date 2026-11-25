# 03-deathtouch-grant — {B} deathtouch grant plus one printed keyword or CDA

- Queue: `build/campaign/queue-v85`.
- Cards: 3.
- Band: multi-clause (3–5).
- Status: **verified**; independent review v1 CHANGES REQUIRED (the shared findings), v2 delta
  APPROVED; batch gate pass.
- Pinned bulk: SHA-256 `a9a2fd3b550cc5cff4cec9bbc82aaf5fb8e3f781b4b95a896a0d94c15dce7853`.
- Pinned rulings: SHA-256 `3ee07dbf91347f7940b4067ab1c01e0d5097da8f6c27ef620c202ed4262aaf1f`.

## Typed shape and analogues

One printed Keyword (or the Changeling characteristic-defining ability) plus
`activated_abilities: [{ costs: [Mana("{B}")], effect: [GrantKeywords(subject: Source, keywords:
[Deathtouch])] }]`. Analogue: Pestilent Wolf and its `q37_self_keyword_activated_batch` suite for the
grant; Sizzling Changeling for the Changeling CDA.

## Cards

| Card | Oracle ID | Cost | Type | P/T | Printed | RON | Map |
|---|---|---|---|---|---|---|---|
| Battlefly Swarm | `1396e7f1-ac91-4778-9f71-493c7a44bd51` | `{B}` | Creature — Phyrexian Insect | 1/1 | Flying | `data/battlefly_swarm.ron` | `review-maps/battlefly_swarm.json` |
| Fetid Imp | `cdb68656-633d-436e-ad56-0fdb4814bd19` | `{1}{B}` | Creature — Imp | 1/2 | Flying | `data/fetid_imp.ron` | `review-maps/fetid_imp.json` |
| Moonglove Changeling | `61b74f82-39f3-4618-b8e7-292058421d1b` | `{2}{B}` | Creature — Shapeshifter | 2/2 | Changeling | `data/moonglove_changeling.ron` | `review-maps/moonglove_changeling.json` |

No card has a pinned ruling.

## Scenario evidence

Module: `tricerules-core/tests/scenario/q85_batch_a.rs`. Registry coverage:
`tricerules-cards/tests/q85_batch_a.rs`.

- Canary (simplest): Battlefly Swarm has Flying but no deathtouch before activation, then the exact
  `{B}` grant applies.
- Source-only scope: each card grants deathtouch to itself and not to another creature, and its
  printed keyword is unaffected.
- Cleanup: the grant expires at end of turn.
- Illegal path: activation with no black mana is rejected and changes no state; the same activation
  then succeeds once the printed mana is available, so the rejection proves the cost rather than an
  absent definition.
- Printed characteristics: body, printed keyword or CDA, and the fact that deathtouch is granted
  rather than printed are all asserted. Moonglove Changeling's Test additionally asserts CR
  702.73/205.3m: it has every creature type.

## Presentation

`OracleLines([1])` selects the printed keyword or Changeling line; `OracleLines([2])` selects the
`{B}` grant line. Token use: none.

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
  CHANGES REQUIRED. This family's findings were the shared clerical ones: its "cast" test never
  cast, so it was renamed to `q85_deathtouch_cards_carry_their_exact_printed_characteristics`, with
  every map reference updated.
- v2 `product-v2.patch` SHA-256 `964aeb7a35d36f7957e517e29dc6245a618786af37e64f64720a9362468a9acd`:
  delta APPROVED.
- Batch queue-v85 batch A full gate covers 01-prevention-shield-clerics..03-deathtouch-grant
  (10 cards).

MTG rules surface: CR 702.73/205.3m (changeling is every creature type), 602.1/602.2 (activation and
printed cost), 611.2a (grant ends at cleanup), 514.2, 702.2 (deathtouch), and 702.9 (flying).

Acceptance: rules-data only; no GUI surface (N/A).
