# Card-coverage calibration — 2026-08-18

This second calibration measures the live ruled registry against one core-like set and two
mechanic-heavy recent premier sets. The committed row-level evidence is
[`card-coverage-calibration-2026-08-18.csv`](card-coverage-calibration-2026-08-18.csv).

## Result

| Classification | Cards | Rate |
|---|---:|---:|
| Fully implementable now | 64 | 35.6% |
| Partially implementable now | 55 | 30.6% |
| Full or partial | 119 | 66.1% |
| Blocked | 61 | 33.9% |
| **Total** | **180** | **100.0%** |

The combined rate is almost unchanged from the first calibration (66.1% versus 65.0%), but this
is not a paired trend comparison: the first sample used M19–M21 commons, while this sample
deliberately mixes a core-like control with two recent set-mechanic stress tests. The useful result
is the shape. Foundations is already 58.3% fully data-authorable; Duskmourn and Tarkir:
Dragonstorm expose the zone, face-state, casting-mode, and turn-event systems that now limit
coverage.

By stratum:

| Set | Full | Partial | Blocked | Total |
|---|---:|---:|---:|---:|
| Foundations (FDN) | 35 | 12 | 13 | 60 |
| Duskmourn: House of Horror (DSK) | 11 | 22 | 27 | 60 |
| Tarkir: Dragonstorm (TDM) | 18 | 21 | 21 | 60 |

## Reproducible sample

- **Rules capability baseline:** `master` at
  `459c0ab20fe078bd98af962196892be5892b7d94`, with 1,026 full and 17 partial card names in
  the generated registry checklist.
- **Oracle retrieval:** Scryfall card search, 2026-08-19 04:48 UTC, with
  `unique=cards&order=name` and one query per set:
  `set:<fdn|dsk|tdm> rarity:common game:paper is:booster lang:en`.
- **Input populations:** 95 FDN, 96 DSK, and 91 TDM English paper booster commons.
- **Implementation exclusion:** every name resolved by the live registry, whether full or
  partial, was excluded before sampling.
- **Deduplication:** cards are keyed by Scryfall `oracle_id`; a reprint in more than one target
  set is assigned to the earliest stratum in FDN → DSK → TDM order.
- **Eligible populations:** 77 FDN, 90 DSK, and 80 TDM cards after registry exclusion and
  cross-stratum deduplication.
- **Selection:** within each stratum, sort ascending by
  `SHA-256("cockatrice-card-coverage-2026-08-v2\n<oracle_id>")` and take the first 60. The CSV
  preserves Oracle ID, selection hash, mana cost, type line, Oracle text, keywords, classification,
  gap labels, rationale, and Scryfall URI for every row.
