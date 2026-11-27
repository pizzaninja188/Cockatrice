# Handoff after run 89 — three families verified and delivered

## Outcome

Queue-v89 batch A: three families, 10 cards, one consolidated full gate exit 0 on the final
reviewed bytes. This reaches the session's three-family cap.

| Family | Cards | Review | Gate |
|---|---|---:|---|
| 01-turbulent-dual-lands | 3 | APPROVED | `verify-0133872a-7d0a-435b-acd5-478a839abc33` |
| 02-global-once-pump | 4 | APPROVED | same |
| 03-sac-creature-counter | 3 | APPROVED | same |

Cards: Turbulent Crater, Turbulent Shore, Turbulent Wetlands; Basking Rootwalla, Ghor-Clan
Bloodscale, Twinblade Slasher, Viashino Slaughtermaster; Gobbling Ooze, Hobblefiend, Ravenous Harpy.

## Queue expansion

Queue-v88 was fully delivered, so `build/campaign/queue-v89/` was created. The exact-text and
exact-analogue searches are effectively exhausted: `scan.py` now finds **one** exact group (the
three Turbulent remnants, 24 fully-covered cards) and 35 keyword-peeled cohorts at 4+, almost all
three-card remnants of shapes that already ship. Selection rationale, rejected cohorts and the
identity-drift analysis are in `build/campaign/queue-v89/selection.md`.

The Turbulent entry condition was re-examined rather than assumed blocked: `blocked.tsv` `#519`
covers "enters tapped unless you have two or more opponents", a live *player* count. Turbulent Fen
and four siblings already ship the sibling but distinct *battlefield aggregate of opponents' lands*
condition, so these three are pure reuse and were admitted. `#519` was not retried.

## What this run actually had to solve

**1. A printed hyphen is part of the card id.** Engine `slugify` keeps it, so Ghor-Clan Bloodscale
is `ghor-clan_bloodscale` — and the RON **file name, review map file name and packet name must all
match that stem**, because `check-card-evidence.ps1` and `routine-ron-checks.ps1` derive the map
path from the RON stem and `resolve_batch_review_draft` derives the draft from the map stem. The
first attempt named the map `ghor_clan_bloodscale.json`; the routine map check then rejected it.
The split is: hyphen kept in file stems, canonical snake_case `ghor_clan_bloodscale` in the
`face_id` and inside map spans — the same split shipped `an-havva_township` uses.

**2. A mutation probe exposed a weak test rather than a weak card.** Mutating Viashino
Slaughtermaster's `{B}{G}` to `{1}{G}` *passed* the scenario tests, because black plus green can
also pay one generic plus one green. The canary now also supplies exactly one generic plus one
green and asserts the activation is refused; the mutation then fails as intended. Any future
two-color activation test in this repository needs that pool, not just the correct colors.

**3. Review rounds found real documentation defects.** Two independent reviewers rejected the first
freeze for, among others, CR 702.34 cited for madness (that is Flashback; madness is 702.35a-c),
CR 118.12 cited for an activation-cost mana payment (it is the resolution-time-cost rule), CR 305.8
glossed as the two-basic-type mana rule (that is 305.6), a claimed gate and mutation run that did
not exist, and red logs that predated the frozen tests. All were fixed, the red logs were
re-captured on the frozen bytes, and per-mutation probe logs replaced the one combined log whose
later failures were `once_cell` poisoning. The third delta review approved.

## Review

Round 1: two independent read-only reviewers, both CHANGES REQUIRED. Round 2 delta: CHANGES
REQUIRED on five residual record/evidence items. Round 3 delta: **APPROVED**, no blocking defects.
Delta reviews confirmed the fixes and checked the new control-change and not-basic assertions were
non-vacuous.

## Delivery

Product commit `ac53cc5e9`; evidence commit `a8d9be973`. Pushed to `origin/card-campaign`; remote
SHA `a8d9be973a9b3752d521ecd34b6373f69cb150e0` verified by `git fetch` and `git rev-parse`.
Only the three `family.md` records and `status.tsv` are tracked from `build/` (matching the v88
precedent); the queue directory stays ignored.

## Evidence

- Intended red re-captured on the frozen tests with all ten RONs held out:
  `build/campaign/queue-v89/logs/red-q89-registry.log` (4 failed / 0 passed) and
  `red-q89-scenario.log` (13 failed / 0 passed).
- Focused green: `green-q89-registry.log` (4 passed), `green-q89-scenario.log` (13 passed).
- Non-vacuity, one mutation per run, each restored and hash-verified afterwards:
  `probe-crater-color.log` (0/4), `probe-viashino-cost.log` (4 passed / 1 failed),
  `probe-gobbling-cost.log` (2 passed / 2 failed).
- Preflight: `.log` records fmt 0, Clippy 0, `git diff --check` 0, and
  `check-card-evidence.ps1 -Preparation` PASS over all 3549 maps.
- Batch gate `verify-0133872a-7d0a-435b-acd5-478a839abc33`, Status Pass, exit 0 — environment,
  diff check, CardData, dependency boundary, five package format checks, Clippy, full Rust suite
  with all 17 q89 tests observed passing.

## Disk

`tricerules/target/debug/incremental` had grown to ~9 GiB (19 GiB free). Removing it restored
28 GiB before the gate. The gate then completed with the 10 GiB reserve intact.
`cargo clean --profile dev` remains the documented recovery if more space is needed.

## Next

Queue-v89 is exhausted as selected. Remaining leads, all real but thin:

- The 13-card `{M}: this creature gets +N/+N` cohort still needs splitting by P/T magnitude and
  cost shape before it is one typed cohort. Natural sub-cohorts are `{M}` +1/+1 shades (largest),
  `{M}` +1/+0 pumpers, and the `{M}{M}` generic-plus-pip group.
- `{M}: this creature gains shroud until end of turn` (3 cards, `{U}`) and
  `{M}{M}: this creature gains first strike until end of turn` (3 cards, three colors, two costs)
  are genuine three-card cohorts with a single shipped analogue each; both were deferred as too
  thin to be a family in this session, not rejected.
- The regenerate cohorts (8 and 8 cards) carry morph/megamorph/infect/cascade/echo/protection
  riders that each need their own evidence; split them by rider.
- Blockers `#515`–`#536` stay excluded. No row was retried and no new engine blocker appeared.

No context compaction occurred. Three families delivered, reaching the family cap.
No GUI acceptance surface (data-only change).
