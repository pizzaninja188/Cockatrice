# Family: 07-damage-creature-lifegain (6 cards)

## Selection

Multi-clause composition, 6 cards. Revalidated by Oracle ID and printed name against the pinned
bulk, live registry, maps, `CARDS.md`, `queue-exclusions.txt` and `blocked.tsv`; see
`build/campaign/queue-v23/revalidation.tsv`.

## Exact cards

| Oracle ID | Card | Mana | Type line | Damage/life |
|---|---|---|---|---|
| 06d77149-52b6-4d2c-bfb9-1c25f6b1b0bb | Pharika's Cure | {B}{B} | Instant | 2 |
| 14363df6-ac0f-476a-bbd8-ce351de0babf | Essence Extraction | {1}{B}{B} | Instant | 3 |
| 1fd754fd-c053-4638-b379-46d903357b01 | Douse in Gloom | {2}{B} | Instant | 2 |
| 38ea22cd-2c5d-4f66-a111-207aca4c67c3 | Vicious Hunger | {B}{B} | Sorcery | 2 |
| bc59bf58-373d-4562-9a67-0721fcf2f098 | Last Kiss | {2}{B} | Instant | 2 |
| ff27ff37-96c0-41af-8881-a078e884e67b | Sorin's Thirst | {B}{B} | Instant | 2 |

## Copy-from and differences

`winters_intervention` for the exact `DamageTarget(... kind: Creature)` plus `GainLife` sequence and
targeting group. Damage and life are 2 or 3; Vicious Hunger is a sorcery, the other five are
instants. All six target a creature only, never a player.

## Rules evidence

CR 115.1 (target), 120.3 (damage), 119.3 (life gain) and 608.2b (an illegal target fizzles the
whole spell). The Pharika's Cure, Essence Extraction, Last Kiss and Sorin's Thirst rulings confirm
that no life is gained on a fizzle and that life is still gained when the damage is prevented.

## Canaries

- Simplest: Pharika's Cure ({B}{B}, 2 damage).
- Hardest: Essence Extraction ({1}{B}{B}, 3 damage).
- Every card is executed by the all-card nonlethal-damage, prevented-damage, stale-fizzle and
  noncreature-rejection tests.

## Semantic evidence

- `scenario damage_creature_lifegain_batch::damage_creature_lifegain_batch_canary_cards_deal_exact_damage_and_gain_life`
- `scenario damage_creature_lifegain_batch::damage_creature_lifegain_batch_cards_mark_nonlethal_damage_and_gain_life`
- `scenario damage_creature_lifegain_batch::damage_creature_lifegain_batch_cards_still_gain_life_when_damage_is_prevented`
- `scenario damage_creature_lifegain_batch::damage_creature_lifegain_batch_cards_fizzle_a_stale_target_without_life`
- `scenario damage_creature_lifegain_batch::damage_creature_lifegain_batch_cards_reject_a_noncreature_target_without_paying`
- `scenario damage_creature_lifegain_batch::damage_creature_lifegain_batch_cards_respect_instant_and_sorcery_timing`
- `mechanic_registry::damage_creature_lifegain_batch::damage_creature_lifegain_batch_maps_definitions`
- `mechanic_registry::damage_creature_lifegain_batch::damage_creature_lifegain_batch_fingerprints_match_the_pinned_oracle_text`

The six maps cover the single Oracle line, both effects and the target group. The prevented-damage
test casts Anti-Venom and proves life is still gained while the creature survives without damage.

## Focused evidence

- Intended canary red: `build/campaign/queue-v23/tmp/redC-damage_creature_lifegain_batch.log` (missing pharikas_cure, exit 101).
- Focused canary green: `build/campaign/queue-v23/tmp/greenC-damage_creature_lifegain_batch.log`, exit 0.
- Focused registry green: `build/campaign/queue-v23/tmp/regC5detail-damage_creature_lifegain_batch.log`, exit 0 (3 tests).
- Focused scenario green: `build/campaign/queue-v23/tmp/scnC5detail-damage_creature_lifegain_batch.log`, exit 0 (6 tests).
- Card data check: `build/campaign/queue-v23/tmp/carddata-c2.log`, exit 0.
- Review maps: `scripts/check-card-evidence.ps1 -Preparation` PASS (1822 maps) at
  `build/campaign/queue-v23/tmp/evidence-c.log`.

## Review and gate

Frozen in `build/campaign/review/queue-v23-batch-c-v2` (patch SHA-256
`a671cd9841c7ece680f401ac05036964b7d2fbc11b633a450f11e92fc17a1fb7`). Independent read-only review
approved the family (with delta approval after material findings were corrected). Batch queue-v23
batch C full gate `verify-991ead11-4b55-4081-a1d2-d7a4c600524e` passed exit 0, covering 07-damage-creature-lifegain..
09-etb-vanilla-tokens; commit `15e0b3a9f810dfeec954d331a81a108f4b04ee17`; remote `origin/card-campaign` verified at the same SHA.