- **Rules source:** Wizards' 2026-08-08 [Comprehensive Rules](https://magic.wizards.com/en/rules),
  plus the official [Duskmourn mechanics](https://magic.wizards.com/en/news/feature/duskmourn-house-of-horror-mechanics)
  and [Tarkir: Dragonstorm release notes](https://magic.wizards.com/en/news/feature/tarkir-dragonstorm-release-notes).
  Exact mechanics checked include
  surveil (CR 701.25), manifest and manifest dread (CR 701.40 and 701.62), behold (CR 701.4),
  Ward (CR 702.21), cycling/typecycling (CR 702.29), kicker (CR 702.33), Rooms (CR 709.5),
  Omen cards (CR 720), harmonize (CR 702.180), and mobilize (CR 702.181).

## Classification rubric

- **`full_now`:** every functional Oracle clause is representable in RON with the current
  registry vocabulary. New card/token data is allowed; new Rust is not.
- **`partial_now`:** a faithful, useful subset remains after a precise bounded omission. A
  keyword body with its defining mechanic removed does not qualify.
- **`blocked`:** faithful useful behavior needs a new primitive, custom resolution algorithm,
  engine state, protocol/client contract, or physical-zone behavior.

Each full row has one implementation family. Each non-full row has one primary gap and optional
secondary gaps. A capability's immediate yield counts only cards for which it is the sole gap.
Oracle text controls each classification; current Comprehensive Rules and Wizards release notes
resolve mechanic timing and identity questions.

## Coverage shape

The ready full-data families are:

| Existing family | Full cards |
|---|---:|
| Simple triggered creatures | 29 |
| Activated creatures | 8 |
| Temporary abilities and combat modifiers | 5 |
| Equipment and Auras | 4 |
| Draw/discard sequences | 3 |
| Mana permanents | 2 |
| Combat tricks | 2 |
| Targeted removal | 2 |
| Value spells | 2 |
| Other existing families | 7 |

The leading measured gaps are:

| Missing capability | Occurrences | Sole-gap unlocks | Tracker |
|---|---:|---:|---|
| Planeswalker targets | 9 | 6 | Existing #72 |
| Surveil / ordered library partition | 9 | 5 | #96 |
| Conditional enters-tapped replacement | 7 | 7 | #97 |
| Manifest dread / face-down permanents | 7 | 5 | #98 |
| Omen cards | 5 | 2 | #100 |
| Once-per-turn activation limits | 5 | 3 | #102 |
| Cycling and graveyard activated abilities | 8 | 6 | #101 |
| Ward | 4 | 1 | #103 |
| Behold | 4 | 2 | #104 |
| Mobilize | 4 | 3 | #106 |
| Rooms and Room events/state | 9 | 8 | #99 |
| Graveyard card actions | 10 | 4 | #107 |

The CSV contains 63 raw primary/secondary labels. They are normalized into existing Issue #72
and Issues #96–#129. The tracker groups shared substrate rather than creating one issue per card:
for example, Cycling and renew share non-battlefield activation publication; threshold and
delirium share graveyard aggregate conditions; Rooms own both unlock state and their emitted
events; and second-card/second-spell/raid checks extend `TurnHistory` rather than adding card-local
state.

Full raw-label traceability:

| Issue | Gap labels |
|---|---|
| #72 | `planeswalker_targets` |
| #96 | `library_partition_choice`, `surveil` |
| #97 | `conditional_enters_tapped`, `global_entry_counter_replacement` |
| #98 | `face_down_permanents`, `manifest_dread` |
| #99 | `room_state_condition`, `room_unlock_trigger`, `rooms` |
| #100 | `omen` |
| #101 | `cycling`, `graveyard_activated_ability` |
| #102 | `once_per_turn_activation` |
| #103 | `ward` |
| #104 | `behold`, `kicker_cast_option` |
| #105 | `harmonize` |
| #106 | `mobilize` |
| #107 | `disjunctive_graveyard_filter`, `graveyard_card_exile`, `graveyard_result_condition`, `graveyard_to_library` |
| #108 | `graveyard_card_count_condition`, `graveyard_card_type_count` |
| #109 | `linked_exile` |
| #110 | `conditional_search_destination`, `library_filter_power`, `multi_zone_named_search`, `named_library_search`, `owner_library_placement_choice` |
| #111 | `attacked_this_turn_condition`, `spells_cast_this_turn_condition`, `turn_indexed_draw_trigger`, `turn_indexed_spell_cost_reduction`, `turn_indexed_spell_trigger` |
| #112 | `affinity`, `static_spell_cost_reduction`, `target_dependent_cost_reduction` |
| #113 | `characteristic_setting_and_ability_loss` |
| #114 | `disjunctive_target_filter` |
| #115 | `attached_combat_damage_trigger`, `attached_damage_trigger`, `conditional_attached_modifier` |
| #116 | `conditional_effect_branch`, `conditional_static_scope` |
| #117 | `fight` |
| #118 | `targeted_combat_damage_prevention` |
| #119 | `self_block_restriction` |
| #120 | `return_source_to_hand`, `sacrifice_cost_exclude_source` |
| #121 | `player_set_discard_result`, `player_set_draw` |
| #122 | `paid_card_characteristics_condition` |
| #123 | `impulse_play` |
| #124 | `counter_types`, `stun_counters` |
| #125 | `death_exile_replacement` |
| #126 | `private_hand_exile` |
| #127 | `trigger_event_power_filter` |
| #128 | `combat_phase_trigger`, `second_main_phase_trigger` |
| #129 | `special_action_mana_restriction` |

## Next-increment decision

As in the first calibration, compare capped immediate yield:

- **Data yield:** `min(20, 29 simple triggered creatures) = 20`.
- **Primitive yield:** `min(20, 8 entry-replacement sole-gap cards) = 8` (seven conditional
  enters-tapped lands plus Dragonstorm Globe's filtered global counter replacement).

The data route wins 20 to 8 and avoids protocol, relay, Qt, hidden-information, and physical-zone
changes. Issue #130 therefore batches the first 20 fully implementable triggered creatures in
selection-hash order:

1. Watcher of the Wayside
2. Sanguine Syphoner
3. Burglar Rat
4. Flesh Burrower
5. Prideful Parent
6. Apothecary Stomper
7. Elfsworn Giant
8. Helpful Hunter
9. Kin-Tree Nurturer
10. Dusyut Earthcarver
11. Sandskitter Outrider
12. Beast-Kin Ranger
13. Dwynen's Elite
14. Humbling Elder
15. Reputable Merchant
16. Delta Bloodflies
17. Iceridge Serpent
18. Felidar Savior
19. Infestation Sage
20. Summit Intimidator

## Interaction-checklist closeout

- **Rules authority:** Oracle data is preserved row by row; the current official CR and Wizards
  mechanic notes were used for non-obvious timing and zone behavior. No mechanic is approximated
  as implemented.
- **State and identity:** capability issues keep tricerules authoritative and call out hidden-zone,
  object-generation, face, attachment, and zone-action identity where material.
- **Timing:** issues distinguish replacement effects, triggers, special actions, cast-cost
  selection, resolution-time choices, and delayed actions instead of merging them into generic
  text execution.
- **Players and legality:** new scopes are player-set-generic; owner/controller/chooser roles and
  illegal, stale, or unavailable choices remain engine decisions.
- **Visibility:** private library, hand, and face-down choices require per-player events and
  fail-closed relay redaction; public state mechanics do not expose hidden candidates.
- **Propagation:** issues that need visible actions explicitly include protobuf, Servatrice,
  ruled-client, and two-client acceptance. Rust/data-only issues say so.
- **Verification:** this calibration itself is documentation/data only. Each implementation issue
  requires red/green scenario coverage and the full affected-side gates from
  `AGENT-VERIFICATION.md`; visible or physical identity changes require the real two-client flow.

## MTG applicability

This audit covers casting and activation zones, costs, triggers, replacement effects, linked
objects, face state, hidden-zone choices, counters, continuous characteristics, and combat. It
changes no game behavior; it measures current compliance and records missing capabilities without
approximating them.
