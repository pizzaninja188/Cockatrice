# Family: 08-etb-pump-flying (3 cards)

## Selection

Queue-v26 Multi-clause composition, 3 cards. Revalidated by Oracle ID and printed name against the pinned bulk,
live registry, maps, `CARDS.md`, `queue-exclusions.txt` and `blocked.tsv`; see
`build/campaign/queue-v26/revalidation.tsv`.

Stable typed shape: Flying creature whose entry trigger gives one chosen creature `+N/+N` and flying until end of turn.

## Exact cards

| Oracle ID | Card | Mana | Type line | P/T | Oracle text |
|---|---|---|---|---|---|
| 0ce31ff1-6cfe-4b34-bd02-7d727934e754 | Welkin Guide | {4}{W} | Creature — Bird Cleric | 2/2 | Flying / When this creature enters, target creature gets +2/+2 and gains flying until end of turn. |
| 9a17452d-3adb-42ec-99f8-0083740cefa0 | Arborea Pegasus | {3}{W} | Creature — Pegasus | 2/3 | Flying / When this creature enters, target creature gets +1/+1 and gains flying until end of turn. |
| f36e316d-c0ad-414d-8cf0-b1c9ea779e03 | Battleflight Eagle | {4}{W} | Creature — Bird | 2/2 | Flying / When this creature enters, target creature gets +2/+2 and gains flying until end of turn. |

## Copy-from and differences

Compose `due_diligence`'s ordered `PumpTarget` + `GrantKeywords` with `gale_swooper`'s entry target; only the printed pump amount and body differ.

## Rules evidence

CR 702.9a-b (flying), 603.6a (enters-the-battlefield trigger), 115.1/608.2b (target), 611.2a (until-end-of-turn duration), 613.1f (layer-6 keyword grant) and 613.4c (layer-7c pump). Battleflight Eagle's pinned ruling confirms the ability is mandatory and may target the source itself.

Pinned rulings are saved per Oracle ID under
`build/campaign/queue-v26/families/08-etb-pump-flying/rulings/`.

## Canaries

- Simplest: Welkin Guide (`welkin_guide`).
- Hardest: Battleflight Eagle (`battleflight_eagle`).
- Every card in the family is executed by the shared all-card scenarios below.

## Semantic evidence

- `scenario etb_pump_flying_batch::etb_pump_flying_batch_cards_pump_and_grant_flying_to_the_chosen_creature`
- `scenario etb_pump_flying_batch::etb_pump_flying_batch_cards_can_target_the_source_or_an_opponents_creature`
- `scenario etb_pump_flying_batch::etb_pump_flying_batch_cards_reject_a_declined_mandatory_target`
- `scenario etb_pump_flying_batch::etb_pump_flying_batch_cards_end_the_pump_and_flying_at_cleanup`
- `scenario etb_pump_flying_batch::etb_pump_flying_batch_cards_apply_the_pump_only_when_the_trigger_resolves`
- `scenario etb_pump_flying_batch::etb_pump_flying_batch_cards_fizzle_when_the_only_target_leaves`
- `mechanic_registry::etb_pump_flying_batch::etb_pump_flying_batch_maps_definitions`
- `mechanic_registry::etb_pump_flying_batch::etb_pump_flying_batch_fingerprints_match_the_pinned_oracle_text`

## Focused evidence

- Intended simplest canary red: `build/verification-logs/20261116-213655-400-red-simplest-etb_pump_flying.log` (missing definition, exit 101).
- Intended hardest canary red: `build/verification-logs/20261116-213849-208-red-hardest-etb_pump_flying.log` (missing definition at the hardest card, exit 101).
- Focused all-card green: `build/verification-logs/20261116-223834-410-scenario-v3-formatted-etb_pump_flying.log` (exit 0).
- Registry green: `build/verification-logs/20261116-215415-217-registry-full-queue-v26.log` (full 911-test suite, exit 0).
- Review maps: `scripts/check-card-evidence.ps1 -Preparation` PASS for all 1,974 review maps (including all 47 queue-v26 maps) (card-evidence `2f26da74-67cc-4014-8ae0-ee05190d84f4`).
- Metadata refresh: only the 47 expected fingerprint rows and their `CARDS.md` checkboxes plus the `Implemented:` total changed.
- Formatting and focused lint: pending the batch preflight.

## Review and gate

Review required pre-resolution and stale-target evidence; added with a non-flying bystander assertion and delta-approved.

The batch queue-v26 combined full gate `verify-00668ab2-d089-4ca9-85bb-c6a48f810a49` passed exit 0 on the unchanged nine-family tree, covering `01-pain-taplands` through `09-mill-two`. Code commit `b01d2c8e9e8f497e0cbb8418e94a2f7f628cefa3`; remote `b01d2c8e9e8f497e0cbb8418e94a2f7f628cefa3`. Metadata refresh changed only the 47 expected fingerprint rows, their `CARDS.md` checkboxes and the `Implemented:` total.
