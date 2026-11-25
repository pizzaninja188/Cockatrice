# 02-crew-vehicles — five Vehicles that are printed keywords plus Crew N

- Queue: `build/campaign/queue-v86`.
- Cards: 5.
- Band: simple single-primitive cohort (5).
- Status: **verified**; independent review verdict below.
- Pinned bulk: SHA-256 `a9a2fd3b550cc5cff4cec9bbc82aaf5fb8e3f781b4b95a896a0d94c15dce7853`.
- Pinned rulings: SHA-256 `3ee07dbf91347f7940b4067ab1c01e0d5097da8f6c27ef620c202ed4262aaf1f`.

## Typed shape and analogues

Every card is one or more printed keywords plus exactly one activated crew ability. Crew is the
reviewed

```
costs: [TapPermanents(
  constraint: AggregateMinimum(minimum: N, contribution: CurrentPower),
  filter: (kind: Creature, controller: You),
  exclude_source: true,
)]
effect: [AddTypes(subject: Source, addition: (card_types: [Creature]))]
```

already shipped on Consulate Dreadnought, Brute Suit, Dusk Legion Dreadnought, Futurist Sentinel
and Sleek Schooner (`vanilla_vehicles_batch`).

The crew threshold varies 1–4, so the family proves the printed value rather than a hard-coded one.

## Cards

| Card | Oracle ID | Cost | Type | P/T | Keywords | Crew | RON | Map |
|---|---|---|---|---|---|---|---|---|
| Aradara Express | `be96b9e1-10ef-4955-8d23-c39384def86d` | `{5}` | Artifact — Vehicle | 8/6 | Menace | 4 | `data/aradara_express.ron` | `review-maps/aradara_express.json` |
| Dragonfly Suit | `f242d66c-011c-4b5b-a7d6-5e0bce257f73` | `{2}{W}` | Artifact — Vehicle | 3/2 | Flying | 1 | `data/dragonfly_suit.ron` | `review-maps/dragonfly_suit.json` |
| Ovalchase Dragster | `345de988-4d26-4400-8202-225968db370f` | `{4}` | Artifact — Vehicle | 6/1 | Trample, haste | 1 | `data/ovalchase_dragster.ron` | `review-maps/ovalchase_dragster.json` |
| Sky Skiff | `f2c43b85-8c42-4b23-84c2-bec8d6230039` | `{2}` | Artifact — Vehicle | 2/3 | Flying | 1 | `data/sky_skiff.ron` | `review-maps/sky_skiff.json` |
| Thundersteel Colossus | `9cef3013-6a84-4edd-9a47-e842df2200e3` | `{7}` | Artifact — Vehicle | 7/7 | Trample, haste | 2 | `data/thundersteel_colossus.ron` | `review-maps/thundersteel_colossus.json` |

## Pinned rulings

Dragonfly Suit and Thundersteel Colossus have none. Aradara Express, Ovalchase Dragster and Sky
Skiff carry the 2017-09-29 Vehicle rulings. This family's suite asserts the ones that bear on the
typed shape: a Vehicle is not a creature until crewed, crew taps only your creatures and rejects
insufficient power, and the type change ends at cleanup. Two further pinned claims are asserted by
the shipped suites: `issue_215_crew.rs` shows a summoning-sick creature may crew, and
`vanilla_vehicles_batch` shows crew 6 paid with seven power (overpaying is legal). The remaining
claim — that becoming a
creature is not an enters-the-battlefield event — is not asserted by any shipped suite; it is
checked here only against the pinned 2017-09-29 rulings and the engine's `AddTypes` shape, which
performs no entry event. Packets are under
`build/campaign/queue-v86/families/02-crew-vehicles/`.

## Scenario evidence

Module: `tricerules-core/tests/scenario/q86_batch_b.rs`. Registry coverage:
`tricerules-cards/tests/q86_batch_a.rs`.

- Canary (simplest): Sky Skiff — cast as a noncreature artifact, crewed into the printed 2/3 with
  Flying, payer tapped, the Vehicle itself untapped.
- Canary (hardest): Aradara Express — three power is rejected for crew 4, then exactly four
  power (3 + 1) crews into the printed 8/6 with Menace.
- Every card crews into its exact printed body and keywords.
- Insufficient power is rejected and pays nothing; an opponent's creature can never pay your
  crew. The one-power case uses a legal 0-power 0/2 body, never a 0/0 that would die first.
- The type change ends at cleanup (CR 514.2).
- Every card's noncreature artifact characteristics are asserted before crew.

## Presentation

`OracleLines([1])` selects the printed keyword line and `OracleLines([2])` the crew line. Token
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
  `425239156e9d805f0cc01d06c653480038889d4f09c58f0524471e1b23352594`: CHANGES REQUIRED — four
  material findings: the review maps cited the other Vehicle's canary as covering each card; this
  record overstated which pinned rulings the shipped analogue asserts; the crew-4 payer totalled
  five power so a Crew 5 authoring error would have passed; and the below-threshold payer for the
  crew-1 cards was an illegal 0/0. Corrected respectively by per-card canary citations, restating
  the ruling reuse honestly, exact-threshold payers (3 + 1 in the canary, 2 + 2 in the loop), and
  an Ornithopter 0/2 payer. Added `addition.creature_types`/`land_types` absence guards and CR
  208.3/301.7a to the five RON headers.
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

MTG rules surface: CR 301.7 (Vehicle), 702.122a-e (crew), 208.3 (printed body only while a
creature), 611.2a/514.2 (until end of turn), 702.111 (menace), 702.9 (flying), 702.19 (trample),
702.10 (haste).

Acceptance: rules-data only; no GUI surface (N/A).
