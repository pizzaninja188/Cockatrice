# Family: 01-flying-any-mana (3 cards)

## Selection

Multi-clause composition, 3 cards. Revalidated by Oracle ID and printed name against the pinned
bulk, live registry, maps, `CARDS.md`, `queue-exclusions.txt` and `blocked.tsv`; see
`build/campaign/queue-v23/revalidation.tsv`.

## Exact cards

| Oracle ID | Card | Mana | Type line | P/T |
|---|---|---|---|---|
| d3a0b660-358c-41bd-9cd2-41fbf3491b1a | Birds of Paradise | {G} | Creature — Bird | 0/1 |
| 143d7a32-c15d-4a1f-ad81-cf1e6aa92bf3 | Lotus Guardian | {7} | Artifact Creature — Dragon | 4/4 |
| 3a940dfa-a026-4969-8981-ef90cdfbe9ef | Ornithopter of Paradise | {2} | Artifact Creature — Thopter | 0/2 |

## Copy-from and differences

`lifespring_druid` for the five-option `{T}` mana ability; `welkin_tern` and the flying cohort for
the printed Flying keyword. The mana ability has no target and resolves immediately (CR
605.1a/605.3b); the creature still obeys summoning sickness for its `{T}` cost (CR 302.6).

The three cards differ only in mana cost, body and type line; every one offers exactly the five
colors and no other ability.

## Rules evidence

CR 702.9 (flying), 605.1a/605.3b (tap mana ability), 106.1a (five colors) and 302.6/602.5a
(untapped, not summoning-sick). No card in this cohort has a pinned ruling.

## Canaries

- Simplest: Birds of Paradise ({G}, 0/1).
- Hardest: Lotus Guardian ({7}, 4/4 artifact creature).
- Both exercise the five-color options; every card is executed by the all-card cast, summoning
  sickness, five-color, evasion and battlefield-persistence tests.

## Semantic evidence

- `scenario flying_any_mana_batch::flying_any_mana_batch_canary_cards_cast_and_offer_every_color`
- `scenario flying_any_mana_batch::flying_any_mana_batch_cards_offer_all_five_colors`
- `scenario flying_any_mana_batch::flying_any_mana_batch_cards_cast_with_exact_cost_and_body`
- `scenario flying_any_mana_batch::flying_any_mana_batch_cards_apply_summoning_sickness_to_the_tap_cost`
- `scenario flying_any_mana_batch::flying_any_mana_batch_cards_are_fliers_that_ground_creatures_cannot_block`
- `scenario flying_any_mana_batch::flying_any_mana_batch_cards_remain_on_the_battlefield_after_resolution`
- `mechanic_registry::flying_any_mana_batch::flying_any_mana_batch_maps_definitions`
- `mechanic_registry::flying_any_mana_batch::flying_any_mana_batch_fingerprints_match_the_pinned_oracle_text`

The three maps cover both Oracle lines and reference the shared actual-card scenarios plus the
registry checks. Broken-blocker and wrong-cost rejections are asserted; a stale mana source is not
a target and therefore has no fizzle path.

## Focused evidence

- Intended canary red: `build/campaign/queue-v23/tmp/red1.log` (missing birds_of_paradise, exit 101).
- Focused canary green: `build/campaign/queue-v23/tmp/green-flying_any_mana_batch.log`, exit 0.
- Focused registry green: `build/campaign/queue-v23/tmp/reg3-flying_any_mana_batch.log`, exit 0 (3 tests).
- Focused scenario green: `build/campaign/queue-v23/tmp/scn3-flying_any_mana_batch.log`, exit 0 (6 tests).
- Review maps: `scripts/check-card-evidence.ps1 -Preparation` PASS (1784 maps) at
  `build/campaign/queue-v23/tmp/evidence-a2.log`.

## Review and gate

Frozen in `build/campaign/review/queue-v23-batch-a-v3` (patch SHA-256
`53d7b8fc93fb96589e91b5a6bff627483f16dcbcd519b2f6a146436ab29517af`). Independent read-only
review approved the family after three delta rounds (false canary fixtures, vacuous evasion test,
unused import and missing stack/fizzle evidence were corrected). Batch queue-v23 batch A full gate
`verify-3d5e8584-e3f0-49c4-8141-553d37ac44f2` passed exit 0, covering 01-flying-any-mana..
03-mana-target-pump; commit `1f613082b68b5f99602c4944081d03e80c17f98e`; remote
`origin/card-campaign` verified at the same SHA.
