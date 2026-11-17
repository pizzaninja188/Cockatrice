# Family: 05-other-anthems (3 cards)

## Selection

Queue-v26 Simple single-primitive cohort, 3 cards. Revalidated by Oracle ID and printed name against the pinned bulk,
live registry, maps, `CARDS.md`, `queue-exclusions.txt` and `blocked.tsv`; see
`build/campaign/queue-v26/revalidation.tsv`.

Stable typed shape: Controller-scoped layer-7c `AnthemPt` with `exclude_self`.

## Exact cards

| Oracle ID | Card | Mana | Type line | P/T | Oracle text |
|---|---|---|---|---|---|
| 1b1214e9-1581-4008-b3d4-8b7af6b092fd | Veteran Armorer | {1}{W} | Creature — Human Soldier | 2/2 | Other creatures you control get +0/+1. |
| 2cc439e8-d112-44e6-bc5a-6e99333c519a | Benalish Marshal | {W}{W}{W} | Creature — Human Knight | 3/3 | Other creatures you control get +1/+1. |
| c0856fea-cbdc-427b-ae00-bfd9311f284f | Blessed Orator | {3}{W} | Creature — Human Cleric | 1/4 | Other creatures you control get +0/+1. |

## Copy-from and differences

Copy `corsair_captain`'s controller-scoped `AnthemPt` filter and clear the subtype, because these cards pump every other creature you control.

## Rules evidence

CR 604.2/611.3b (the continuous effect is active while the source remains on the battlefield), 613.4c (layer 7c) and 109.5 ("you" is the source's controller); the printed "other" excludes the source. Benalish Marshal's pinned ruling confirms that marked damage can become lethal when the anthem leaves.

Pinned rulings are saved per Oracle ID under
`build/campaign/queue-v26/families/05-other-anthems/rulings/`.

## Canaries

- Simplest: Veteran Armorer (`veteran_armorer`).
- Hardest: Blessed Orator (`blessed_orator`).
- Every card in the family is executed by the shared all-card scenarios below.

## Semantic evidence

- `scenario other_anthems_batch::other_anthems_batch_cards_pump_only_other_creatures_you_control`
- `scenario other_anthems_batch::other_anthems_batch_cards_apply_to_later_entries_and_stop_when_the_source_leaves`
- `scenario other_anthems_batch::other_anthems_batch_cards_stack_with_a_second_copy`
- `mechanic_registry::other_anthems_batch::other_anthems_batch_maps_definitions`
- `mechanic_registry::other_anthems_batch::other_anthems_batch_fingerprints_match_the_pinned_oracle_text`

## Focused evidence

- Intended simplest canary red: `build/verification-logs/20261116-213650-345-red-simplest-other_anthems.log` (missing definition, exit 101).
- Intended hardest canary red: `build/verification-logs/20261116-213843-641-red-hardest-other_anthems.log` (missing definition at the hardest card, exit 101).
- Focused all-card green: `build/verification-logs/20261116-221554-785-scenario-postfix-other_anthems.log` (exit 0).
- Registry green: `build/verification-logs/20261116-215415-217-registry-full-queue-v26.log` (full 911-test suite, exit 0).
- Review maps: `scripts/check-card-evidence.ps1 -Preparation` PASS for all 1,974 review maps (including all 47 queue-v26 maps) (card-evidence `2f26da74-67cc-4014-8ae0-ee05190d84f4`).
- Metadata refresh: only the 47 expected fingerprint rows and their `CARDS.md` checkboxes plus the `Implemented:` total changed.
- Formatting and focused lint: pending the batch preflight.

## Review and gate

Review required the static-ability citations to use CR 604.2/611.3b/613.4c/109.5; corrected and delta-approved.

The batch queue-v26 combined full gate `verify-00668ab2-d089-4ca9-85bb-c6a48f810a49` passed exit 0 on the unchanged nine-family tree, covering `01-pain-taplands` through `09-mill-two`. Code commit `b01d2c8e9e8f497e0cbb8418e94a2f7f628cefa3`; remote `b01d2c8e9e8f497e0cbb8418e94a2f7f628cefa3`. Metadata refresh changed only the 47 expected fingerprint rows, their `CARDS.md` checkboxes and the `Implemented:` total.
