# Family: 03-mana-target-pump (4 cards)

## Selection

Multi-clause composition, 4 cards. Revalidated by Oracle ID and printed name against the pinned
bulk, live registry, maps, `CARDS.md`, `queue-exclusions.txt` and `blocked.tsv`; see
`build/campaign/queue-v23/revalidation.tsv`.

## Exact cards

| Oracle ID | Card | Mana | Type line | Activation | Pump |
|---|---|---|---|---|---|
| 0439df39-0324-4110-b3f4-4a32393021de | Ghitu War Cry | {2}{R} | Enchantment | {R} | +1/+0 |
| fada1102-bbb7-4c90-a72e-6c595c08a55d | Captive Flame | {2}{R} | Enchantment | {R} | +1/+0 |
| 37d74df9-cf87-4b82-842a-3941064078fb | Sacred Armory | {2} | Artifact | {2} | +1/+0 |
| fcf92abb-6c06-42d2-a3d1-f44ec997d56d | Ursapine | {3}{G}{G} | Creature — Beast (3/3) | {G} | +1/+1 |

## Copy-from and differences

`nylea_god_of_the_hunt` for `PumpTarget(... subject: Chosen((kind: Creature)))` in an activated
ability; the enchantment/artifact permanent shapes follow the shipped permanents with activated
abilities.

Ghitu War Cry and Captive Flame are identical {2}{R} enchantments with {R}: +1/+0; Sacred Armory is
a colorless artifact with {2}: +1/+0; Ursapine is a 3/3 creature with {G}: +1/+1. All four target
any creature, including an opponent's or the source itself.

## Rules evidence

CR 602.2 (activate), 115.1 (target chosen at activation), 611.2c/613.4c (until-end-of-turn
modifier), and 514.2 (the modifier ends during cleanup). No card in this cohort has a pinned
ruling.

## Canaries

- Simplest: Sacred Armory ({2} artifact, {2} activation).
- Hardest: Ursapine ({3}{G}{G} creature, {G} activation).
- Every card is executed by the all-card target-scope, stack, exact-payment, repeated-activation,
  rejection, fizzle and cleanup tests.

## Semantic evidence

- `scenario mana_target_pump_batch::mana_target_pump_batch_canary_cards_pump_a_chosen_creature`
- `scenario mana_target_pump_batch::mana_target_pump_batch_cards_activate_through_the_stack`
- `scenario mana_target_pump_batch::mana_target_pump_batch_cards_fizzle_a_target_that_left_before_resolution`
- `scenario mana_target_pump_batch::mana_target_pump_batch_cards_target_any_creature_and_leave_others_unchanged`
- `scenario mana_target_pump_batch::mana_target_pump_batch_cards_stack_repeated_activations`
- `scenario mana_target_pump_batch::mana_target_pump_batch_cards_reject_illegal_targets_without_paying`
- `scenario mana_target_pump_batch::mana_target_pump_batch_cards_expire_the_pump_at_cleanup`
- `scenario mana_target_pump_batch::mana_target_pump_batch_ursapine_can_target_itself`
- `mechanic_registry::mana_target_pump_batch::mana_target_pump_batch_maps_definitions`
- `mechanic_registry::mana_target_pump_batch::mana_target_pump_batch_fingerprints_match_the_pinned_oracle_text`

The four maps cover the single Oracle line and reference the shared actual-card scenarios plus the
registry checks. The all-card stack test proves one pending stack object and exact payment; the
fizzle test proves a removed target installs no continuous effect and pumps no bystander.

## Focused evidence

- Intended canary red: `build/campaign/queue-v23/tmp/red3.log` (missing sacred_armory, exit 101).
- Focused canary green: `build/campaign/queue-v23/tmp/green-mana_target_pump_batch.log`, exit 0.
- Focused registry green: `build/campaign/queue-v23/tmp/reg3-mana_target_pump_batch.log`, exit 0 (3 tests).
- Focused scenario green: `build/campaign/queue-v23/tmp/scn4-mana_target_pump_batch.log`, exit 0 (8 tests).
- Review maps: `scripts/check-card-evidence.ps1 -Preparation` PASS (1784 maps) at
  `build/campaign/queue-v23/tmp/evidence-a2.log`.

## Review and gate

Frozen in `build/campaign/review/queue-v23-batch-a-v3` (patch SHA-256
`53d7b8fc93fb96589e91b5a6bff627483f16dcbcd519b2f6a146436ab29517af`). Independent read-only
review approved the family after delta rounds added exact-payment, stack and fizzle assertions and
removed false canary fixtures. Batch queue-v23 batch A full gate
`verify-3d5e8584-e3f0-49c4-8141-553d37ac44f2` passed exit 0, covering
01-flying-any-mana..03-mana-target-pump; commit `1f613082b68b5f99602c4944081d03e80c17f98e`;
remote `origin/card-campaign` verified at the same SHA.
