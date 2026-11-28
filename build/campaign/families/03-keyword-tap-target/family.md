# 03-keyword-tap-target — four keyword creatures with "{cost}, {T}: Tap target creature."

- Queue: `build/campaign/queue-v90`.
- Cards: 4.
- Band: multi-clause composition (4).
- Status: **verified**; independent read-only review approved the reviewed semantics (APPROVED at
  bundle revision 10) and the queue-v90 consolidated batch gate passed exit 0.
- Pinned bulk: SHA-256 `1cee09ee1ac70977c46171f4abf5cdfe170ab43bc32232af9138404e27524ce6`.
- Pinned rulings: SHA-256 `84f4c793e6d4f1256206b915da007111e1a445fe7cad99147802086454e8a95d`.

## Typed shape and analogue

Each card prints exactly one keyword line, one `{cost}, {T}: Tap target creature.` line, and nothing
else. The activation is the shipped
`costs: [Mana("<printed cost>"), Tap]` with
`effect: [Tap(subject: Chosen((kind: Creature)))]` — the same shape **Akroan Jailer**,
**Checkpoint Officer**, **Elite Arrester**, **Rathi Trapper** and **Tyrant's Machine** ship and the
shared `tap_target_mana_batch` scenario already exercises end to end (activate, pay, resolve, stale
target, insufficient mana).

The family, not a single card, is what this cohort adds: the `tap_target_mana_batch` cohort is
artifact-or-creature bodies with **no** keyword. These four are the keyword-bearing half —
`Defender` on the two Walls, `Flying` on the two fliers — with four different printed activation
costs.

## Cards

| Card | Oracle ID | Card cost | Activation | Keyword | Body |
|---|---|---|---|---|---|
| Mothrider Patrol | `c4073754-7d92-4976-8e7d-d6eb85b91608` | `{W}` | `{3}{W}` | Flying | 1/1 |
| Dazzling Ramparts | `0527fc47-2bfc-4d02-90fa-5a1284f89a0a` | `{4}{W}` | `{1}{W}` | Defender | 0/7 |
| Squall Drifter | `63ec5617-0da6-4534-b502-34304b1ea02b` | `{1}{W}` | `{W}` | Flying | 1/1 |
| Clockwork Drawbridge | `e49eb21c-2a0d-4fd0-9f69-1bff6e94f9e7` | `{W}` | `{2}{W}` | Defender | 0/3 |

Squall Drifter is a printed **Snow** creature, so `Snow` sits in `supertypes` — not in `types`,
where an earlier draft of this family had put it (see the defect note below).

Mothrider Patrol's card cost `{W}` and its activation `{3}{W}` are deliberately different; the
registry test asserts both, and the scenario asserts the printed amount is consumed exactly.

## Pinned rulings

None of the four has a pinned ruling. The tap effect, the resolution revalidation of a stale target
and the insufficient-mana refusal are all shipped evidence on the shared
`tap_target_mana_batch` scenarios, which this family's scenarios extend with the exact-amount and
keyword assertions.

## Copy-from and differences

Copy `akroan_jailer` for the ability and `bladed_sentinel` / `goblin_balloon_brigade` for the
keyword line. Differences: those two grant a keyword *until end of turn* through an activated
ability; these have a **printed static** keyword plus a separately printed tap ability, so the
keyword is `keywords:` and the tap ability is the whole activation.

Ordinary creatures with a `{T}` activation are summoning-sick (CR 302.6); Clockwork Drawbridge is an
artifact creature but still a creature, so the same rule applies.

## Rules evidence

CR 602.1/602.2 (activation and costs), CR 118.3 (mana payment), CR 701.26 (tap), CR 608.2b (a target
that has left before resolution is not affected), CR 302.6 (summoning sickness), CR 702.3 (defender)
and CR 702.9 (flying).

## Semantic evidence

- `scenario q90_batch_a::q90_keyword_tap_canary_taps_the_chosen_creature_and_itself`
- `scenario q90_batch_a::q90_keyword_tap_requires_the_printed_colored_cost`
- `scenario q90_batch_a::q90_keyword_tap_requires_the_exact_printed_amount`
- `scenario q90_batch_a::q90_keyword_tap_fizzles_on_a_stale_target`
- `scenario q90_batch_a::q90_keyword_tap_cards_keep_their_printed_keyword_and_body`
- `mechanic_registry::q90_batch_a::q90_keyword_tap_target_maps_definitions`
- `mechanic_registry::q90_batch_a::q90_batch_fingerprints_match_the_pinned_oracle_text`

## Canaries

- Simplest: Squall Drifter (cheapest activation at `{W}`, a flier).
- Hardest: Dazzling Ramparts (`{1}{W}` activation on a `{4}{W}` 0/7 Defender, so both a
  copy-pasted cost and a dropped keyword fail) and Mothrider Patrol (card cost `{W}` but activation
  `{3}{W}`).

## Focused evidence

- Intended red: `build/campaign/queue-v90/logs/red-q90-registry.log` (3 failed / 2 passed) and
  `red-q90-scenario.log` (16 failed / 0 passed) with all seventeen RONs held out.
- Focused green: `build/campaign/queue-v90/logs/green-q90-registry.log` (5 passed) and
  `green-q90-scenario.log` (16 passed).
- Non-vacuity probe: `build/campaign/queue-v90/logs/probe-keyword-cost.log` mutates Mothrider
  Patrol's activation from `{3}{W}` to `{W}`; the scenario fails 3 passed / 2 failed, on "exactly 1
  white and 3 generic are spent" (the exact-amount test) and on the stale-target test's cost
  assertion. Restored and hash-verified (`13e56586…`). The first version of that probe
  **passed**, which is why the exact-amount assertion exists.
- Format, Clippy and `git diff --check`: exit 0 each.
- Card evidence preparation: PASS over 3566 maps.

## Review round 1 findings, all fixed before the re-freeze

- The stale-target scenario was near-vacuous: it moved the target but never bumped
  `zone_change_generation` and never asserted the target was untapped, so it would have passed even
  if the engine tapped the departed object. It now mirrors the shipped
  `tap_target_mana_batch_cards_handle_stale_or_expired_state` fixture (generation bump, exile, and
  assertions that the stale object is untapped while both costs stay paid).
- Target scope was untested: a mutation adding `controller: Opponent` would have passed. The registry
  test now asserts `TargetController::Any` with the message "either player's creature", so that
  mutation fails there.
- The frozen green logs predated later edits to the RONs and tests. They were re-captured on the
  final bytes; the counts in this record are the re-captured ones.

## Defects this family's own tests caught before freeze

1. `Snow` was authored in `types` instead of `supertypes`. `check-card-evidence -Preparation`
   rejected it with "draft face 0 type line does not match the source"; `boreal_shelf` and
   `arctic_flats` are the shipped precedents.
2. The first cost probe mutated `{3}{W}` to `{W}` and **passed**, because `{W}` can be paid from
   the same pool as `{3}{W}`. A cost-shape assertion was not enough; the test now asserts the pool
   is emptied exactly, which the mutation does catch.
3. The test struct conflated the card's mana cost with the ability's activation cost; both are now
   separate fields asserted separately.

## Batch gate

`build/verification-logs/verify-8c1fef97-ec11-4987-9219-5a25ff4870a8` — Status Pass, exit 0,
covering `01-planeswalker-untap-lands`..`03-keyword-tap-target` (17 cards). Environment, diff
check, CardData, dependency boundary, five package format checks, Clippy, and the full Rust
suite; all 21 q90 tests (5 registry, 16 scenario) were observed passing in that run.
Product commit `f01e5bd20`.
