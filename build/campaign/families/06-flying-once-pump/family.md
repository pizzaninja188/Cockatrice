# Family: 06-flying-once-pump (4 cards)

## Selection

Queue-v26 Multi-clause composition, 4 cards. Revalidated by Oracle ID and printed name against the pinned bulk,
live registry, maps, `CARDS.md`, `queue-exclusions.txt` and `blocked.tsv`; see
`build/campaign/queue-v26/revalidation.tsv`.

Stable typed shape: Flying creature with `{M}: This creature gets +1/+0 until end of turn. Activate only once each turn.`

## Exact cards

| Oracle ID | Card | Mana | Type line | P/T | Oracle text |
|---|---|---|---|---|---|
| a82d9c9b-ec17-4f51-a6bf-5984ca4433e5 | Azimaet Drake | {2}{U} | Creature — Drake | 1/3 | Flying / {U}: This creature gets +1/+0 until end of turn. Activate only once each turn. |
| bd55bcc4-f0c5-4c09-8b3b-2e25e9fbd9ee | Drake Hatchling | {2}{U} | Creature — Drake | 1/3 | Flying / {U}: This creature gets +1/+0 until end of turn. Activate only once each turn. |
| bfbc12ec-ecc0-4e23-a9d1-d13babd99a46 | Spitting Drake | {3}{R} | Creature — Drake | 2/2 | Flying / {R}: This creature gets +1/+0 until end of turn. Activate only once each turn. |
| fd3bcc9b-7d84-478e-aef5-2e44610107c7 | Fire Drake | {1}{R}{R} | Creature — Drake | 1/2 | Flying / {R}: This creature gets +1/+0 until end of turn. Activate only once each turn. |

## Copy-from and differences

Copy `darkthicket_wolf`'s `PumpTarget(subject: Source)` with `ActivationLimit::PerTurn`; add the printed flying keyword.

## Rules evidence

CR 702.9a-b (flying), 602.1a/602.2 (activated ability and printed cost), 602.5b (once each turn), 611.2c/613.4c (pump) and 514.2 (cleanup). The cycle has no pinned ruling.

Pinned rulings are saved per Oracle ID under
`build/campaign/queue-v26/families/06-flying-once-pump/rulings/`.

## Canaries

- Simplest: Azimaet Drake (`azimaet_drake`).
- Hardest: Fire Drake (`fire_drake`).
- Every card in the family is executed by the shared all-card scenarios below.

## Semantic evidence

- `scenario flying_once_pump_batch::flying_once_pump_batch_cards_cast_with_exact_characteristics_and_flying`
- `scenario flying_once_pump_batch::flying_once_pump_batch_cards_activate_once_each_turn_and_reset_on_the_next_turn`
- `scenario flying_once_pump_batch::flying_once_pump_batch_cards_require_the_printed_activation_cost`
- `mechanic_registry::flying_once_pump_batch::flying_once_pump_batch_maps_definitions`
- `mechanic_registry::flying_once_pump_batch::flying_once_pump_batch_fingerprints_match_the_pinned_oracle_text`

## Focused evidence

- Intended simplest canary red: `build/verification-logs/20261116-213652-020-red-simplest-flying_once_pump.log` (missing definition, exit 101).
- Intended hardest canary red: `build/verification-logs/20261116-213845-388-red-hardest-flying_once_pump.log` (missing definition at the hardest card, exit 101).
- Focused all-card green: `build/verification-logs/20261116-221556-518-scenario-postfix-flying_once_pump.log` (exit 0).
- Registry green: `build/verification-logs/20261116-215415-217-registry-full-queue-v26.log` (full 911-test suite, exit 0).
- Review maps: `scripts/check-card-evidence.ps1 -Preparation` PASS for all 1,974 review maps (including all 47 queue-v26 maps) (card-evidence `2f26da74-67cc-4014-8ae0-ee05190d84f4`).
- Metadata refresh: only the 47 expected fingerprint rows and their `CARDS.md` checkboxes plus the `Implemented:` total changed.
- Formatting and focused lint: pending the batch preflight.

## Review and gate

Review required unpaid-activation evidence; the rejection and second-creature scope assertions were added and delta-approved.

The batch queue-v26 combined full gate `verify-00668ab2-d089-4ca9-85bb-c6a48f810a49` passed exit 0 on the unchanged nine-family tree, covering `01-pain-taplands` through `09-mill-two`. Code commit `b01d2c8e9e8f497e0cbb8418e94a2f7f628cefa3`. Metadata refresh changed only the 47 expected fingerprint rows, their `CARDS.md` checkboxes and the `Implemented:` total.
