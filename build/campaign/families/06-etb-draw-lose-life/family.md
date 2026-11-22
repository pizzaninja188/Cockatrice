# Family: 06-etb-draw-lose-life (3 cards)

## Selection

Multi-clause composition, 3 cards. Revalidated by Oracle ID and printed name against the pinned
bulk, live registry, maps, `CARDS.md`, `queue-exclusions.txt` and `blocked.tsv`; see
`build/campaign/queue-v22/revalidation.tsv`.

## Exact cards

| Oracle ID | Card | Mana | Type line | P/T |
|---|---|---|---|---|
| ea2dce18-195e-4681-a687-f9819edaf9fc | Dusk Legion Zealot | {1}{B} | Creature — Vampire Soldier | 1/1 |
| e409c9be-0c9a-43c3-adb4-8c47afc1d551 | Phyrexian Rager | {2}{B} | Creature — Phyrexian Horror | 2/2 |
| 475aaf7c-b0f8-40f3-ad91-3e7d414de933 | Tithebearer Giant | {5}{B} | Creature — Giant Warrior | 4/5 |

## Copy-from and differences

`audacious_thief` for the ordered `Draw` then fixed `LoseLife` pair; `shroudstomper` is a
different multi-clause entry example, not this pair.

The pair is on the entry trigger and the life loss is exactly 1; there is no other text.

## Rules evidence

CR 603.6a (entry trigger), 121.1 (draw), 119.3 (life loss), 608.2c (written order). None of the
three cards has a pinned ruling.

## Canaries

- Simplest: Dusk Legion Zealot ({1}{B} 1/1).
- Hardest: Tithebearer Giant ({5}{B} 4/5).
- Both are exercised by the shared canary scenario; every card is executed by the all-card loops,
  including the one-life loss test.

## Semantic evidence

- `scenario etb_draw_lose_life_batch::etb_draw_lose_life_batch_canary_cards_draw_one_and_lose_one`
- `scenario etb_draw_lose_life_batch::etb_draw_lose_life_batch_cards_scope_the_draw_and_loss_to_their_controller`
- `scenario etb_draw_lose_life_batch::etb_draw_lose_life_batch_cards_draw_exactly_one`
- `scenario etb_draw_lose_life_batch::etb_draw_lose_life_batch_cards_lose_the_game_at_one_life`
- `scenario etb_draw_lose_life_batch::etb_draw_lose_life_batch_cards_enter_the_battlefield`
- `mechanic_registry::etb_draw_lose_life_batch::etb_draw_lose_life_batch_maps_definitions`
- `mechanic_registry::etb_draw_lose_life_batch::etb_draw_lose_life_batch_fingerprints_match_the_pinned_oracle_text`

The three maps cover the Oracle line and reference the shared actual-card scenarios plus the
registry checks.

## Focused evidence

- Intended canary red: `build/verification-logs/20261116-041935-097-queue-v22-batchB-etb_draw_lose_life_batch-canary-red.log` (missing dusk_legion_zealot, exit 101).
- Focused canary green: `build/verification-logs/20261116-041942-772-queue-v22-batchB-etb_draw_lose_life_batch-canary-green.log`, exit 0.
- Focused scenario green: `build/verification-logs/20261116-043842-195-queue-v22-batchB-etb-draw-lose-life-scenario.log`, exit 0 (5 tests passed).
- Focused registry green: `build/verification-logs/20261116-041859-558-queue-v22-batchB-etb_draw_lose_life_batch-registry.log`, exit 0 (3 tests passed).
- Review maps: `scripts/check-card-evidence.ps1 -Preparation` batch B PASS (9 maps) at
  `build/verification-logs/card-evidence-bdf44cef-7b6a-4d2c-8321-2369b396cacc`.

## Review and gate

Frozen in `build/campaign/review/queue-v22-batch-b-v2`. Independent read-only review approved the
family (with delta approval after the material findings were corrected). Batch queue-v22 batch B
full gate `verify-a6c0f2d5-b70e-4603-9634-2f38427de0dc` passed exit 0, covering
04-etb-edict..06-etb-draw-lose-life; commit `bd8b308c9bb0ad452a21688d6140662c422fa587`;
remote `origin/card-campaign` verified at the same SHA.
