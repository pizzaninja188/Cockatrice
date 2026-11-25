# 03-x-entry-counters — five creatures that enter with X +1/+1 counters

- Queue: `build/campaign/queue-v86`.
- Cards: 5.
- Band: simple single-primitive cohort (5).
- Status: **verified**; independent review verdict below.
- Pinned bulk: SHA-256 `a9a2fd3b550cc5cff4cec9bbc82aaf5fb8e3f781b4b95a896a0d94c15dce7853`.
- Pinned rulings: SHA-256 `3ee07dbf91347f7940b4067ab1c01e0d5097da8f6c27ef620c202ed4262aaf1f`.

## Typed shape and analogues

Every card's only non-keyword clause is "This creature enters with X +1/+1 counters on it." That is
`StaticAbilityDef::EntersWithCounters { counter: PlusOnePlusOne, amount: Amount::X }`, exactly
Endless One (`data/endless_one.ron`).

The family spans `{X}` through `{X}{G}{W}` and carries four different printed keyword sets, so it
proves the replacement independently of the body and keywords.

## Cards

| Card | Oracle ID | Cost | Type | P/T | Keyword(s) | RON | Map |
|---|---|---|---|---|---|---|---|
| Ivy Elemental | `38517711-e570-4337-b269-addcf8bfdd74` | `{X}{G}` | Creature — Elemental | 0/0 | — | `data/ivy_elemental.ron` | `review-maps/ivy_elemental.json` |
| Nimbus Swimmer | `ce02ef24-d53e-4025-bc45-ca27aba89fe9` | `{X}{G}{U}` | Creature — Leviathan | 0/0 | Flying | `data/nimbus_swimmer.ron` | `review-maps/nimbus_swimmer.json` |
| Shifting Wall | `7c852dfe-8238-461e-814e-9667807f2cf5` | `{X}` | Artifact Creature — Wall | 0/0 | Defender | `data/shifting_wall.ron` | `review-maps/shifting_wall.json` |
| Shivan Devastator | `b7daa74c-6142-4107-9355-be98af6ccf13` | `{X}{R}` | Creature — Dragon Hydra | 0/0 | Flying, haste | `data/shivan_devastator.ron` | `review-maps/shivan_devastator.json` |
| Worldsoul Colossus | `6970f285-dc2b-437b-b099-ca1d3ef106ab` | `{X}{G}{W}` | Creature — Elemental | 0/0 | Convoke | `data/worldsoul_colossus.ron` | `review-maps/worldsoul_colossus.json` |

## Pinned rulings

Four cards have none. Worldsoul Colossus carries the 2024-01-12 Convoke rulings; the one that bears
on this shape is "when using convoke to cast a spell with {X} in its mana cost, first choose the
value for X … then you can tap creatures you control to help pay that cost," so the counters equal
the chosen X and not the number of creatures tapped. Packets are under
`build/campaign/queue-v86/families/03-x-entry-counters/`.

## Scenario evidence

Module: `tricerules-core/tests/scenario/q86_batch_c.rs`. Registry coverage:
`tricerules-cards/tests/q86_batch_a.rs`.

- Canary (simplest): Ivy Elemental at X=3 enters 3/3 with three counters.
- Canary (hardest): Worldsoul Colossus cast with X=5 and three convoked creatures — the counters
  still equal 5, not 3, every convoked creature is tapped, the pool is fully spent, and the
  convoked creatures receive no counters (the replacement is self-scoped).
- X=1 and X=4 for every card produce exactly X counters and an X/X body.
- X=0 produces a 0/0 that dies as a state-based action (CR 704.5f) and reaches the graveyard.
- Every card keeps its printed keywords and only its printed keywords: Flying, Haste,
  Defender and Convoke are each asserted present or absent against the printed set.
- The printed colored pip is required: the all-colorless case supplies the exact generic total
  (X plus every pip), so it can only fail on the missing color, and a rejected cast spends nothing.
  Shifting Wall prints `{X}` with no pip, so it is asserted as castable with colorless mana.

## Presentation

`OracleLines([1])` selects the printed keyword line where one exists and the entry line where the
card has no keyword; `OracleLines([2])` selects the entry line for the four keyworded cards. Token
use: none.

## Focused evidence

- Intended red: `build/campaign/queue-v86/logs/red-v86.log`.
- Focused green (19 scenario tests):
  `build/verification-logs/20261125-122748-122-Rust-scenario-q86-batch-A-focused-green.log`.
- Registry green (4 tests):
  `build/verification-logs/20261125-122749-030-Rust-registry-q86-batch-A-focused.log`.
- Preflight: format, Clippy and card evidence all exit 0; `git diff --check` was run
  separately and also exited 0.
- Metadata refresh: `build/verification-logs/card-data-0e08cb09-bad8-4b78-b3a4-89af0501348e/`.

## Review and gate

- Independent review v1 `product-v1.patch` SHA-256
  `425239156e9d805f0cc01d06c653480038889d4f09c58f0524471e1b23352594`: CHANGES REQUIRED — one
  material finding, the review maps cited the other card's canary as covering each card; one
  material finding that the all-colorless rejection supplied three mana for the four-cost cards, so
  it could pass on quantity alone; and clerical findings on keyword-absence coverage, missing pool
  exhaustion after the convoke cast, a tautological post-mortem counter assertion, and the
  unasserted self-scope of the entry replacement. All corrected: per-card canary citations, exact
  generic totals (`2 + pips`), Defender/Convoke absence assertions, a `pool == 0` assertion, a
  `creatures_died == 1` death record, and zero counters asserted on every convoked payer.
- Independent review v2 `product-v2.patch` SHA-256
  `e0ccf155274b745432cdd3096320cdd2302dba66cdbfdd65b9bc5b8fdec67b31`: DELTA CHANGES REQUIRED on
  four evidence-accuracy items (one remaining code defect: the crew-1 below-threshold payer was
  still a 0/0; plus three record/brief mismatches).
- Independent review v3 `product-v3.patch` SHA-256
  `d65f5b97023fff0ee6a9f0737de9788ab68e54b19ee7e996c5649f7b0b056992`: DELTA APPROVED for all
  product semantics; the only residuals were the review brief naming the wrong frozen bundle and
  one attribution in this record, both corrected in evidence-only changes with no product byte
  difference.
- Batch queue-v86 batch A full gate `verify-91491640-d516-4a21-b0fd-fd7570b2673a` covers
  01-ally-entry-counter..03-x-entry-counters (16 cards), Status Pass, exit 0: environment, diff
  check, CardData, dependency boundary, all five package format checks, Clippy and full Rust tests
  all passed.
- Product commit `6160c9685`; `origin/card-campaign` verified at
  `6160c9685cfcb2d44d899279798b2f81c19d8d91`.

MTG rules surface: CR 107.3a-b (X is chosen as the spell is cast), 601.2f (the chosen X sets the
total cost), 614.1c/122.6 (the entry replacement adds the counters), 704.5f (zero toughness),
702.9 (flying), 702.10 (haste), 702.3 (defender), 702.51 (convoke).

Acceptance: rules-data only; no GUI surface (N/A).
