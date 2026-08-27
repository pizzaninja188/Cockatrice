# Card-coverage evaluation — 2026-08-27

Fourth evaluation: **360 unimplemented cards**, twice the earlier 180-card sample, with
**180 commons and 180 uncommons** from six previously unsampled sets. The purpose is to identify
reusable engine work that does not require new UI surface. **13 live Rust-only capability issues
were created: #164–#176.** No engine or card behavior is implemented by this evaluation.

## Result

| Classification | Cards | Rate |
|---|---:|---:|
| Fully implementable now | 146 | 40.6% |
| Partially implementable now | 136 | 37.8% |
| Full or partial | 282 | 78.3% |
| Blocked | 78 | 21.7% |
| **Total** | **360** | **100.0%** |

These are **source-level capability assessments**, not 360 compiled RON implementations or
360 passing card scenarios. Every sampled Oracle text and face was reviewed against the
current vocabulary and relevant engine paths. `full_now` permits new RON/token data but no new
Rust. `partial_now` retains an independently meaningful ability, keyword, mode or spell face
after an explicitly recorded omission; a plain body or drawback alone is not sufficient.
`blocked` has no useful faithful implementation under that rubric. Partial is not playable
rules compliance, and no partial cards were added.

Do not compare these percentages as an engine-maturity trend: rarity, sets, eligible population,
sample and baseline all differ from earlier reports. Uncommons deliberately expose more mechanics.

By stratum:

| Set and rarity | Full | Partial | Blocked | Total |
|---|---:|---:|---:|---:|
| BRO-common | 16 | 10 | 4 | 30 |
| BRO-uncommon | 6 | 14 | 10 | 30 |
| WOE-common | 15 | 9 | 6 | 30 |
| WOE-uncommon | 11 | 12 | 7 | 30 |
| LCI-common | 18 | 8 | 4 | 30 |
| LCI-uncommon | 9 | 13 | 8 | 30 |
| MKM-common | 19 | 8 | 3 | 30 |
| MKM-uncommon | 10 | 13 | 7 | 30 |
| OTJ-common | 15 | 11 | 4 | 30 |
| OTJ-uncommon | 3 | 15 | 12 | 30 |
| BLB-common | 17 | 9 | 4 | 30 |
| BLB-uncommon | 7 | 14 | 9 | 30 |

## Reproducible sample

- Baseline: `5ecc9219e9e5551156a97e49cd5895eaaa662ca4`; clean worktree before evaluation.
  The live registry checklist was regenerated/checked successfully: **1,449 full + 8 partial names**.
- Sets in assignment order: The Brothers' War (BRO), Wilds of Eldraine (WOE), The Lost Caverns
  of Ixalan (LCI), Murders at Karlov Manor (MKM), Outlaws of Thunder Junction (OTJ), Bloomburrow (BLB).
  Within each set, common precedes uncommon.
- Live Scryfall queries: `set:<set> rarity:<common|uncommon> game:paper is:booster lang:en`,
  with `unique=cards&order=name`, an explicit User-Agent, and pagination support.
  Response files were captured from 2026-08-27T07:05:02.3866037Z to
  2026-08-27T07:05:13.3973419Z.
- Exclude full and partial registry-resolved names, checking full names and face names. Then
  deduplicate Oracle IDs in the declared stratum order. Do not exclude a card merely because it
  has appeared in an earlier capability assessment.
- Within each of twelve strata, take the first **30** ascending
  `SHA-256("cockatrice-card-coverage-2026-08-v4\n<oracle_id>")`.
  Here `\n` is one actual LF character, not a backslash and an n.
- Preserve the **1,092-card eligible population** so the exact selection can be reproduced offline.
  Selected rows retain Oracle/Scryfall IDs, hash, rank, mana, type, Oracle text, keywords,
  P/T, face snapshots, rationale, code paths and source/ruling URLs.
  Joined face-label text strips trailing whitespace; face snapshots retain the original fields.
- This is an immutable snapshot. Future implementations must not rewrite its classifications.
  The manifest records file and source-response SHA-256 values. Raw response caches are temporary;
  current Scryfall searches may differ. Registry names are tied to the baseline and recorded hash.

