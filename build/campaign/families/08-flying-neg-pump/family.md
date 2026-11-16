# Family: 08-flying-neg-pump (4 cards)

## Selection

Multi-clause composition, 4 cards. Revalidated by Oracle ID and printed name against the pinned
bulk, live registry, maps, `CARDS.md`, `queue-exclusions.txt` and `blocked.tsv`; see
`build/campaign/queue-v22/revalidation.tsv`.

## Exact cards

| Oracle ID | Card | Mana | Type line | P/T | Activation |
|---|---|---|---|---|---|
| e48d9efa-c169-4587-86fc-0021f265d69b | Darklit Gargoyle | {1}{W} | Artifact Creature — Gargoyle | 1/2 | {B}: +2/-1 |
| 13459bf3-fcdc-4838-9669-918f97113054 | Flowstone Wyvern | {3}{R}{R} | Creature — Drake | 3/3 | {R}: +2/-2 |
| 924c7c4e-9c4b-496a-ba49-7fb3918a8d07 | Metropolis Sprite | {1}{U} | Creature — Faerie Rogue | 1/2 | {U}: +1/-1 |
| 85c6290e-2bf2-4666-a606-326a8209ace6 | Talonrend | {4}{U} | Creature — Elemental | 0/5 | {U/R}: +1/-1 |

## Copy-from and differences

`frostburn_weird` and the self_pump_negative cohort for the printed `+X/-Y` activation; flying comes
from `welkin_tern` and the flying cohorts. Chilling Shade has the same combination but is blocked
for its snow cost.

The static flying keyword and the negative self-pump are independent; Talonrend uses a hybrid
`{U/R}` symbol. There is no other text.

## Rules evidence

CR 702.9 (flying), 602.1/602.2 (activation and cost), 611.2a/613.4c (until-end-of-turn P/T),
107.4e (hybrid) and 514.2 (cleanup expiry). None of the four cards has a pinned ruling.

## Canaries

- Simplest: Metropolis Sprite ({1}{U} 1/2, {U}: +1/-1).
- Hardest: Talonrend ({4}{U} 0/5, {U/R}: +1/-1).
- Both are exercised by the shared canary scenario; every card is executed by the all-card loops,
  including the zero-toughness, expiry, evasion and no-mana tests.

## Semantic evidence

- `scenario flying_neg_pump_batch::flying_neg_pump_batch_canary_cards_apply_the_printed_pump`
- `scenario flying_neg_pump_batch::flying_neg_pump_batch_cards_apply_the_exact_pump_to_every_card`
- `scenario flying_neg_pump_batch::flying_neg_pump_batch_cards_die_at_zero_toughness`
- `scenario flying_neg_pump_batch::flying_neg_pump_batch_cards_expire_at_cleanup`
- `scenario flying_neg_pump_batch::flying_neg_pump_batch_cards_keep_flying_evasion`
- `scenario flying_neg_pump_batch::flying_neg_pump_batch_cards_reject_activation_without_mana`
- `mechanic_registry::flying_neg_pump_batch::flying_neg_pump_batch_maps_definitions`
- `mechanic_registry::flying_neg_pump_batch::flying_neg_pump_batch_fingerprints_match_the_pinned_oracle_text`

The four maps cover both Oracle lines; the activations are resolved on the stack before asserting
the new body. Talonrend is activated with both hybrid colors across the tests.

## Focused evidence

- Intended canary red: `build/verification-logs/20261116-050831-796-queue-v22-batchC-flying_neg_pump_batch-canary-red.log` (missing metropolis_sprite, exit 101).
- Focused canary green: `build/verification-logs/20261116-050839-584-queue-v22-batchC-flying_neg_pump_batch-canary-green.log`, exit 0.
- Focused scenario green: `build/verification-logs/20261116-054113-573-queue-v22-batchC-flying_neg_pump_batch-scenario.log`, exit 0 (6 tests passed).
- Focused registry green: `build/verification-logs/20261116-053806-256-queue-v22-batchC-flying_neg_pump_batch-registry.log`, exit 0 (3 tests passed).
- Review maps: `scripts/check-card-evidence.ps1 -Preparation` batch C PASS (10 maps) at
  `build/verification-logs/card-evidence-25a519cc-372f-441e-90ad-799c14bbe2bd`.

## Review and gate

Frozen in `build/campaign/review/queue-v22-batch-c-v3`. Independent read-only review and the
batch-C full gate are recorded in the batch gate summary below.
