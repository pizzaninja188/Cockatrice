# 01-turbulent-dual-lands — the three remaining Turbulent duals

- Queue: `build/campaign/queue-v89`.
- Cards: 3.
- Band: simple single-primitive cohort (3).
- Status: **verified**; independent read-only review approved the reviewed semantics and the
  queue-v89 consolidated batch gate passed exit 0.
- Pinned bulk: SHA-256 `1cee09ee1ac70977c46171f4abf5cdfe170ab43bc32232af9138404e27524ce6`.
- Pinned rulings: SHA-256 `84f4c793e6d4f1256206b915da007111e1a445fe7cad99147802086454e8a95d`.

## Typed shape and analogue

Each card prints `({T}: Add {X} or {Y}.)` on line 1 and
`This land enters tapped unless your opponents control eight or more lands.` on line 2. That is one
intrinsic land mana ability (`intrinsic_land_mana: true`, `costs: [Tap]`, two-entry
`ProduceMana(options: …)`) plus one `StaticAbilityDef::EntersTapped` whose `condition` is
`GameCondition::BattlefieldAggregate { filter: (controllers: Opponents, matching: Land),
aggregate: Count, max: Some(7) }` — byte-equivalent to the shipped Turbulent Fen, Moor, Springs,
Steppe and Wilderness definitions (`q31_turbulent_lands_batch`).

`max: Some(7)` is the encoded form of "eight or more": the condition holds while the opponents'
land count is at most seven.

This is *not* the blocked `#519` "enters tapped unless you have two or more opponents" condition.
That one reads a live player count, which ships only as the dedicated
`PlayerCount(players: Opponents, max: Some(1))` leaf. This one reads a battlefield aggregate of
lands, which ships on five cards and is asserted by a shipped test. They are different typed
leaves, so this family adds no support for the blocked condition.

## Cards

| Card | Oracle ID | Type | Mana pair | Activation line | RON | Map |
|---|---|---|---|---|---|---|
| Turbulent Crater | `e86a0b64-fe4a-4ea8-99e8-35860dbac765` | Land — Swamp Mountain | `{B}` / `{R}` | 1 | `data/turbulent_crater.ron` | `review-maps/turbulent_crater.json` |
| Turbulent Shore | `c13c5f07-dac5-47d0-a088-e24b9fbec3c5` | Land — Plains Island | `{W}` / `{U}` | 1 | `data/turbulent_shore.ron` | `review-maps/turbulent_shore.json` |
| Turbulent Wetlands | `b51c8659-ec4f-4213-beb1-49a7acd7366c` | Land — Island Swamp | `{U}` / `{B}` | 1 | `data/turbulent_wetlands.ron` | `review-maps/turbulent_wetlands.json` |

The printed pair order is the mana option order: Crater black then red, Shore white then blue,
Wetlands blue then black. Wetlands is deliberately not the same shape as its shipped neighbour
Turbulent Moor (`Plains Swamp`, white then black), so a copy-pasted option list is visible.

## Pinned rulings

All three carry the same two 2026-08-21 rulings:

1. The land has two basic land types and is not basic, so basic-land searches cannot find it, but
   effects that name a basic land type (Dragonskull Summit, Glacial Fortress, Drowned Catacomb) do
   see it.
2. If it enters at the same time as any number of lands an opponent controls, those lands are not
   counted for its own entry replacement.

Ruling 1 is the stored type line: the two basic land types are in `types`, and the absence of the
`Basic` supertype is asserted by the registry test. Ruling 2 (lands entering simultaneously are not
counted) is the `614.1d/614.12` "check before it enters" behaviour that the shipped replacement
layer tests assert directly
(`conditional_slow_land_ignores_lands_entering_in_the_same_batch`,
`conditional_fast_land_ignores_lands_entering_in_the_same_batch` in
`tricerules-core/src/engine/replacement/entry_layers_tests.rs`). The eight-or-more boundary is printed
Oracle text, not a ruling.

Packets: `build/campaign/queue-v89/families/01-turbulent-dual-lands/packets/`.

## Canaries

- Simplest: Turbulent Shore — the allied `{W}{U}` pair. Proves the conditional entry (seven
  opponent lands tapped, eight untapped) and one printed color end to end.
- Hardest: Turbulent Wetlands — `{U}{B}`. It is the pair whose shipped neighbour is a *different*
  combination, so a copied `ProduceMana` option list fails, and it also proves a tapped land cannot
  produce a second mana.

Turbulent Crater is the replicated remnant; it shares the exact shape and is covered by the shared
per-card loop plus its own fingerprint.

## Semantic evidence

- `scenario q89_batch_a::q89_turbulent_land_canary_simplest_checks_the_live_opponent_land_count`
- `scenario q89_batch_a::q89_turbulent_land_canary_ub_offers_both_printed_colors_once`
- `scenario q89_batch_a::q89_turbulent_lands_apply_their_printed_colors_and_entry_replacement`
- `scenario q89_batch_a::q89_turbulent_lands_count_only_the_opponents_lands`
- `mechanic_registry::q89_batch_a::q89_turbulent_dual_lands_map_definitions`
- `mechanic_registry::q89_batch_a::q89_batch_a_fingerprints_match_the_pinned_oracle_text`

