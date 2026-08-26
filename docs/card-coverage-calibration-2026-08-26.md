# Card-coverage calibration — 2026-08-26

This third calibration measures the live ruled registry against three recent, previously
unsampled premier sets: Edge of Eternities, Magic: The Gathering | Avatar: The Last Airbender,
and Lorwyn Eclipsed. The committed row-level evidence is
[`card-coverage-calibration-2026-08-26.csv`](card-coverage-calibration-2026-08-26.csv).

## Result

| Classification | Cards | Rate |
|---|---:|---:|
| Fully implementable now | 90 | 50.0% |
| Partially implementable now | 72 | 40.0% |
| Full or partial | 162 | 90.0% |
| Blocked | 18 | 10.0% |
| **Total** | **180** | **100.0%** |

This is a capability snapshot, not a trend line. The earlier calibrations used different sets,
different immutable samples, and older registry baselines. The useful result is the current gap
shape: ordinary cards now compose broadly from shipped data vocabulary, while new payment,
designation, event-memory, and continuous-characteristic mechanics form the remaining clusters.

By stratum:

| Set | Full | Partial | Blocked | Total |
|---|---:|---:|---:|---:|
| Edge of Eternities (EOE) | 29 | 26 | 5 | 60 |
| Avatar: The Last Airbender (TLA) | 36 | 17 | 7 | 60 |
| Lorwyn Eclipsed (ECL) | 25 | 29 | 6 | 60 |

## Reproducible sample

- **Rules capability baseline:** `master` at
  `384e1c448b5fd8a2a0bb2a8197569e46d8992d30`, with 1,384 full and 6 partial card names in
  the generated registry checklist.
- **Snapshot immutability:** this report and CSV are historical evidence at that capability
  baseline. Later implementation belongs in [`tricerules/CARDS.md`](../tricerules/CARDS.md) or
  a separately named calibration; do not rewrite these classifications.
- **Oracle retrieval:** Scryfall card search, 2026-08-26 05:24 UTC, with
  `unique=cards&order=name` and one query per set:
  `set:<eoe|tla|ecl> rarity:common game:paper is:booster lang:en`.
- **Input populations:** 86 EOE, 96 TLA, and 86 ECL English paper booster commons.
- **Implementation exclusion:** every name resolved by the live registry, whether full or
  partial, was excluded before sampling.
- **Deduplication:** cards are keyed by Scryfall `oracle_id`; a reprint in more than one target
  set is assigned to the earliest stratum in EOE → TLA → ECL order.
- **Eligible populations:** 78 EOE, 91 TLA, and 79 ECL cards after registry exclusion and
  cross-stratum deduplication.
- **Selection:** within each stratum, sort ascending by
  `SHA-256("cockatrice-card-coverage-2026-08-v3\n<oracle_id>")` and take the first 60. The CSV
  preserves Oracle ID, selection hash, mana cost, type line, Oracle text, keywords,
  classification, gap labels, rationale, and Scryfall URI for every row.
