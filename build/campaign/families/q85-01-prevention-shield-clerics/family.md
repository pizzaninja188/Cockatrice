# 01-prevention-shield-clerics — tap prevention shield plus one printed keyword

- Queue: `build/campaign/queue-v85`.
- Cards: 4.
- Band: multi-clause (3–5).
- Status: **verified**; independent review v1 CHANGES REQUIRED (seven findings), v2 delta APPROVED;
  batch gate pass.
- Pinned bulk: SHA-256 `a9a2fd3b550cc5cff4cec9bbc82aaf5fb8e3f781b4b95a896a0d94c15dce7853`.
- Pinned rulings: SHA-256 `3ee07dbf91347f7940b4067ab1c01e0d5097da8f6c27ef620c202ed4262aaf1f`.

## Typed shape and analogues

One printed Keyword plus `activated_abilities: [{ costs: [Tap], effect: [PreventNextDamage(amount,
target: (kind: AnyTarget))] }]`. Analogue for the shield: Femeref Healer and Master Healer, whose
`prevention_shield_batch` suite already asserts shield placement, absorption, rejection, and cleanup.
The printed keyword differs per card and is the only variation.

## Cards

| Card | Oracle ID | Cost | Type | P/T | Keyword | Shield | RON | Map |
|---|---|---|---|---|---|---|---|---|
| Militant Monk | `29c0a904-43e3-44c3-8e00-deea523de8eb` | `{1}{W}{W}` | Creature — Human Monk Cleric | 2/1 | Vigilance | 1 | `data/militant_monk.ron` | `review-maps/militant_monk.json` |
| Benevolent Ancestor | `ee414d20-b18e-4bed-97fc-3c7535089ec1` | `{2}{W}` | Creature — Spirit | 0/4 | Defender | 1 | `data/benevolent_ancestor.ron` | `review-maps/benevolent_ancestor.json` |
| Alabaster Wall | `f9f52651-cc64-48c5-bc18-cd5f646dfb1f` | `{2}{W}` | Creature — Wall | 0/4 | Defender | 1 | `data/alabaster_wall.ron` | `review-maps/alabaster_wall.json` |
| Aven Redeemer | `faac29f5-10ad-4800-a9d3-2259c4800352` | `{3}{W}` | Creature — Bird Cleric | 2/2 | Flying | 2 | `data/aven_redeemer.ron` | `review-maps/aven_redeemer.json` |

Pinned 2017-11-17 Benevolent Ancestor ruling: if the shielded target would be dealt damage by multiple
sources at once, that player or that permanent's controller chooses one of those sources at the time
damage would be dealt, and 1 damage from that source is prevented. The ruling describes the choice a
player makes inside a single prevention event; it does not scale the shield or fire once per source.

## Scenario evidence

Module: `tricerules-core/tests/scenario/q85_batch_a.rs`, registered in `tests/scenario.rs`. Registry
coverage: `tricerules-cards/tests/q85_batch_a.rs`, registered in `tests/suites/mechanic_registry.rs`.

- Canary (simplest): Militant Monk's Vigilance plus an exact one-point player shield, consumed by a
  Lightning Bolt that then deals exactly two.
- Exact amounts: each card shields a creature for its printed amount against a Shock, and the test
  asserts the resulting damage and the residual shield. Aven Redeemer is the only two-point member, so
  a hard-coded one-point shield fails.
- Illegal paths: activation with no target is rejected and pays nothing; a second activation on a
  tapped source is rejected with the absolute printed shield still in place.
- Stale target: the any-target creature is destroyed while the shield ability is on the stack, so the
  ability resolves with a stale target and places no shield.
- Summoning sickness: each card cast from hand is summoning sick and cannot pay `{T}`.
- Real cast: each card's printed cost, body and keyword are asserted after a cast from hand.
- Cleanup: every shield expires at end of turn.

## Presentation

Each printed line maps to its keyword list and its single activated ability; `OracleLines([1])` and
`OracleLines([2])` select the keyword line and the activation line respectively. Token use: none.

## Focused evidence

- Intended red on the final test bytes with all ten RONs absent:
  `build/campaign/queue-v85/logs/red-v2.log` (18 failed, 0 passed).
- Focused green (19 scenario tests):
  `build/verification-logs/20261125-100724-984-Rust-scenario-q85-batch-A-focused-green.log`.
- Registry green (4 tests):
  `build/verification-logs/20261125-100809-326-Rust-registry-q85-batch-A-focused-green.log`.
- Preflight: format core and cards exit 0, Clippy exit 0, card evidence exit 0,
  `git diff --check` exit 0.

## Review and gate

- v1 bundle `build/campaign/queue-v85/review/product-v1.patch`, SHA-256
  `8464df1c2a2e42cd0d9fc2f0fafcc406e9482478dd5be4e914deb0ec97fccba1`: CHANGES REQUIRED. One material
  finding (Flowstone Wall's claimed survival branch was unreachable in the pump family, an evidence
  overclaim rather than a card error) and six clerical findings (Aven Redeemer cited the wrong
  analogue amount; two tests were named `_cast_` without casting; shield headers cited CR 614.1a/615.1
  and 115.1 instead of 615.1a/615.7 and 115.4; map wording said "copy" and claimed a positive
  residual shield; no stale-target path; a rejection test compared against a stored value).
- v2 bundle `build/campaign/queue-v85/review/product-v2.patch`, SHA-256
  `964aeb7a35d36f7957e517e29dc6245a618786af37e64f64720a9362468a9acd`: all seven findings resolved,
  delta APPROVED, no new problems.
- Batch queue-v85 batch A full gate covers 01-prevention-shield-clerics..03-deathtouch-grant
  (10 cards).

MTG rules surface: CR 615.1a/615.7 (fixed-amount prevention shields and the multi-source chooser),
115.4 ("any target"), 602.2/302.6 (tap ability and summoning sickness), 514.2 (cleanup expiry),
702.20 (vigilance), 702.3 (defender), 702.9 (flying), and 608.2b (stale target fizzles).

Acceptance: rules-data only; no GUI surface (N/A).
