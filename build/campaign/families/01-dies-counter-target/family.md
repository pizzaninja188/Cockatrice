# Family: 01-dies-counter-target (5 cards)

## Selection

Multi-clause composition, 5 cards. Revalidated by Oracle ID and printed name against the
pinned bulk, live registry, maps, `CARDS.md`, `queue-exclusions.txt` and `blocked.tsv`; see
`build/campaign/queue-v22/revalidation.tsv`.

## Exact cards

| Oracle ID | Card | Mana | Type line | P/T |
|---|---|---|---|---|
| 43d44a7f-829a-4f4d-9f71-2f4384ddd803 | Spinal Centipede | {2}{B} | Creature — Insect | 3/2 |
| ad62cd58-fe6f-4844-b3dc-5ba21327fc43 | Guul Draz Mucklord | {2}{B} | Creature — Crocodile | 2/3 |
| bc948e19-816b-4c13-a36b-73b3e0472dca | Sparring Construct | {1} | Artifact Creature — Construct | 1/1 |
| d17d085f-aeda-4b23-ab80-729d070ae813 | Lawless Broker | {2}{B} | Creature — Aetherborn Rogue | 3/2 |
| d464b54c-3b02-4c4e-99ef-1ad75f336c65 | Goblin Assault Team | {3}{R} | Creature — Goblin Warrior | 4/1 |

## Copy-from and differences

`steadfast_sentry` for the dies trigger and targeted counter, with the same printed shape.

No vigilance; four cards have no other ability, and Goblin Assault Team adds haste. The counter
target is any creature you control, including a different creature.

## Rules evidence

CR 700.4 (dies), 603.6c (leaves-the-battlefield trigger), 122.1 (counters) and 608.2c (the
controller follows the instruction). Pinned rulings are saved per card under `rulings/`; Spinal
Centipede, Sparring Construct and Goblin Assault Team each have one ruling, all about simultaneous
death: a creature that dies at the same time cannot be saved by the counter.

## Canaries

- Simplest: Sparring Construct ({1} 1/1 artifact creature).
- Hardest: Goblin Assault Team (haste plus the dies trigger).
- Both are exercised by the shared canary scenario; every card is executed by the all-card loop.

## Semantic evidence

- `scenario dies_counter_target_batch::dies_counter_target_batch_canary_cards_place_one_counter_on_a_chosen_creature`
- `scenario dies_counter_target_batch::dies_counter_target_batch_cards_place_exactly_one_counter`
- `scenario dies_counter_target_batch::dies_counter_target_batch_cards_do_not_trigger_when_exiled`
- `scenario dies_counter_target_batch::dies_counter_target_batch_cards_without_a_legal_target_do_not_prompt`
- `scenario dies_counter_target_batch::dies_counter_target_batch_cards_do_not_counter_a_departed_target`
- `scenario dies_counter_target_batch::dies_counter_target_batch_cards_scope_the_target_to_their_controller`
- `mechanic_registry::dies_counter_target_batch::dies_counter_target_batch_maps_definitions`
- `mechanic_registry::dies_counter_target_batch::dies_counter_target_batch_fingerprints_match_the_pinned_oracle_text`

The five maps cover every normalized Oracle line and reference the shared actual-card scenarios
plus the registry checks. All five cards are executed by the all-card scenario.

## Focused evidence

- Intended canary red: `build/verification-logs/20261116-025507-034-queue-v22-batchA-dies-counter-canary-red.log` (missing sparring_construct, exit 101).
- Focused canary green: `build/verification-logs/20261116-025511-894-queue-v22-batchA-dies-counter-canary-green.log`, exit 0.
- Focused scenario green: `build/verification-logs/20261116-032039-853-queue-v22-batchA-dies-counter-scenario-all.log`, exit 0 (6 tests passed).
- Focused registry green: `build/verification-logs/20261116-032352-276-queue-v22-batchA-dies-counter-registry-all.log`, exit 0 (3 tests passed).
- Review maps: `scripts/check-card-evidence.ps1 -Preparation` batch A PASS (14 maps) at
  `build/verification-logs/card-evidence-5d19c6de-5ded-45de-9b5d-055c4e593879`.

## Review and gate

Frozen in `build/campaign/review/queue-v22-batch-a-v3`. Independent read-only review approved the
family (with delta approval after the material findings were corrected). Batch queue-v22 batch A
full gate `verify-9fa5b1cf-055b-4e8f-bd7a-5169dff96367` passed exit 0, covering
01-dies-counter-target..03-sacrifice-draw; commit `b765b1ca2f2e2415fc8027ef8ff330829deb28ec`;
remote `origin/card-campaign` verified at the same SHA.
