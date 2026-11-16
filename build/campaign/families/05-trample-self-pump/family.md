# Family: 05-trample-self-pump (3 cards)

## Selection

Multi-clause composition, 3 cards. Revalidated by Oracle ID and printed name against the pinned
bulk, live registry, maps, `CARDS.md`, `queue-exclusions.txt` and `blocked.tsv`; see
`build/campaign/queue-v23/revalidation.tsv`.

## Exact cards

| Oracle ID | Card | Mana | Type line | P/T | Activation | Pump |
|---|---|---|---|---|---|---|
| d8a3a89d-9d27-40ae-a1aa-d55cc6a79b0b | Almighty Brushwagg | {G} | Creature — Brushwagg | 1/1 | {3}{G} | +3/+3 |
| 3a799843-1304-4abf-b8d7-db616628ee3f | Feral Ridgewolf | {2}{R} | Creature — Wolf | 1/2 | {1}{R} | +2/+0 |
| 9debaed0-b605-4778-9977-216d194ecdeb | Sun-Collared Raptor | {1}{R} | Creature — Dinosaur | 1/2 | {2}{R} | +3/+0 |

## Copy-from and differences

`nightwing_shade` for the activated source pump and `valley_mightcaller` for the printed trample
keyword.

Feral Ridgewolf and Sun-Collared Raptor pump power only; Almighty Brushwagg pumps both. Activation
costs and bonuses are card-specific and are not collapsed. Sun-Collared Raptor's canonical id is
`sun-collared_raptor` (hyphen preserved) while its face id is `sun_collared_raptor`; the review map
filename matches the RON stem.

## Rules evidence

CR 702.19 (trample), 602.2 (activate), 611.2c/613.4c (pump) and 510.1 (assigning combat damage). No
card in this cohort has a pinned ruling.

## Canaries

- Simplest: Feral Ridgewolf ({2}{R}, 1/2, {1}{R}: +2/+0).
- Hardest: Almighty Brushwagg ({G}, 1/1, {3}{G}: +3/+3).
- Every card is executed by the all-card cast, pump, stack, exact-payment, rejection, cleanup and
  trample-excess combat tests.

## Semantic evidence

- `scenario trample_self_pump_batch::trample_self_pump_batch_canary_cards_cast_and_apply_the_printed_keyword`
- `scenario trample_self_pump_batch::trample_self_pump_batch_cards_cast_with_exact_characteristics`
- `scenario trample_self_pump_batch::trample_self_pump_batch_cards_activate_only_the_source_with_the_printed_pump`
- `scenario trample_self_pump_batch::trample_self_pump_batch_cards_stack_repeated_activations`
- `scenario trample_self_pump_batch::trample_self_pump_batch_cards_reject_activation_without_the_exact_cost`
- `scenario trample_self_pump_batch::trample_self_pump_batch_cards_expire_the_pump_at_cleanup`
- `scenario trample_self_pump_batch::trample_self_pump_batch_cards_trample_excess_after_the_printed_pump`
- `mechanic_registry::trample_self_pump_batch::trample_self_pump_batch_maps_definitions`
- `mechanic_registry::trample_self_pump_batch::trample_self_pump_batch_fingerprints_match_the_pinned_oracle_text`

The three maps cover both Oracle lines and reference the shared actual-card scenarios plus the
registry checks. The combat test assigns exact lethal damage to a 2/2 blocker and asserts the
remaining printed pump damage reaches the defending player.

## Focused evidence

- Intended canary red: `build/campaign/queue-v23/tmp/redB-trample_self_pump_batch.log` (missing almighty_brushwagg, exit 101).
- Focused canary green: `build/campaign/queue-v23/tmp/greenB-trample_self_pump_batch.log`, exit 0.
- Focused registry green: `build/campaign/queue-v23/tmp/regBdetail-trample_self_pump_batch.log`, exit 0 (3 tests).
- Focused scenario green: `build/campaign/queue-v23/tmp/scnBdetail-trample_self_pump_batch.log`, exit 0 (7 tests).
- Review maps: `scripts/check-card-evidence.ps1 -Preparation` PASS (1796 maps) at
  `build/campaign/queue-v23/tmp/evidence-b2.log`.

## Review and gate

Frozen in `build/campaign/review/queue-v23-batch-b-v1` (patch SHA-256
`f34ebb8ebc1852ea93619ca2a24b2ca40f70e2682653f095790aa578d99e43ef`). Independent read-only review
approved the family (with delta approval after map fixture and hyphenated-id corrections). Batch
queue-v23 batch B full gate `verify-4d575129-898b-4fb8-9819-5bffb0cab0d8` passed exit 0, covering
04-haste-self-pump..06-naturalize-lifegain; commit `3b4f0f8cecaa53e66008c35eaf97d804c5dc66bc`; remote `origin/card-campaign`
verified at the same SHA.