The registry test additionally asserts the `Basic` supertype is absent, which is the pinned
rulings' "not basic" consequence. The shared loop asserts, per card and per printed option: the
exact produced color, that the land taps, that the mana ability uses no stack, that life is
unchanged, and that the entry replacement turns on at seven opponent lands and off at eight. The second shared test asserts the condition
counts only opponents: eight lands the controller owns leave it on, and six own plus seven opponent
lands leave it on.

## Independent review

Round 1 (bundle `7dbe68e72b312e5b3c548c15cac4c9711d44d19b40246cda37ed4d4f4784f661`), two
independent read-only reviewers, both CHANGES REQUIRED. Material findings, all fixed: the
Ghor-Clan review map was named with a collapsed stem instead of the RON stem (which keeps the
printed hyphen) so the routine map check would have failed; CR 702.34 was cited for madness (that
is Flashback; madness is 702.35a-c); CR 305.8 was glossed as the two-type mana rule (that is
305.6; 305.8 is basic/nonbasic status); CR 118.12 was cited for the mana component of an
activation cost (it governs resolution-time costs; activation payment is 602.1a/602.2,
118.3/118.3a, 601.2h/602.2b, and 115.1c makes a cost not a target); the pinned control-change
ruling was untested; the "not basic" consequence was unasserted; and the records claimed a gate,
a mutation probe and a remote SHA that did not exist, while the red logs predated the frozen
tests.

Round 2 delta review (bundle `8463959d846de39f5755bcc6d675b46f4af37998ec726b72bc261ba91205ae2f`)
confirmed the functional, CR, control-change, not-basic and provenance findings resolved and left
five residual document/evidence items, all fixed: the family-02 map cell, the `selection.md` counts
and CR, the family-03 rider wording, the `revalidation.tsv` stem path plus a faithful
`prepare.py` slugify mirror, and per-mutation probe logs replacing one combined log whose later
failures were `once_cell` poisoning rather than independent detections.

Round 3 delta review (bundle
`6c7e0dda33f99a4bc2a02e14ea5a672af259fa1ddd2d6346bde674d94c8fa62c`): **APPROVED for reviewed
semantics**, no blocking defects. It also showed that the Viashino `{B}{G}` mutation was not
caught by the scenario test (black plus green can also pay one generic plus one green), so the
hardest canary gained a discriminating one-generic-plus-one-green step; that fix and the remaining
record corrections (115.1c, clearing the whole pool, accurate Crater probe wording) are in the
current bytes and covered by the final green logs.

## Focused evidence

- Intended red on the frozen test bytes, with all ten q89 RONs held out of `data/`:
  `build/campaign/queue-v89/logs/red-q89-registry.log` (4 failed, 0 passed) and
  `red-q89-scenario.log` (13 failed, 0 passed).
- Registry green: `build/campaign/queue-v89/logs/green-q89-registry.log` (4 passed).
- Scenario green: `build/campaign/queue-v89/logs/green-q89-scenario.log` (13 passed).
- Non-vacuity: changing Turbulent Crater's second mana option from red to green is rejected by the
  definition validator (`InvalidCard: intrinsic land mana must be the unrestricted tap-only
  bundle`), and every turbulent scenario fails as a result
  (`logs/probe-crater-color.log`, 0 passed / 4 failed, exit 101). The bytes were restored and
  re-verified against the draft.

## Conformance and scope

Pure data plus tests, maps and metadata. No engine change. Affected side: Rust and CardData.

## Final gate

PASS, exit 0. Queue-v89 consolidated batch gate `verify-0133872a-7d0a-435b-acd5-478a839abc33` on the reviewed tree, covering all
three families (10 cards): verification environment, `git diff --check`, CardData, the card-data
dependency boundary, all five package format checks, Clippy and the full Rust suite, with all 17
q89 tests observed passing.
Summary: `build/verification-logs/verify-0133872a-7d0a-435b-acd5-478a839abc33/summary.json`.
Recorded in `build/campaign/status.tsv`.

Product commit `ac53cc5e9`; evidence commit `a8d9be973`. Pushed to `origin/card-campaign`;
remote SHA `a8d9be973a9b3752d521ecd34b6373f69cb150e0` verified by `git fetch` and `git rev-parse`.

## Blockers and gaps

No card in this family is blocked. No partial admission. No new engine capability was needed.

## MTG applicability

CR 305.6 (a land with two basic land types has a mana ability for each), CR 305.8 (a land is
basic only if it has the basic supertype, so these are nonbasic despite their basic land types),
CR 614.1d and 614.12 (the entry replacement checks the game state
immediately before the land enters), CR 605.1a and 605.3b (the mana ability resolves without using
the stack). No Comprehensive Rules deviation.