- **Rules sources:** Wizards' Comprehensive Rules effective 2026-08-07, the official
  [Edge of Eternities mechanics](https://magic.wizards.com/en/news/feature/edge-of-eternities-mechanics),
  [Avatar mechanics](https://magic.wizards.com/en/news/feature/avatar-the-last-airbender-mechanics),
  and [Lorwyn Eclipsed release notes](https://magic.wizards.com/en/news/feature/lorwyn-eclipsed-release-notes).
  Exact concepts checked include Convoke (CR 702.51), Station and station cards (CR 702.184 and
  721), Warp (CR 702.185), Airbend/Earthbend/Waterbend (CR 701.65–701.67), Blight (CR 701.68),
  Changeling, Exhaust, Firebending, Wither, typecycling, delayed triggers, and layer interactions.

## Classification rubric

- **`full_now`:** every functional Oracle clause is representable in RON with the current
  registry vocabulary. New card or token data is allowed; new Rust is not.
- **`partial_now`:** a faithful, useful subset remains after a precise bounded omission. A
  creature reduced to only its body does not qualify.
- **`blocked`:** faithful useful behavior needs a new primitive, engine state, resolution
  algorithm, protocol/client contract, or physical-zone behavior.

Each full row has one implementation family. Each non-full row has one primary gap and optional
secondary gaps. A capability's sole-gap yield counts a card only when every observed gap on that
card maps to the same capability issue. Oracle text controls each classification; current rules
and official release notes resolve mechanic timing, payment, and identity questions.

## Coverage shape

The largest ready full-data families are:

| Existing family | Full cards |
|---|---:|
| Activated permanents | 9 |
| Token makers | 8 |
| Combat and landfall triggers | 7 |
| Utility lands | 7 |
| Keyword and typecycling creatures | 6 |
| Targeted and modal removal | 6 |
| Damage and counter creatures | 5 |
| Value spells | 4 |
| Combat tricks | 3 |
| Conditional static creatures | 3 |
| Other existing families | 32 |

The normalized capability gaps are:

| Capability | Occurrences | Sole-gap unlocks | Tracker |
|---|---:|---:|---|
| Richer characteristic, aggregate, and turn-history predicates | 18 | 16 | [#158](https://github.com/pizzaninja188/Cockatrice/issues/158) |
| Blight payments | 8 | 6 | [#153](https://github.com/pizzaninja188/Cockatrice/issues/153) |
| Attachment entry and source-relative zone actions | 8 | 5 | [#155](https://github.com/pizzaninja188/Cockatrice/issues/155) |
| Tapped, leave, and sacrifice observers | 8 | 8 | [#156](https://github.com/pizzaninja188/Cockatrice/issues/156) |
| Warp and Void | 7 | 7 | [#148](https://github.com/pizzaninja188/Cockatrice/issues/148) |
| Changeling | 7 | 6 | [#154](https://github.com/pizzaninja188/Cockatrice/issues/154) |
| Typed resolution receipts and result cohorts | 7 | 6 | [#159](https://github.com/pizzaninja188/Cockatrice/issues/159) |
| Counter manipulation, prohibition, and Wither | 6 | 4 | [#157](https://github.com/pizzaninja188/Cockatrice/issues/157) |
| Convoke | 4 | 4 | [#145](https://github.com/pizzaninja188/Cockatrice/issues/145) |
| Selectable tap-payment substrate | 3 | 3 | [#144](https://github.com/pizzaninja188/Cockatrice/issues/144) |
| Waterbend | 3 | 1 | [#146](https://github.com/pizzaninja188/Cockatrice/issues/146) |
| Earthbend | 3 | 2 | [#150](https://github.com/pizzaninja188/Cockatrice/issues/150) |
| Firebending | 3 | 1 | [#151](https://github.com/pizzaninja188/Cockatrice/issues/151) |
| Blocking-specific filters and restrictions | 3 | 2 | [#160](https://github.com/pizzaninja188/Cockatrice/issues/160) |
| Existing characteristic-setting layers | 3 | 2 | [#113](https://github.com/pizzaninja188/Cockatrice/issues/113) |
| Station and charge thresholds | 2 | 2 | [#147](https://github.com/pizzaninja188/Cockatrice/issues/147) |
| Exhaust lifetime limits | 2 | 0 | [#152](https://github.com/pizzaninja188/Cockatrice/issues/152) |
| Controller-relative turn hooks | 2 | 2 | [#161](https://github.com/pizzaninja188/Cockatrice/issues/161) |
| Tapped token creation | 2 | 2 | [#162](https://github.com/pizzaninja188/Cockatrice/issues/162) |
| Airbend | 1 | 1 | [#149](https://github.com/pizzaninja188/Cockatrice/issues/149) |

The shared tap-payment substrate is separate from Convoke, Waterbend, and Station so ordinary
cards can land independently and each mechanic retains a bounded rules contract. Issues #145–#147
depend on #144. Existing Issue #113 absorbs the sampled temporary base-P/T, animation, and color
gaps instead of receiving a duplicate tracker item.

Full raw-label traceability:

| Issue | Gap labels |
|---|---|
| #113 | `temporary_animation`, `temporary_base_pt`, `temporary_color_change` |
| #144 | `tap_permanent_ability_cost`, `tap_permanent_resolution_cost` |
| #145 | `convoke` |
| #146 | `waterbend` |
| #147 | `charge_counters`, `station` |
| #148 | `void_history`, `warp` |
| #149 | `airbend` |
| #150 | `earthbend` |
| #151 | `firebending` |
| #152 | `exhaust_once_per_object` |
| #153 | `blight` |
| #154 | `changeling` |
| #155 | `attached_object_zone_effect`, `equipment_enters_attachment`, `return_graveyard_source_to_hand` |
| #156 | `attached_becomes_tapped_trigger`, `becomes_tapped_trigger`, `sacrifice_event_trigger`, `self_leaves_battlefield_trigger` |
| #157 | `counter_removal`, `counter_transfer`, `prohibit_counter_placement`, `wither` |
| #158 | `attacked_with_subtype_this_turn`, `battlefield_union_count_condition`, `cards_drawn_this_turn_condition`, `conditional_activated_cost_reduction`, `count_scaled_static_pt`, `distinct_name_battlefield_count`, `graveyard_subtype_condition`, `permanent_entered_this_turn_condition`, `source_counter_condition`, `spell_mana_value_trigger_filter`, `tapped_creature_count_condition`, `target_damage_history_condition`, `trigger_object_subtype_filter` |
| #159 | `cast_cost_conditional_search_filter`, `library_second_from_top`, `milled_result_choice`, `multi_card_impulse_play`, `soft_counter_payment_result`, `target_characteristic_conditional_effect`, `target_post_effect_condition` |
| #160 | `blocker_count_restriction`, `blocker_power_restriction`, `blocking_target_filter` |
| #161 | `delayed_trigger_on_next_controller_turn`, `other_player_untap_trigger` |
| #162 | `tapped_token_creation` |

## Next-increment decision

Compare capped immediate yield:

- **Data yield:** `min(20, 90 full_now cards) = 20`.
- **Capability yield:** `min(20, 16 predicate sole-gap cards) = 16`.

The data route wins 20 to 16 and changes only Rust card/token data through shipped contracts.
[Issue #163](https://github.com/pizzaninja188/Cockatrice/issues/163) therefore batches the first
20 fully implementable cards in selection-hash order:

1. Galactic Wayfarer
2. Sun-Blessed Peak
3. Biosynthic Burst
4. Glider Kids
5. Pretending Poxbearers
6. Radiant Strike
7. Cloudsculpt Technician
8. Mistmeadow Council
9. Octopus Form
10. Rig for War
11. Boggart Prankster
12. Azula Always Lies
13. Surly Farrier
14. Otter-Penguin
15. Crossroads Watcher
16. Thawbringer
17. Abandon Attachments
18. Rowdy Snowballers
19. Wandering Musicians
20. Mongoose Lizard

## Interaction-checklist closeout

- **Rules authority:** every row preserves current Oracle text. Official current rules and set
  mechanic notes were checked for costs, alternative casting, delayed identity, designation,
  counter, and layer behavior; no mechanic is approximated as implemented.
- **State and identity:** issues keep tricerules as the sole rules writer and explicitly bind
  delayed permissions, attachments, counter cohorts, and zone actions to object ID plus
  zone-change generation. Servatrice remains the physical-card/redaction owner and Qt remains a
  display of published state.
- **Timing and ordering:** issues identify cast/activation/resolution payment boundaries,
  declared-attack events, untap/end-step hooks, APNAP trigger collection, SBA timing, replacement
  ordering, layer evaluation, and parked-resolution continuation where applicable.
- **Players and legality:** conditions and recipients remain player-set-generic. Owner,
  controller, chooser, affected player, and defending player are kept distinct; illegal, stale,
  duplicate, and no-longer-payable commands fail atomically.
- **Visibility:** hidden-zone casting permissions and result cohorts are per-player and fail
  closed; public battlefield costs expose only engine-authored candidates. New broadcast fields
  must use the existing visibility classification and reflection gate.
- **Propagation:** every visible payment, permission, characteristic, attachment, or physical-zone
  issue calls out protobuf, Servatrice, Qt, and two-client acceptance. Rust-only issues state when
  those layers are N/A. Every UI path remains ruled-gated so freeform behavior is unchanged.
- **Verification:** each implementation issue requires red/green focused coverage and the full
  affected-side gates from `AGENT-VERIFICATION.md`. This calibration changes only documentation,
  evidence, and the tracker, so hands-on UI testing is N/A and was not performed.

## MTG applicability

This audit covers alternative and additional costs, tap-based payment, triggered-event memory,
zone-linked casting permissions, counters and replacement effects, continuous characteristics,
attachments, combat restrictions, and delayed actions. It changes no game behavior; it measures
current compliance and records missing capabilities without approximation.
