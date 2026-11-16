# Family: 04-haste-self-pump (3 cards)

## Selection

Multi-clause composition, 3 cards. Revalidated by Oracle ID and printed name against the pinned
bulk, live registry, maps, `CARDS.md`, `queue-exclusions.txt` and `blocked.tsv`; see
`build/campaign/queue-v23/revalidation.tsv`.

## Exact cards

| Oracle ID | Card | Mana | Type line | P/T | Activation | Pump |
|---|---|---|---|---|---|---|
| 18bcf4c5-7abc-4ca5-b805-4bbebf4e433c | Crypt Ripper | {2}{B}{B} | Creature — Shade | 2/2 | {B} | +1/+1 |
| 86b33d25-6123-439c-9026-a4e835b48562 | Furnace Spirit | {2}{R} | Creature — Spirit | 1/1 | {R} | +1/+0 |
| 50bc464e-1c8d-455c-a870-ddbbc4618343 | Pyre Charger | {R}{R} | Creature — Elemental Warrior | 1/1 | {R} | +1/+0 |

## Copy-from and differences

`nightwing_shade` for `PumpTarget(... subject: Source)` in an activated ability and `flamekin_brawler`
for the exact pump values; the printed haste keyword comes from the shipped haste cohort.

Crypt Ripper pumps toughness as well as power; Furnace Spirit and Pyre Charger pump power only.
All three are summoning-sick on entry and can attack immediately only because of the printed haste.

## Rules evidence

CR 702.10 (haste), 602.2 (activate), 611.2c/613.4c (until-end-of-turn pump) and 514.2 (cleanup). No
card in this cohort has a pinned ruling.

## Canaries

- Simplest: Pyre Charger ({R}{R}, 1/1, {R}: +1/+0).
- Hardest: Crypt Ripper ({2}{B}{B}, 2/2, {B}: +1/+1).
- Every card is executed by the all-card cast, pump, stack, exact-payment, rejection, cleanup and
  immediate-attack tests.

## Semantic evidence

- `scenario haste_self_pump_batch::haste_self_pump_batch_canary_cards_cast_and_apply_the_printed_keyword`
- `scenario haste_self_pump_batch::haste_self_pump_batch_cards_cast_with_exact_characteristics`
- `scenario haste_self_pump_batch::haste_self_pump_batch_cards_activate_only_the_source_with_the_printed_pump`
- `scenario haste_self_pump_batch::haste_self_pump_batch_cards_stack_repeated_activations`
- `scenario haste_self_pump_batch::haste_self_pump_batch_cards_reject_activation_without_the_exact_cost`
- `scenario haste_self_pump_batch::haste_self_pump_batch_cards_expire_the_pump_at_cleanup`
- `scenario haste_self_pump_batch::haste_self_pump_batch_cards_attack_immediately_after_being_cast`
- `mechanic_registry::haste_self_pump_batch::haste_self_pump_batch_maps_definitions`
- `mechanic_registry::haste_self_pump_batch::haste_self_pump_batch_fingerprints_match_the_pinned_oracle_text`

The three maps cover both Oracle lines and reference the shared actual-card scenarios plus the
registry checks. A rejected activation is asserted to leave the body, pool and stack unchanged.

## Focused evidence

- Intended canary red: `build/campaign/queue-v23/tmp/redB-haste_self_pump_batch.log` (missing crypt_ripper, exit 101).
- Focused canary green: `build/campaign/queue-v23/tmp/greenB-haste_self_pump_batch.log`, exit 0.
- Focused registry green: `build/campaign/queue-v23/tmp/regBdetail-haste_self_pump_batch.log`, exit 0 (3 tests).
- Focused scenario green: `build/campaign/queue-v23/tmp/scnBdetail-haste_self_pump_batch.log`, exit 0 (7 tests).
- Review maps: `scripts/check-card-evidence.ps1 -Preparation` PASS (1796 maps) at
  `build/campaign/queue-v23/tmp/evidence-b2.log`.

## Review and gate

Frozen in `build/campaign/review/queue-v23-batch-b-v1` (patch SHA-256
`f34ebb8ebc1852ea93619ca2a24b2ca40f70e2682653f095790aa578d99e43ef`). Independent read-only review
approved the family (with delta approval after map fixture corrections). Batch queue-v23 batch B
full gate `verify-4d575129-898b-4fb8-9819-5bffb0cab0d8` passed exit 0, covering 04-haste-self-pump..06-naturalize-lifegain; commit
`3b4f0f8cecaa53e66008c35eaf97d804c5dc66bc`; remote `origin/card-campaign` verified at the same SHA.
