# Handoff after run 88 — three families verified and delivered

## Outcome

Queue-v88 batch A: three families, 16 cards, one full batch gate exit 0 on the final reviewed
bytes. This reaches the session's three-family cap.

| Family | Cards | Review | Gate |
|---|---|---:|---|
| 01-planet-refuges | 10 | APPROVED (delta) | `verify-1175655f-71da-4886-a7bb-f74e3eadc5af` |
| 02-sac-draw-creatures | 3 | APPROVED (delta) | same |
| 03-counter-activation | 3 | APPROVED (delta) | same |

Cards: Andoria, Bajor, Cardassia Prime, Earth Sector 001, Ferenginar, Orion, Qo'noS, Risa, Romulus,
Vulcan; Brass Secretary, Parcel Myr, Slinking Skirge; Jungle Delver, Unholy Officiant,
Verdant Automaton.

## Queue expansion

Queue-v87 was fully delivered, so `build/campaign/queue-v88/` was created. The exact-clause and
exact-analogue searches are now almost empty (1 exact group, 38 keyword-peeled cohorts), so v88 is
again a reuse-only queue grouped by typed shape. Selection rationale, rejected cohorts and the
identity-drift analysis are in `build/campaign/queue-v88/selection.md`.

## The two things this run actually had to solve

**1. The prompt's bulk pin is not the tree's pin.** The brief names `a9a2fd3b…` (2026-11-10), but
HEAD's presentation registry was regenerated in run 87 from `1cee09ee…` (2026-11-26), and the
tree-default `oracle-cards.jsonl.gz` is that newer file. `a9a2fd3b…` only survives at
`/tmp/scryfall-analysis/oracle-cards.jsonl.gz`. Using the older pin would have produced
fingerprints the registry does not contain. This run therefore uses the tree pin `1cee09ee…` and
**additionally** asserts every selected card is byte-identical under both pins
(`prepare.py`), so nothing in the batch depends on which pin a later check uses. Five cards in the
whole corpus drift between the pins (`identity-drift.json`); none is in this batch. Fetching or
switching bulk would have required separate authorization and was not needed.

**2. The comma in `Earth, Sector 001`.** Engine `slugify` preserves the printed comma, so the card
id is `earth,_sector_001` (exactly like the shipped `kavaron,_memorial_world`), while
`CardFaceId::validate` requires the face id to be canonical snake_case `earth_sector_001`. The
presentation registry is keyed by `(card id, face id)`, so the first refresh (run against the
un-comma'd id) produced a row the registry could not resolve. The refresh was re-run after the id
fix, and the registry test now derives the face id from the registered definition instead of a
literal table.

## Review

Round 1, two independent read-only reviewers, both CHANGES REQUIRED. Material findings, all fixed:

- All three controller-scope tests activated as the foreign player without handing over priority,
  so the rejection was `"not your priority"` and the tests would have passed even if any player
  could activate a foreign permanent. They now hand priority over, assert it is held, and assert the
  exact controller/zone rejection. (The sacrifice branch's fix initially did not apply and was
  caught in delta review.)
- The sacrifice draw was proven only in the Brass Secretary canary; the per-card loop now asserts
  hand `+1` / library `-1` for all three.
- Wrong CR citations: 701.16 is Investigate (sacrifice is 701.21a), 107.5 is the tap symbol (these
  cards have no `{T}` cost), and 305.6 is the basic-land-type rule (Planet is 205.3i/305.5). All
  corrected in the RONs and records.
- "Clue subtype is display-only" was wrong; Clue is a real subtype other costs consume.
- The claimed "reviewed in queue-v84" Illvoi Galeblade provenance did not exist; reworded to
  "shipped analogue".
- No test pinned the absence of a `{T}` cost or a timing restriction; the registry now asserts the
  full cost list, `ActivationTiming::Normal`, empty conditions, no activation limit, no targeting,
  and an exhaustive `EntersTapped { affected: Self_, condition: None, unless_cost: None }`.
- The red log predated the hardened tests and there was no scenario green log; both were re-run on
  the frozen bytes and all three family records, `selection.md`, `revalidation.tsv`,
  `identity-drift.json` and the four logs were added to the freeze manifest.

Delta review: APPROVED, then a second delta review after the sacrifice-branch fix: APPROVED.

## Delivery

Product commit `f7880fa10`; evidence commit `aefadf876`. Pushed to `origin/card-campaign`; remote
SHA `aefadf876f2010da083bf06aa5a2a724c55aaf31` verified by `git fetch` and `git rev-parse`.
Worktree clean.

## Evidence

- Intended red: `build/campaign/queue-v88/logs/red-q88.log` (4 failed, 0 passed with the sixteen
  RONs held out; predates the hardening and is described as such in the records).
- Focused green: `green-q88-registry.log` (4 passed) and `green-q88-scenario.log` (12 passed),
  both produced after the final edits.
- Preflight: `preflight-q88.log` (fmt 0, Clippy 0, `git diff --check` 0).
- Non-vacuity probes: a wrong color pair, a wrong counter cost, an added `Tap` cost and a doubled
  draw each fail the focused tests.
- Batch gate: `verify-1175655f-71da-4886-a7bb-f74e3eadc5af`, Status Pass, exit 0 — environment,
  diff check, CardData, dependency boundary, five package format checks, Clippy, full Rust tests
  (all 16 q88 tests observed passing in the gate log).

## Disk

`tricerules/target/debug/incremental` had grown to 17 GiB and left only 11 GiB free, under the
gate's 10 GiB reserve. That directory is disposable build output; removing it restored 28 GiB.
`cargo clean --profile dev` remains the documented recovery if more space is ever needed.

## Next

Queue-v88 is exhausted as selected. The deferred leads are the three-card Turbulent land cycle
(opponent-controls-eight-or-more-lands entry condition, a live opponent count adjacent to the
already-blocked `#519`) and the `{M}: this creature gets +N/+N` cohort, which needs splitting by
P/T magnitude before it is a coherent typed shape. Blockers `#515`–`#532` remain excluded; no row
was retried and no new engine blocker appeared.

No context compaction occurred. Three families delivered, reaching the family cap.
No GUI acceptance surface.
