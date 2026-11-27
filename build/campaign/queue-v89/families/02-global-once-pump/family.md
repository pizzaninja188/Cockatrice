# 02-global-once-pump — four once-each-turn self-pumps

- Queue: `build/campaign/queue-v89`.
- Cards: 4.
- Band: multi-clause composition (4).
- Status: **verified**; independent read-only review approved the reviewed semantics and the
  queue-v89 consolidated batch gate passed exit 0.
- Pinned bulk: SHA-256 `1cee09ee1ac70977c46171f4abf5cdfe170ab43bc32232af9138404e27524ce6`.
- Pinned rulings: SHA-256 `84f4c793e6d4f1256206b915da007111e1a445fe7cad99147802086454e8a95d`.

## Typed shape and analogue

Every card prints "`{cost}`: This creature gets +N/+M until end of turn. Activate only once each
turn." That is one activated ability with `costs: [Mana(...)]`,
`effect: [PumpTarget(power: N, toughness: M, subject: Source)]` and
`activation_limit: Some(PerTurn(max_activations: 1))`. The exact limit-and-pump pair ships on
Darkthicket Wolf, Frilled Oculus, Rootwalla, Plated Rootwalla and Knight of the Skyward Eye
(`self_pump_once_batch`), and on Snarling Wolf / Frilled Oculus / Knight of the Skyward Eye.

The family varies the printed cost, the P/T delta and, where printed, one rider. This is not the
13-card "`{M}`: gets +N/+N" keyword-peeled cohort: that one merges `{0}` costs, hybrid and
colorless symbols, and unlimited shades, which are genuinely different activation shapes.

## Cards

| Card | Oracle ID | Cost | Type | P/T | Delta | Activation | Rider | RON | Map |
|---|---|---|---|---|---|---|---|---|---|
| Basking Rootwalla | `b2ec080f-ad5d-4274-91e3-bc6ac89fc3e4` | `{G}` | Creature — Lizard | 1/1 | +2/+2 | `{1}{G}` | Madness `{0}` | `data/basking_rootwalla.ron` | `review-maps/basking_rootwalla.json` |
| Ghor-Clan Bloodscale | `e3367ed8-16a3-4b04-bc6a-e0a3c794e6d3` | `{3}{R}` | Creature — Lizard Warrior | 2/1 | +2/+2 | `{3}{G}` | First strike | `data/ghor-clan_bloodscale.ron` | `review-maps/ghor-clan_bloodscale.json` |
| Twinblade Slasher | `2375a9c6-28ef-4c8f-909c-0aa7e5fc085c` | `{G}` | Creature — Elf Warrior | 1/1 | +2/+2 | `{1}{G}` | Wither | `data/twinblade_slasher.ron` | `review-maps/twinblade_slasher.json` |
| Viashino Slaughtermaster | `34bdf2f1-0cc9-4588-9a3e-5cfe5cde815d` | `{1}{R}` | Creature — Lizard Warrior | 1/1 | +1/+1 | `{B}{G}` | Double strike | `data/viashino_slaughtermaster.ron` | `review-maps/viashino_slaughtermaster.json` |

### Card-id note

Engine `slugify` keeps a printed hyphen, so Ghor-Clan Bloodscale's card id is
`ghor-clan_bloodscale` and its RON file and review map are `ghor-clan_bloodscale.ron` /`.json`, exactly like the shipped `an-havva_township`. The face id used inside the definition and
the map span is canonical snake_case `ghor_clan_bloodscale`, the same split the shipped
`kavaron,_memorial_world` / `kavaron_memorial_world` pair uses. The family and registry tests assert both halves explicitly
rather than deriving the face id from the card id.

## Line order

Ghor-Clan Bloodscale, Twinblade Slasher and Viashino Slaughtermaster each print the keyword on line
1 and the pump on line 2. Basking Rootwalla prints the pump on line 1 and Madness on line 2. Each
map's spans match the pinned bulk order exactly, and the registry test asserts each ability's
`OracleLines` index.

## Pinned rulings

- Basking Rootwalla: ten pinned rulings. The 2022-12-08 set is the shared madness contract already
  asserted by the shipped Arrogant Wurm / Fiery Temper path. The decisive 2006-02-01 ruling — "If
  this card's ability is activated by one player, then another player takes control of it on the
  same turn, the second player can't activate its ability that turn" — is asserted directly by
  `q89_once_pump_cards_keep_the_limit_after_a_control_change`, which activates, moves control
  without a zone change, hands the new controller priority and the printed mana, and requires the
  `activation limit reached` refusal with unchanged P/T. The new-turn test then proves the limit
  resets with the turn rather than with the object.
- Ghor-Clan Bloodscale: one 2006-02-01 ruling, the same once-each-turn statement. Covered by the
  same assertion.
- Twinblade Slasher, Viashino Slaughtermaster: no pinned ruling.

Packets: `build/campaign/queue-v89/families/02-global-once-pump/packets/`.

## Canaries

- Simplest: Basking Rootwalla — the cheapest body, `{1}{G}`, no keyword, plus the Madness static.
  The canary proves one activation applies the printed +2/+2, spends exactly the printed mana, and
  that the second same-turn activation is refused by the *limit* (the pool is refilled first, so an
  empty pool cannot explain the refusal).
- Hardest: Viashino Slaughtermaster — a two-color `{B}{G}` activation and a double-strike body. It
  charges both colors (green alone is refused), applies +1/+1 (not the family's +2/+2), and keeps
  double strike across the pump.

## Semantic evidence

- `scenario q89_batch_a::q89_once_pump_canary_simplest_pumps_once_per_turn`
- `scenario q89_batch_a::q89_once_pump_canary_hardest_charges_two_colors_and_keeps_its_keyword`
- `scenario q89_batch_a::q89_once_pump_cards_pump_once_each_turn_and_require_their_controller`
- `scenario q89_batch_a::q89_once_pump_cards_reset_the_limit_on_a_new_turn`
- `scenario q89_batch_a::q89_once_pump_cards_keep_the_limit_after_a_control_change`
- `mechanic_registry::q89_batch_a::q89_global_once_pump_cards_map_definitions`
- `mechanic_registry::q89_batch_a::q89_batch_a_fingerprints_match_the_pinned_oracle_text`

The shared loop asserts, per card: one activation applies the printed delta once, the printed mana
is spent, the source stays untapped, a refilled pool still cannot buy a second same-turn
activation (the refusal names the limit), a foreign player holding priority is rejected with the
controller/zone message and changes nothing, and a synthetic control change keeps the spent
activation without a zone change. The new-turn test advances to the next turn and
proves exactly one more activation is available and that the previous turn's pump has expired.

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
- Non-vacuity: replacing Viashino Slaughtermaster's `{B}{G}` with `{1}{G}` fails the hardest
  canary while the other four once-pump scenarios still pass (`logs/probe-viashino-cost.log`,
  4 passed / 1 failed, exit 101) — a `{1}{G}` copy is caught even though black plus green can also
  pay one generic plus one green. Restored and re-verified against the draft.

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

No card in this family is blocked. No partial admission. The activated-modal, hybrid and `{0}`
neighbours in the keyword-peeled cohort remain queued and were not merged here.

## MTG applicability

CR 602.1a/602.2 (activated ability and its printed cost), CR 602.5b and the printed instruction
(one activation per turn, resetting when the turn ends), CR 611.2c/613.4c (the pump is a layer-7c
continuous effect), CR 514.2 (it ends at cleanup), CR 702.4a-b (double strike), CR 702.7a-b (first
strike), CR 702.35a-c (madness) and CR 702.80a (wither). No Comprehensive Rules deviation.
