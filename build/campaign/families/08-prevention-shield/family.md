# Family: 08-prevention-shield (8 cards)

## Selection

Simple single-primitive cohort, 8 cards. Revalidated by Oracle ID and printed name against the
pinned bulk, live registry, maps, `CARDS.md`, `queue-exclusions.txt` and `blocked.tsv`; see
`build/campaign/queue-v23/revalidation.tsv`.

## Exact cards

| Oracle ID | Card | Mana | Type line | P/T | Shield |
|---|---|---|---|---|---|
| 4d979fbf-5009-47a1-8d77-b1ce80bbe90d | Sanctum Custodian | {2}{W} | Creature — Human Cleric | 1/2 | 2 |
| 7695b61a-8d44-44b1-9783-ea3114111c3e | Master Healer | {4}{W} | Creature — Human Cleric | 1/4 | 4 |
| 95a0ca48-d924-47f4-86ed-42c673ee778c | Samite Healer | {1}{W} | Creature — Human Cleric | 1/1 | 1 |
| 9d405e5b-0cee-4c8f-b3ef-65765853b8a4 | Orim, Samite Healer | {1}{W}{W} | Legendary Creature — Human Cleric | 1/3 | 3 |
| a5a1b4c9-474a-42b7-b412-c2d43a3353db | Clergy en-Vec | {1}{W} | Creature — Human Cleric | 1/1 | 1 |
| c70dded8-658b-4250-8057-54894350455c | Loxodon Anchorite | {2}{W}{W} | Creature — Elephant Cleric | 2/3 | 2 |
| e2117373-6f0a-41c8-ab6a-9014b9628e64 | Femeref Healer | {1}{W} | Creature — Human Cleric | 1/1 | 1 |
| e3bd74f3-1f38-4162-a0be-c76ad63e2d9b | Kithkin Healer | {2}{W} | Creature — Kithkin Cleric | 2/2 | 1 |

## Copy-from and differences

`healing_salve` mode 2 for `PreventNextDamage(amount, target: (kind: AnyTarget))`; the no-cost `{T}`
activated shape comes from the tap-ability cohort. The shield amount is 1, 2, 3 or 4; Orim is
legendary and has a distinct type line. Every card is a vanilla body plus only this tap ability.

## Rules evidence

CR 614.1a/615 (prevention shields), 115.1 (any target), 602.2 and 302.6 (activating a tap ability)
and 514.2 (the shield ends during cleanup). No card in this cohort has a pinned ruling.

## Canaries

- Simplest: Samite Healer ({1}{W}, 1/1, prevent 1).
- Hardest: Master Healer ({4}{W}, 1/4, prevent 4).
- Every card is executed by the all-card creature-shield, player-damage, residual-shield, tap,
  no-target and cleanup tests.

## Semantic evidence

- `scenario prevention_shield_batch::prevention_shield_batch_canary_cards_shield_a_player_and_absorb_damage`
- `scenario prevention_shield_batch::prevention_shield_batch_cards_shield_a_creature`
- `scenario prevention_shield_batch::prevention_shield_batch_cards_absorb_exact_damage_on_a_player`
- `scenario prevention_shield_batch::prevention_shield_batch_cards_reject_a_second_activation_while_tapped`
- `scenario prevention_shield_batch::prevention_shield_batch_cards_require_a_target`
- `scenario prevention_shield_batch::prevention_shield_batch_cards_expire_at_cleanup`
- `scenario prevention_shield_batch::prevention_shield_batch_cards_apply_summoning_sickness_to_the_tap_cost`
- `mechanic_registry::prevention_shield_batch::prevention_shield_batch_maps_definitions`
- `mechanic_registry::prevention_shield_batch::prevention_shield_batch_fingerprints_match_the_pinned_oracle_text`

The eight maps cover the single Oracle line and the tap/prevention nodes. The creature test asserts
the exact residual shield after Shock; the player test asserts exact life loss and residual shield.

## Focused evidence

- Intended canary red: `build/campaign/queue-v23/tmp/redC-prevention_shield_batch.log` (missing master_healer, exit 101).
- Focused canary green: `build/campaign/queue-v23/tmp/greenC-prevention_shield_batch.log`, exit 0.
- Focused registry green: `build/campaign/queue-v23/tmp/regC5detail-prevention_shield_batch.log`, exit 0 (3 tests).
- Focused scenario green: `build/campaign/queue-v23/tmp/scnC5detail-prevention_shield_batch.log`, exit 0 (7 tests).
- Card data check: `build/campaign/queue-v23/tmp/carddata-c2.log`, exit 0.
- Review maps: `scripts/check-card-evidence.ps1 -Preparation` PASS (1822 maps) at
  `build/campaign/queue-v23/tmp/evidence-c.log`.

## Review and gate

Frozen in `build/campaign/review/queue-v23-batch-c-v2` (patch SHA-256
`a671cd9841c7ece680f401ac05036964b7d2fbc11b633a450f11e92fc17a1fb7`). Independent read-only review
approved the family (with delta approval after material findings were corrected). Batch queue-v23
batch C full gate `verify-991ead11-4b55-4081-a1d2-d7a4c600524e` passed exit 0, covering 07-damage-creature-lifegain..
09-etb-vanilla-tokens; commit `15e0b3a9f810dfeec954d331a81a108f4b04ee17`; remote `origin/card-campaign` verified at the same SHA.
