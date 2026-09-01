# Current-Standard card-coverage calibration — 2026-09-01

Fifth evaluation: **160 unimplemented cards**, with **20 commons and 20 uncommons** from each of the four newest paper Standard sets: Teenage Mutant Ninja Turtles (TMT), Secrets of Strixhaven (SOS), Marvel Super Heroes (MSH), and The Hobbit (HOB). The official [Standard format page](https://magic.wizards.com/en/formats/standard) was the live legality authority at capture time.

No gameplay behavior is implemented by this evaluation. Thirteen reusable capability issues and one bounded 20-card data issue were filed as issues #179 through #192, with the approved `type:feature` and `priority:medium` labels.

## Result

| Classification | Cards | Rate |
|---|---:|---:|
| Fully implementable now | 80 | 50.0% |
| Partially implementable now | 46 | 28.8% |
| Full or partial | 126 | 78.8% |
| Blocked | 34 | 21.3% |
| **Total** | **160** | **100.0%** |

These are source-level capability assessments, not compiled RON implementations. `full_now` requires every sampled rules clause to be expressible with current data and shared primitives. `partial_now` retains independently meaningful behavior after a recorded omission. A plain body, drawback, or keyword-only remainder is not enough for partial. No partial cards should ship.

Do not treat percentages as a maturity trend: sets, rarity mix, population, and baseline differ from earlier calibrations.

By stratum:

| Set and rarity | Full | Partial | Blocked | Total |
|---|---:|---:|---:|---:|
| HOB-common | 14 | 4 | 2 | 20 |
| HOB-uncommon | 9 | 4 | 7 | 20 |
| MSH-common | 14 | 6 | 0 | 20 |
| MSH-uncommon | 6 | 6 | 8 | 20 |
| SOS-common | 12 | 5 | 3 | 20 |
| SOS-uncommon | 5 | 5 | 10 | 20 |
| TMT-common | 13 | 6 | 1 | 20 |
| TMT-uncommon | 7 | 10 | 3 | 20 |

## Reproducible sample

- Baseline: `3aaf6db956be6210dde7bde4d351924076daa862`; user-owned deck edits were present and untouched.
- Registry baseline: **1,548 full + 10 partial / 35,523 cards across 343 sets**; checklist SHA-256 `45329b17371d8d572a65b1c4b12ce4f5a47ff30308a8ed363cd21db8feca1158`.
- Source: local Cockatrice `cards.xml`, last written `08/31/2026 07:27:00`, SHA-256 `bd001fcea73c3c3763d729fbad9d0528a456ca4a8219e76ad5ef55058e08d729`. This was necessary because the public Scryfall API had not indexed all four 2026 sets at capture time.
- Exclude registry-resolved full/partial names and RON-authored names, then deduplicate names in declared stratum order.
- Within each stratum, take the first **20** ascending `SHA-256("cockatrice-card-coverage-2026-09-current-v6\n<printing_uuid>")`, where `\n` is one LF.
- Preserve all **610 eligible rows**, printing UUIDs, hashes, ranks, card text, and classifications. This is an immutable snapshot.
- Rules authority is the [current Comprehensive Rules](https://media.wizards.com/2026/downloads/MagicCompRules%2020260819.txt), effective August 7, 2026, plus the official release notes recorded in the authority file.

| Stratum | Source cards | Eligible after exclusion/dedup | Selected |
|---|---:|---:|---:|
| TMT-common | 63 | 63 | 20 |
| TMT-uncommon | 55 | 55 | 20 |
| SOS-common | 82 | 82 | 20 |
| SOS-uncommon | 100 | 100 | 20 |
| MSH-common | 90 | 90 | 20 |
| MSH-uncommon | 100 | 100 | 20 |
| HOB-common | 65 | 65 | 20 |
| HOB-uncommon | 55 | 55 | 20 |

## Filed reusable backlog

The normalized filed capabilities are:

| Capability | Occurrences | Sole-gap candidates | Tracker |
|---|---:|---:|---|
| Sneak casting through the declare-blockers payment lifecycle | 7 | 7 | [#179](https://github.com/pizzaninja188/Cockatrice/issues/179) |
| Power-up entered-this-turn activation cost reduction | 6 | 5 | [#183](https://github.com/pizzaninja188/Cockatrice/issues/183) |
| Object-backed cast-cost options and Teamwork receipts | 5 | 5 | [#182](https://github.com/pizzaninja188/Cockatrice/issues/182) |
| Preparation cards and engine-authored prepared spell-copy actions | 4 | 3 | [#180](https://github.com/pizzaninja188/Cockatrice/issues/180) |
| Storied and the persistent enduring-story player designation | 4 | 2 | [#184](https://github.com/pizzaninja188/Cockatrice/issues/184) |
| Controller-relative permanent-left-battlefield turn facts for Disappear | 3 | 3 | [#189](https://github.com/pizzaninja188/Cockatrice/issues/189) |
| One-shot base power/toughness setters with source-relative values | 3 | 3 | [#187](https://github.com/pizzaninja188/Cockatrice/issues/187) |
| Per-spell actual mana-spent context for Opus and Increment | 3 | 2 | [#181](https://github.com/pizzaninja188/Cockatrice/issues/181) |
| Reusable Amass action with Army choice and subtype addition | 3 | 3 | [#186](https://github.com/pizzaninja188/Cockatrice/issues/186) |
| Attach a chosen Equipment to a chosen creature | 2 | 1 | [#188](https://github.com/pizzaninja188/Cockatrice/issues/188) |
| Restricted mana that may activate any ability | 2 | 2 | [#190](https://github.com/pizzaninja188/Cockatrice/issues/190) |
| Spell-cast triggers filtered by whether the spell targets a creature | 2 | 2 | [#191](https://github.com/pizzaninja188/Cockatrice/issues/191) |
| Saga lore progression and chapter-trigger lifecycle | 1 | 1 | [#185](https://github.com/pizzaninja188/Cockatrice/issues/185) |

The exact published issues are:

- [#179 Sneak casting through the declare-blockers payment lifecycle](https://github.com/pizzaninja188/Cockatrice/issues/179)
- [#180 Preparation cards and engine-authored prepared spell-copy actions](https://github.com/pizzaninja188/Cockatrice/issues/180)
- [#181 Per-spell actual mana-spent context for Opus and Increment](https://github.com/pizzaninja188/Cockatrice/issues/181)
- [#182 Object-backed cast-cost options and Teamwork receipts](https://github.com/pizzaninja188/Cockatrice/issues/182)
- [#183 Power-up entered-this-turn activation cost reduction](https://github.com/pizzaninja188/Cockatrice/issues/183)
- [#184 Storied and the persistent enduring-story player designation](https://github.com/pizzaninja188/Cockatrice/issues/184)
- [#185 Saga lore progression and chapter-trigger lifecycle](https://github.com/pizzaninja188/Cockatrice/issues/185)
- [#186 Reusable Amass action with Army choice and subtype addition](https://github.com/pizzaninja188/Cockatrice/issues/186)
- [#187 One-shot base power/toughness setters with source-relative values](https://github.com/pizzaninja188/Cockatrice/issues/187)
- [#188 Attach a chosen Equipment to a chosen creature](https://github.com/pizzaninja188/Cockatrice/issues/188)
- [#189 Controller-relative permanent-left-battlefield turn facts for Disappear](https://github.com/pizzaninja188/Cockatrice/issues/189)
- [#190 Restricted mana that may activate any ability](https://github.com/pizzaninja188/Cockatrice/issues/190)
- [#191 Spell-cast triggers filtered by whether the spell targets a creature](https://github.com/pizzaninja188/Cockatrice/issues/191)
- [#192 Author the next 20 fully supported current-Standard calibration cards](https://github.com/pizzaninja188/Cockatrice/issues/192)

The [gap map](card-coverage-calibration-2026-09-01-gaps.csv) accounts for **54 unique raw gap labels**: **14** map to the newly filed reusable issues and **40** remain deliberately deferred. Deferred labels include single-card hidden-zone, delayed-return, damage-observer, cost-relation, search-destination, and copy/visibility seams that need stronger reuse evidence or a separate cross-component design.

The data batch takes the first twenty `full_now` rows in sample order. If authoring exposes a false positive, split the missing primitive rather than adding card-specific Rust or weakening Oracle behavior.

## Evidence and verification

- [Per-card assessment](card-coverage-calibration-2026-09-01.csv)
- [Eligible population](card-coverage-calibration-2026-09-01-population.csv)
- [Gap disposition map](card-coverage-calibration-2026-09-01-gaps.csv)
- [Authority/source capture](card-coverage-calibration-2026-09-01-authority.json)
- [Manifest and SHA-256 values](card-coverage-calibration-2026-09-01-manifest.json)

The Windows calibration validator checks row identities, classifications, strata, selection hashes/ranks against the saved population, gap completeness/dispositions, filed issue numbers, and manifest checksums. A syntactic evidence check cannot prove every proposed encoding.

No Rust, protobuf, relay, or client behavior changed. Engine/C++ builds and hands-on two-client testing are N/A for this documentation/evidence/validator increment. All 14 issues were read back from GitHub on 2026-09-01 and verified open with exact draft titles/bodies plus `type:feature` and `priority:medium`.

## Interaction-checklist closeout

1. **Rules authority:** official Standard legality, release notes/update bulletins, Oracle text in the captured database, and the current CR drive the rows and proposed semantics.
2. **State/identity:** issue drafts preserve ObjectId plus generation, event-time LKI, stable ability/option identity, deterministic history, and player-set-generic state.
3. **Timing:** drafts cover priority windows, cast completion, trigger creation/resolution, turn resets, replacement outcomes, layers, and state-based actions as applicable.
4. **Players/legality:** owner/controller/actor distinctions, multiplayer sets, authoritative candidates, stale commands, and atomic payment rollback are explicit.
5. **Visibility:** preparation and Storied explicitly require public state propagation; Sneak uses engine-authored action/payment state. Hidden-zone identities remain requester-only.
6. **Propagation:** three filed issues are end-to-end; the Rust-only scopes explicitly mark protocol/relay/Qt/freeform N/A unless evidence requires a scope change.
7. **Verification/delivery:** every behavior issue requires focused red/green and the full affected-side Windows gates. The data issue also requires checklist/generator checks.

## MTG applicability

This calibration covers alternative casting and additional costs, preparation spell copies, actual mana spending, persistent player designations, Sagas, Amass, continuous-effect layers, attachments, event history, targeting, and restricted mana. Relevant concepts include CR 106, 115, 400.7, 509, 601–603, 611–615, 701, 702, 707, 714, and 722. It records compliance gaps without approximating missing mechanics and changes no gameplay.
