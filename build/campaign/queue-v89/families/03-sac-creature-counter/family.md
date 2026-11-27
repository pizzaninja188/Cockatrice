# 03-sac-creature-counter — three "sacrifice another creature" counters

- Queue: `build/campaign/queue-v89`.
- Cards: 3.
- Band: simple single-primitive cohort (3).
- Status: **verified**; independent read-only review approved the reviewed semantics and the
  queue-v89 consolidated batch gate passed exit 0.
- Pinned bulk: SHA-256 `1cee09ee1ac70977c46171f4abf5cdfe170ab43bc32232af9138404e27524ce6`.
- Pinned rulings: SHA-256 `84f4c793e6d4f1256206b915da007111e1a445fe7cad99147802086454e8a95d`.

## Typed shape and analogue

Each card prints "`{cost}`, Sacrifice another creature: Put a +1/+1 counter on this creature." The
printed cost pair is a mana payment followed by
`SacrificePermanent(filter: (kind: Creature, controller: You, excluded_objects: [Source]))`, and the
effect is `PutCounters(counter: PlusOnePlusOne, count: 1, subject: Source)`. That pair ships on
Hungry Ghoul and Unburied Earthcarver; the exclusion of the source is what makes the printed "another
creature" a real cost rather than a self-sacrifice outlet, and the shipped
`composite_activated_costs` assertions already prove a self-selection is rejected.

The family varies the mana component (`{G}`, `{1}`, `{1}`) and, where printed, one rider, so the
exact cost mapping is asserted per card instead of one hard-coded amount.

## Cards

| Card | Oracle ID | Cost | Type | P/T | Activation | Rider | RON | Map |
|---|---|---|---|---|---|---|---|---|
| Gobbling Ooze | `765a12fc-9b66-4bb7-88f6-6e7bd2d2f8c7` | `{4}{G}` | Creature — Ooze | 3/3 | `{G}` | — | `data/gobbling_ooze.ron` | `review-maps/gobbling_ooze.json` |
| Hobblefiend | `f2b284cd-fa12-4ba1-b297-80105bb48fde` | `{1}{R}` | Creature — Devil | 2/1 | `{1}` | Trample | `data/hobblefiend.ron` | `review-maps/hobblefiend.json` |
| Ravenous Harpy | `e9ccec56-ec2b-4ab8-b929-6e4795711db9` | `{2}{B}` | Creature — Harpy | 1/2 | `{1}` | Flying | `data/ravenous_harpy.ron` | `review-maps/ravenous_harpy.json` |

Hobblefiend and Ravenous Harpy both charge `{1}`, but they differ in body, color, keyword and
combat role, so they are two cards of one typed shape rather than a duplicated definition. Each is
independently asserted.

## Pinned rulings

None of the three has a pinned ruling. The sacrifice is part of the activation cost (CR 602.1a puts everything before the colon in the
cost, and 701.21a defines the sacrifice action), so it is a cost selection rather than a target
(CR 115.1). The mana component is paid at activation under CR 118.3/118.3a and 601.2h/602.2b, and
the counter placement is the ordinary CR 122.1/122.6 operation. All are covered by the shipped
analogue's assertions.

Packets: `build/campaign/queue-v89/families/03-sac-creature-counter/packets/`.

## Canaries

- Simplest: Gobbling Ooze — a single green pip and no keyword, so the whole activation is `{G}`,
  one sacrificed creature, one counter.
- Hardest: Ravenous Harpy — the only flying body and a generic `{1}` cost. Its canary first proves
  the source cannot be its own sacrifice payment (rejected, mana refunded, source still on the
  battlefield), then succeeds with another creature, checks the counter and that flying survives.

## Semantic evidence

- `scenario q89_batch_a::q89_sac_counter_canary_simplest_sacrifices_another_creature_for_one_counter`
- `scenario q89_batch_a::q89_sac_counter_canary_hardest_keeps_its_keyword_and_cannot_sacrifice_itself`
- `scenario q89_batch_a::q89_sac_counter_cards_require_mana_another_creature_and_their_controller`
- `scenario q89_batch_a::q89_sac_counter_cards_stack_one_counter_per_sacrifice`
- `mechanic_registry::q89_batch_a::q89_sac_creature_counter_cards_map_definitions`
- `mechanic_registry::q89_batch_a::q89_batch_a_fingerprints_match_the_pinned_oracle_text`

The shared loop asserts, per card: activation without the printed mana fails with no sacrifice and
no counter; the source cannot pay its own cost; a foreign player holding priority is rejected with
the controller/zone message and sacrifices nothing. The stacking test performs two activations and
checks the chosen creature each time plus exactly two counters on the printed body.

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
- Non-vacuity: raising Gobbling Ooze's activation from `{G}` to `{1}{G}` fails its canary and the
  stacking test while the two other sac-counter scenarios still pass
  (`logs/probe-gobbling-cost.log`, 2 passed / 2 failed, exit 101). Restored and re-verified against
  the draft.

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

No card in this family is blocked. No partial admission. The neighbouring free-sacrifice outlets
that pump rather than place counters (`sac_creature_self_pump_batch`) are a different typed effect
and are not part of this family.

## MTG applicability

CR 602.1a/602.2 (everything before the colon is an activation cost), CR 701.21a (the sacrifice
action and who may sacrifice), CR 118.3/118.3a (the mana component is paid at activation),
CR 601.2h/602.2b (cost payment during activation), CR 115.1c (a cost is not a target),
CR 122.1a (a +1/+1 counter modifies the permanent's power and toughness), CR 702.9a-b (flying) and
CR 702.19a-b (trample). No
Comprehensive Rules deviation.
