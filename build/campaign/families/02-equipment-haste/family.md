# Family: 02-equipment-haste (3 cards)

## Selection

Simple single-primitive cohort, 3 cards. Revalidated by Oracle ID and printed name against the
pinned bulk, live registry, maps, `CARDS.md`, `queue-exclusions.txt` and `blocked.tsv`; see
`build/campaign/queue-v23/revalidation.tsv`.

## Exact cards

| Oracle ID | Card | Mana | Type line | Bonus |
|---|---|---|---|---|
| 6f6601bc-cb12-4054-9027-e4a0ebdae1a3 | Boots of Speed | {R} | Artifact — Equipment | +1/+0 and haste |
| 8c2d2888-38be-41f9-aaf2-8337e87d2aed | Crystal Slipper | {1}{R} | Artifact — Equipment | +1/+0 and haste |
| bd4ad383-8c40-4dda-bc9c-ba8a001d6882 | Strider Harness | {3} | Artifact — Equipment | +1/+1 and haste |

## Copy-from and differences

`accorders_shield` for `AttachedModifier` with a keyword and `Torch Gauntlet`/`Dueling Rapier`
for the Equipment/equip shape. No entry trigger and no flash.

Boots of Speed and Crystal Slipper grant +1/+0; Strider Harness grants +1/+1. All three cost {1}
to equip and have no other ability.

## Rules evidence

CR 301.5/702.6 (Equipment and equip), 613.4c (attached modifier), 702.10 (haste) and 704.5n (an
Equipment unattaches when its creature leaves). The Crystal Slipper and Strider Harness rulings
confirm that haste must still be present at the declare-attackers step.

## Canaries

- Simplest: Boots of Speed ({R}, +1/+0).
- Hardest: Strider Harness ({3}, +1/+1).
- Every card is executed by the all-card cast/equip, unattached, target-scope, unattach and
  printed pump/haste tests.

## Semantic evidence

- `scenario equipment_haste_batch::equipment_haste_batch_canary_cards_cast_equip_and_grant_haste`
- `scenario equipment_haste_batch::equipment_haste_batch_cards_grant_printed_pump_and_haste`
- `scenario equipment_haste_batch::equipment_haste_batch_cards_cast_and_equip_with_exact_cost`
- `scenario equipment_haste_batch::equipment_haste_batch_cards_unattached_equipment_grants_nothing`
- `scenario equipment_haste_batch::equipment_haste_batch_cards_equip_only_creatures_you_control`
- `scenario equipment_haste_batch::equipment_haste_batch_cards_remain_and_unattach_when_the_creature_leaves`
- `mechanic_registry::equipment_haste_batch::equipment_haste_batch_maps_definitions`
- `mechanic_registry::equipment_haste_batch::equipment_haste_batch_fingerprints_match_the_pinned_oracle_text`

The three maps cover both Oracle lines and reference the shared actual-card scenarios plus the
registry checks. Equip timing/fizzle reuse the generic `equipment_batch` regressions.

## Focused evidence

- Intended canary red: `build/campaign/queue-v23/tmp/red2.log` (missing boots_of_speed, exit 101).
- Focused canary green: `build/campaign/queue-v23/tmp/green-equipment_haste_batch.log`, exit 0.
- Focused registry green: `build/campaign/queue-v23/tmp/reg3-equipment_haste_batch.log`, exit 0 (3 tests).
- Focused scenario green: `build/campaign/queue-v23/tmp/scn3-equipment_haste_batch.log`, exit 0 (6 tests).
- Review maps: `scripts/check-card-evidence.ps1 -Preparation` PASS (1784 maps) at
  `build/campaign/queue-v23/tmp/evidence-a2.log`.

## Review and gate

Frozen in `build/campaign/review/queue-v23-batch-a-v3` (patch SHA-256
`53d7b8fc93fb96589e91b5a6bff627483f16dcbcd519b2f6a146436ab29517af`). Independent read-only
review approved the family after delta rounds added the all-card pump/haste evidence and removed a
false canary map fixture. Batch queue-v23 batch A full gate
`verify-3d5e8584-e3f0-49c4-8141-553d37ac44f2` passed exit 0, covering
01-flying-any-mana..03-mana-target-pump; commit `1f613082b68b5f99602c4944081d03e80c17f98e`;
remote `origin/card-campaign` verified at the same SHA.
