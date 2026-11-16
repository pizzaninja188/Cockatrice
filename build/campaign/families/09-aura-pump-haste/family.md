# Family: 09-aura-pump-haste (3 cards)

## Selection

Simple single-primitive cohort, 3 cards. Revalidated by Oracle ID and printed name against the
pinned bulk, live registry, maps, `CARDS.md`, `queue-exclusions.txt` and `blocked.tsv`; see
`build/campaign/queue-v22/revalidation.tsv`.

## Exact cards

| Oracle ID | Card | Mana | Type line | Bonus |
|---|---|---|---|---|
| 67ca79d7-9064-4605-8625-b1cfe5cb1b45 | Goblin War Paint | {1}{R} | Enchantment — Aura | +2/+2 and haste |
| 3d95e8b1-0ddb-4bd3-b4cc-de55b6f8c403 | Primal Visitation | {3}{R}{G} | Enchantment — Aura | +3/+3 and haste |
| 4992144f-7a36-4f30-9e8a-4355288b50c5 | Swashbuckling | {1}{R} | Enchantment — Aura | +2/+2 and haste |

## Copy-from and differences

`super_speed` for the attached power/keyword modifier and Aura attach; `aura_menace` and the
aura_pump cohort for equal-P/T Auras.

The toughness bonus equals the power bonus (2 or 3); there is no flash and no entry trigger. The
Aura falls off when its creature leaves.

## Rules evidence

CR 303.4 (Aura attached), 611.2c/613.4c (continuous modifier), 702.10 (haste) and 704.5m (Aura
state-based action). None of the three cards has a pinned ruling.

## Canaries

- Simplest: Goblin War Paint ({1}{R}, +2/+2).
- Hardest: Primal Visitation ({3}{R}{G}, +3/+3).
- Both are exercised by the shared canary scenario; every card is executed by the all-card loops,
  including the haste, illegal-target and fall-off tests.

## Semantic evidence

- `scenario aura_pump_haste_batch::aura_pump_haste_batch_canary_cards_attach_and_grant_the_printed_bonus`
- `scenario aura_pump_haste_batch::aura_pump_haste_batch_cards_apply_every_printed_bonus`
- `scenario aura_pump_haste_batch::aura_pump_haste_batch_cards_grant_haste_immediately`
- `scenario aura_pump_haste_batch::aura_pump_haste_batch_cards_reject_a_noncreature_target`
- `scenario aura_pump_haste_batch::aura_pump_haste_batch_cards_go_to_the_graveyard_with_their_creature`
- `mechanic_registry::aura_pump_haste_batch::aura_pump_haste_batch_maps_definitions`
- `mechanic_registry::aura_pump_haste_batch::aura_pump_haste_batch_fingerprints_match_the_pinned_oracle_text`

The three maps cover both Oracle lines and reference the shared actual-card scenarios plus the
registry checks.

## Focused evidence

- Intended canary red: `build/verification-logs/20261116-050847-267-queue-v22-batchC-aura_pump_haste_batch-canary-red.log` (missing goblin_war_paint, exit 101).
- Focused canary green: `build/verification-logs/20261116-050855-065-queue-v22-batchC-aura_pump_haste_batch-canary-green.log`, exit 0.
- Focused scenario green: `build/verification-logs/20261116-050637-992-queue-v22-batchC-aura_pump_haste_batch-scenario.log`, exit 0 (5 tests passed).
- Focused registry green: `build/verification-logs/20261116-050751-975-queue-v22-batchC-aura_pump_haste_batch-registry.log`, exit 0 (3 tests passed).
- Review maps: `scripts/check-card-evidence.ps1 -Preparation` batch C PASS (10 maps) at
  `build/verification-logs/card-evidence-25a519cc-372f-441e-90ad-799c14bbe2bd`.

## Review and gate

Frozen in `build/campaign/review/queue-v22-batch-c-v3`. Independent read-only review and the
batch-C full gate are recorded in the batch gate summary below.
