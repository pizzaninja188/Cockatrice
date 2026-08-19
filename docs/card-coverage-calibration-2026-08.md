# Card-coverage calibration — August 2026

Issue #44 re-measured how many unimplemented modern-core commons can be authored faithfully with
the shipped P1–P5 card vocabulary. The committed row-level evidence is
[`card-coverage-calibration-2026-08.csv`](card-coverage-calibration-2026-08.csv).

## Result

| Classification | Cards | Rate |
|---|---:|---:|
| Fully implementable now | 76 | 42.2% |
| Partially implementable now | 41 | 22.8% |
| Full or partial | 117 | 65.0% |
| Blocked | 63 | 35.0% |
| **Total** | **180** | **100.0%** |

The previous calibration recorded approximately 14% full and 29% full-or-partial. Its exact
sample was not preserved, so this is a directional rather than paired comparison: the full rate
rose by about 28.2 percentage points and the combined rate by about 36.0 points. The result is
well past the point where another primitive should be chosen from anecdotes; the next increment
should author the largest cohesive ready-data family.

By stratum:

| Set | Full | Partial | Blocked | Total |
|---|---:|---:|---:|---:|
| M19 | 31 | 9 | 20 | 60 |
| M20 | 22 | 19 | 19 | 60 |
| M21 | 23 | 13 | 24 | 60 |

## Reproducible sample

- **Rules capability baseline:** `master` at `17954b4a40cee5d00f890dc49a17555f30d66d8f`,
  including the expected post-merge capabilities of pending Issue #48 at
  `d4365cf629d7a0c743387305ec9393153e8927ec`. Issue #48 changes damage prevention and no card in
  this sample's registry-status exclusion set.
- **Snapshot immutability:** this report and its CSV are historical evidence at that capability
  baseline. Later implementation status belongs in [`tricerules/CARDS.md`](../tricerules/CARDS.md)
  or a separately named calibration refresh; do not rewrite these baseline classifications.
- **Oracle retrieval:** Scryfall card search, 2026-08-09 10:15:16–10:15:17 UTC, with
  `unique=cards&order=name` and one query per set:
  `set:<m19|m20|m21> rarity:common game:paper is:booster lang:en`.
- **Input populations:** 116 M19, 117 M20, and 116 M21 English paper booster commons.
- **Implementation exclusion:** a temporary `gen-checklist --check` run against the embedded
  registry and the 2026-07-22 Oracle `cards.xml` reported 880 full + 16 partial cards. Every name
  resolved by that registry, full or partial, was excluded. The command exited 0.
- **Deduplication:** cards are keyed by Scryfall `oracle_id`; a card in more than one target set is
  assigned to its earliest target set in M19 → M20 → M21 order.
- **Eligible populations after deduplication and registry exclusion:** 84 M19, 80 M20, and 78 M21.
- **Selection:** within each stratum, sort ascending by
  `SHA-256("cockatrice-issue-44-v1\n<oracle_id>")` and take the first 60. The CSV records every
  Oracle ID, selection hash, source text, and classification.

The input queries are set-level rather than one request per card. Gatherer rulings were checked
only where they could change a classification, including reflexive ETB payments, same-name
graveyard scaling, attack-recipient damage, next-untap suppression, death-this-turn checks, and
enchanted-creature reanimation.

## Classification rubric

- **`full_now`:** every functional Oracle clause is representable in RON with current generic
  primitives. A new card file or token definition is allowed; new Rust is not.
- **`partial_now`:** the card's main behavior remains useful and safe with a precise bounded
  omission recorded in `partial`. Reducing an unsupported creature to a vanilla body does not
  qualify.
- **`blocked`:** faithful useful behavior requires a new primitive, custom resolution, or broader
  engine/client substrate.

Each full row has one implementation family. Each non-full row has one primary gap and optional
secondary gaps. A row counts toward a primitive's immediate yield only when one focused reusable
primitive is its sole blocker. Exact Oracle wording controls the classification; Comprehensive
Rules and Gatherer rulings break ambiguous cases.

## Coverage shape

The ready full-data families are:

| Existing family | Full cards |
|---|---:|
| Simple triggered creatures | 24 |
| Activated creatures | 8 |
| Targeted removal | 7 |
| Combat tricks | 7 |
| Auras and Equipment | 7 |
| Token makers | 7 |
| Value spells | 5 |
| Mana permanents | 3 |
| Keyword creatures | 3 |
| Counterspells | 2 |
| Other existing families | 3 |

The leading focused missing capabilities are:

| Missing capability | Primary occurrences | Sole-gap unlocks | Posture |
|---|---:|---:|---|
| Permanents entering tapped | 14 | 14 | Best future primitive candidate |
| Composite activated costs | 5 | 3 | Widen `AbilityCost`, not card-specific costs |
| Additional spell costs | 4 | 4 | Needs cast-time cost collection |
| Graveyard card-type filters | 2 | 2 | Small reusable filter widening |
| Conditional P/T characteristics | 2 | 2 | Requires a generic condition model |
| Same-name dynamic counts | 2 | 2 | Requires name-aware dynamic amounts |
| Opponent-controller target filters | 2 | 2 | Reusable target-filter widening |
| Activation conditions over controlled keywords | 2 | 2 | Generic activation restriction |
| Controller discard choices | 2 | 2 | Reusable non-targeted player choice |

Unsupported planeswalker/battle targets account for seven additional primary gaps across two
Oracle wordings, but that is a broad object-model project rather than a focused primitive. The
remaining gaps are mostly singletons or cards with multiple independent blockers; their exact
taxonomy remains in the CSV rather than being promoted from anecdotal demand.

## Next-increment decision

The agreed comparison caps each route at 20 cards:

- **Data yield:** `min(20, 24 simple triggered creatures) = 20`.
- **Primitive yield:** `min(20, 14 sole-blocked enters-tapped cards) = 14`.

The data route wins 20 to 14 and is lower risk because it requires no new primitive, custom Rust,
protocol, relay, or client work. Issue #49 therefore scopes the first 20 triggered creatures in
selection-hash order:

1. Spellgorger Weird
2. Gale Swooper
3. Steadfast Sentry
4. Daybreak Charger
5. Mistral Singer
6. Skyscanner
7. Skymarch Bloodletter
8. Griffin Protector
9. Llanowar Visionary
10. Inspiring Captain
11. Aven Wind Mage
12. Dawning Angel
13. Library Larcenist
14. Highland Game
15. Audacious Thief
16. Spined Megalodon
17. Cloudkin Seer
18. Wall of Runes
19. Rhox Oracle
20. Cavalry Drillmaster

The 14-card enters-tapped cohort remains evidence for a later generic entry-state primitive, but
it is not the next increment.

All 74 primary and secondary gap labels are represented in the tracker by 45 normalized,
reusable capability issues: existing Issue #37 plus Issues #50-#93.

## MTG applicability

Oracle and the Comprehensive Rules govern every row's classification. This calibration changes
no game behavior; it only measures whether the existing engine can represent that behavior.
