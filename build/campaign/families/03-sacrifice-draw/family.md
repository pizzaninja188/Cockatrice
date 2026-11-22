# Family: 03-sacrifice-draw (4 cards)

## Selection

Parameter family, 4 cards. Revalidated by Oracle ID and printed name against the pinned bulk,
live registry, maps, `CARDS.md`, `queue-exclusions.txt` and `blocked.tsv`; see
`build/campaign/queue-v22/revalidation.tsv`.

## Exact cards

| Oracle ID | Card | Mana | Type line |
|---|---|---|---|
| c1cf9d74-e700-456a-9e5a-1c4df22db268 | Blood Divination | {3}{B} | Sorcery |
| 46f78c42-85d1-4cd4-9fc3-d5fdec04a4c8 | Vivisection | {3}{U} | Sorcery |
| 6a125750-2b8c-4f9d-8173-ac8d14c91ddb | Altar's Reap | {1}{B} | Instant |
| 17c7878f-891f-40a6-80a7-47fcc7b8e29b | Skulltap | {1}{B} | Sorcery |

## Copy-from and differences

`village_rites` and `corrupted_conviction` for the mandatory creature-sacrifice additional cost
with a fixed draw; `divination` and `rowdy_research` for the draw values.

Draw count is two or three; Blood Divination and Vivisection are sorceries, Altar's Reap is an
instant; there is no other text or target.

## Rules evidence

CR 118.8 and 601.2f-h (the sacrifice is an additional cost paid before casting), 701.21
(sacrifice), 121.1 (draw). Pinned rulings are saved per card under `rulings/`; each card has two
rulings: exactly one creature must be sacrificed, and opponents can respond only after all costs
are paid.

## Canaries

- Simplest: Altar's Reap ({1}{B} instant, draw two).
- Hardest: Vivisection ({3}{U} sorcery, draw three).
- Both are exercised by the shared canary scenario; every card is executed by the all-card loops,
  and Altar's Reap additionally proves instant-speed casting on the opponent's main phase.

## Semantic evidence

- `scenario sacrifice_draw_batch::sacrifice_draw_batch_canary_cards_sacrifice_the_chosen_creature_then_draw`
- `scenario sacrifice_draw_batch::sacrifice_draw_batch_cards_draw_their_printed_count`
- `scenario sacrifice_draw_batch::sacrifice_draw_batch_cards_require_a_creature_you_control`
- `scenario sacrifice_draw_batch::sacrifice_draw_batch_cards_reject_an_opponents_creature`
- `scenario sacrifice_draw_batch::sacrifice_draw_batch_altars_reap_can_be_cast_at_instant_speed`
- `mechanic_registry::sacrifice_draw_batch::sacrifice_draw_batch_maps_definitions`
- `mechanic_registry::sacrifice_draw_batch::sacrifice_draw_batch_fingerprints_match_the_pinned_oracle_text`

The four maps cover both normalized Oracle lines and reference the shared actual-card scenarios
plus the registry checks. All four cards are executed by the all-card loops.

## Focused evidence

- Intended canary red: `build/verification-logs/20261116-032427-186-queue-v22-batchA-sacrifice-draw-canary-red.log` (missing altars_reap, exit 101).
- Focused canary green: `build/verification-logs/20261116-032434-761-queue-v22-batchA-sacrifice-draw-canary-green.log`, exit 0.
- Focused scenario green: `build/verification-logs/20261116-033157-641-queue-v22-batchA-sacrifice-draw-scenario.log`, exit 0 (6 tests passed).
- Focused registry green: `build/verification-logs/20261116-032347-372-queue-v22-batchA-sacrifice-draw-registry.log`, exit 0 (3 tests passed).
- Review maps: `scripts/check-card-evidence.ps1 -Preparation` batch A PASS (14 maps) at
  `build/verification-logs/card-evidence-5d19c6de-5ded-45de-9b5d-055c4e593879`.

## Review and gate

Frozen in `build/campaign/review/queue-v22-batch-a-v3`. Independent read-only review approved the
family (with delta approval after the material findings were corrected). Batch queue-v22 batch A
full gate `verify-9fa5b1cf-055b-4e8f-bd7a-5169dff96367` passed exit 0, covering
01-dies-counter-target..03-sacrifice-draw; commit `b765b1ca2f2e2415fc8027ef8ff330829deb28ec`;
remote `origin/card-campaign` verified at the same SHA.