| Stratum | Source cards | Eligible after exclusion/dedup | Selected |
|---|---:|---:|---:|
| BRO-common | 106 | 97 | 30 |
| BRO-uncommon | 80 | 78 | 30 |
| WOE-common | 106 | 96 | 30 |
| WOE-uncommon | 80 | 80 | 30 |
| LCI-common | 113 | 108 | 30 |
| LCI-uncommon | 92 | 91 | 30 |
| MKM-common | 86 | 78 | 30 |
| MKM-uncommon | 100 | 100 | 30 |
| OTJ-common | 96 | 90 | 30 |
| OTJ-uncommon | 100 | 100 | 30 |
| BLB-common | 86 | 79 | 30 |
| BLB-uncommon | 100 | 95 | 30 |

## Engine-only backlog

The normalized capability gaps are:

| Capability | Occurrences | Sole-gap candidates | Tracker |
|---|---:|---:|---|
| zone-aware target and cohort characteristic predicates | 21 | 11 | [#176](https://github.com/pizzaninja188/Cockatrice/issues/176) |
| filtered and batched permanent event observers | 14 | 10 | [#168](https://github.com/pizzaninja188/Cockatrice/issues/168) |
| reusable public quantity expressions for counts, scaling, and source power | 20 | 9 | [#165](https://github.com/pizzaninja188/Cockatrice/issues/165) |
| composable blocker restrictions and conditional evasion | 5 | 5 | [#174](https://github.com/pizzaninja188/Cockatrice/issues/174) |
| committed graveyard-entry and sacrifice turn facts | 6 | 3 | [#167](https://github.com/pizzaninja188/Cockatrice/issues/167) |
| Expend thresholds from actual mana spent casting spells | 3 | 3 | [#172](https://github.com/pizzaninja188/Cockatrice/issues/172) |
| committed Crime events and turn predicates | 7 | 2 | [#171](https://github.com/pizzaninja188/Cockatrice/issues/171) |
| actor-aware tap events and grouped tap triggers | 3 | 2 | [#169](https://github.com/pizzaninja188/Cockatrice/issues/169) |
| life-gain and life-loss turn history | 3 | 2 | [#170](https://github.com/pizzaninja188/Cockatrice/issues/170) |
| automatic cast-completion condition snapshots | 2 | 2 | [#173](https://github.com/pizzaninja188/Cockatrice/issues/173) |
| continuous prohibitions on gaining life | 1 | 1 | [#175](https://github.com/pizzaninja188/Cockatrice/issues/175) |
| per-turn trigger limits with stable ability identity | 6 | 0 | [#164](https://github.com/pizzaninja188/Cockatrice/issues/164) |
| filtered spell-cast facts and cast-origin history | 6 | 0 | [#166](https://github.com/pizzaninja188/Cockatrice/issues/166) |

A sole-gap candidate has every observed gap mapped to the same new issue. Counts are not additive
coverage forecasts across issues: **89 cards** encounter at least one new engine gap,
**56 cards** have only gaps in this new engine batch, and **50 cards** are sole-gap
candidates. Dependencies and implementation verification still apply; keyword-only partials are
not treated as full implementations.

Dependencies: **#166 → #165** for quantity consumers; **#169 → #164** for Sharae's trigger cap;
**#171 → #164** for capped Crime consumers. The core predicates can be developed without any open
UI issue. #164 is useful shared infrastructure even though it has no sole-gap card in this sample.

Suggested starting points:

1. [#172 Expend](https://github.com/pizzaninja188/Cockatrice/issues/172): three cards, no new payment choices.
2. [#173 cast snapshots](https://github.com/pizzaninja188/Cockatrice/issues/173): two cards with precise
   Oracle rulings and no new user decisions.
3. [#168 event filters](https://github.com/pizzaninja188/Cockatrice/issues/168): ten sole-gap candidates,
   with more LKI and simultaneous-event interaction work.
4. [#176 target predicates](https://github.com/pizzaninja188/Cockatrice/issues/176): eleven sole-gap
   candidates through existing engine-authored candidate lists.

Each issue names multiple real cards (Giant Cindermaw additionally uses the fetched supplemental
Rampaging Ferocidon example), records current code seams, acceptance cases, dependencies, rules
sources, red/green tests and full Rust gates. Supplemental examples are outside the denominator.

## Existing ownership and deliberate deferrals

The [gap map](card-coverage-calibration-2026-08-27-gaps.csv) accounts for **83 unique raw gap labels** exactly once, with
`new_engine`, `existing_dependency` or `deferred` dispositions, code paths and an explicit reason.

Existing [#46](https://github.com/pizzaninja188/Cockatrice/issues/46),
[#155](https://github.com/pizzaninja188/Cockatrice/issues/155), and
[#159](https://github.com/pizzaninja188/Cockatrice/issues/159) retain ownership of token-copy
substrates, attachment/physical-zone actions, and typed result continuations. They were not
duplicated or expanded. **Offspring is not solved by #46 alone:** its 1/1 copy modifications and
payment-linked entry are a separate deferred label.

New UI, private-zone, payment, designation or physical-identity work was not filed in this batch:
Prototype, Unearth, Roles, Sagas, Explore/Map, Discover, Craft, Disguise, Cases, Suspect, Plot,
Saddle/Crew, Spree, Gift and Offspring remain visible in the evidence. Powerstone mana also stays
deferred: its permission includes activation and resolution payments, not just artifact spells.

Some smaller engine topics are also marked deferred, rather than being called UI requirements:
general damage observers, source/target object conditions, new counter cohorts, selective keyword
removal, and cast prohibitions need another bounded design or stronger consumer evidence. Thus
these 13 issues are a substantiated no-UI backlog, not a claim to have filed every conceivable
missing primitive from the sample.

Completed issue titles were checked against current code, not treated as proof of coverage.
For example, noncreature self graveyard-entry already works through `WhenSelfDies`
(Mephitic Draught, Krovod Haunch), Oaken Siren's restricted mana is expressible, and Karlov
Watchdog's special-action prohibition is already supported. Conversely, completed #158's
damage-history facts do not automatically supply damaged-object target filters; #113's attached
characteristic setters do not author arbitrary one-shot animation effects.

## Evidence and verification

- [Per-card assessment](card-coverage-calibration-2026-08-27.csv)
- [Eligible population](card-coverage-calibration-2026-08-27-population.csv)
- [Gap-to-issue/disposition map](card-coverage-calibration-2026-08-27-gaps.csv)
- [Manifest and SHA-256 values](card-coverage-calibration-2026-08-27-manifest.json)
- [Fetched rulings for 66 relevant sample cards plus one supplemental card](card-coverage-calibration-2026-08-27-rulings.json)

The existing Windows calibration validator includes this expanded snapshot and checks counts,
identities, strata, selection hashes/ranks against the saved population, face/Oracle evidence,
gap completeness/dispositions, issue statistics, source-code paths and manifest checksums.
A syntactic/evidence check cannot prove the semantic correctness of every proposed card encoding.

Verification performed:

- `./tests/scripts/card_coverage_calibration_test.ps1` — exit 0.
- Registry checklist through the quiet Windows runner — exit 0; generated `CARDS.md` unchanged.
- `git diff --check` — exit 0.
- New live issues fetched again after creation and matched to the saved titles/bodies, open state,
  and feature/medium labels.

No Rust or C++ implementation changed; full engine builds and hands-on two-client tests are N/A
for this documentation/evidence/validator task. No commit or push was requested or performed.

## Interaction-checklist closeout

1. **Rules authority:** Oracle clauses/faces drive rows; relevant fetched rulings and the current
   [official Comprehensive Rules](https://media.wizards.com/2026/downloads/MagicCompRules%2020260819.txt)
   (effective August 7, 2026) drive issue semantics. Expend is actual paid mana; Crime is initial
   committed targeting; Descend is permanent-card graveyard entry, not creature death.
2. **State/identity:** all new capabilities remain internal to tricerules. Issues preserve
   object generation, stable ability occurrence, LKI and deterministic accepted-command history.
3. **Timing:** issues explicitly cover cast completion, event batching, APNAP, intervening-if,
   turn-instance resets, replacement outcomes, and layer versus resolution evaluation.
4. **Players/legality:** player sets, owner/controller/actor distinctions, stale commands,
   failed-payment rollback and authoritative candidate revalidation are required.
5. **Visibility:** no new wire field, private candidate cohort or public history payload is
   needed by the bounded new issues. Hidden-zone flows are deferred, not silently exposed.
6. **Propagation:** protobuf, Servatrice, C++, Qt, freeform and physical binding are explicitly
   N/A for the new scopes. Existing generic output carries results. A discovered need to change
   one of those contracts requires splitting scope, not quietly widening a no-UI issue.
7. **Verification/delivery:** each implementation issue requires focused red/green, full Rust
   test/clippy/fmt, checklist for card data, and diff checks. Manual two-client work is N/A for
   these scopes. This evaluation validates evidence and published tracker entries only.

## MTG applicability

This evaluation covers committed events and history, triggered ability limits, public quantities,
cast snapshots, targeting/combat restrictions, and life-gain prohibitions. Relevant concepts include
CR 603, 608, 613, 700.11 (Descend), 700.13 (Crime), and 700.14 (Expend).
It records compliance gaps without approximating missing mechanics and changes no gameplay.
