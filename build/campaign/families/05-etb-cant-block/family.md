# Family: 05-etb-cant-block (3 cards)

## Selection

Multi-clause composition, 3 cards. Revalidated by Oracle ID and printed name against the pinned
bulk, live registry, maps, `CARDS.md`, `queue-exclusions.txt` and `blocked.tsv`; see
`build/campaign/queue-v22/revalidation.tsv`.

## Exact cards

| Oracle ID | Card | Mana | Type line | P/T |
|---|---|---|---|---|
| 543071da-fc89-4274-a2ae-d5d755d1f2a9 | Crossway Vampire | {1}{R}{R} | Creature — Vampire | 3/2 |
| 29863706-fa4e-49ed-9f40-69ae503abba3 | Goblin Shortcutter | {1}{R} | Creature — Goblin Scout | 2/1 |
| 1733777d-87fe-4975-b099-1ff803a721e8 | Unstoppable Ogre | {2}{R} | Artifact Creature — Ogre Warrior | 4/1 |

## Copy-from and differences

`summit_intimidator` and `zog,_triceraton_castaway` for the entry trigger plus
`ApplyCombatRestriction(Chosen, cant_block)`.

There is no other ability; Unstoppable Ogre is an artifact creature. The restriction applies to any
creature, including the source's own controller's, and expires at cleanup.

## Rules evidence

CR 603.6a (entry trigger), 509.1b (a declared block is unaffected), 611.2a and 514.2
(until-end-of-turn expiry). None of the three cards has a pinned ruling.

## Canaries

- Simplest: Goblin Shortcutter ({1}{R} 2/1).
- Hardest: Crossway Vampire ({1}{R}{R} 3/2).
- Both are exercised by the shared canary scenario; every card is executed by the all-card loops,
  including the block-rejection and expiry tests.

## Semantic evidence

- `scenario etb_cant_block_batch::etb_cant_block_batch_canary_cards_publish_the_restriction`
- `scenario etb_cant_block_batch::etb_cant_block_batch_cards_reject_the_target_as_a_blocker`
- `scenario etb_cant_block_batch::etb_cant_block_batch_cards_may_target_their_own_creature`
- `scenario etb_cant_block_batch::etb_cant_block_batch_cards_expire_at_cleanup`
- `scenario etb_cant_block_batch::etb_cant_block_batch_cards_fizzle_when_the_target_leaves`
- `mechanic_registry::etb_cant_block_batch::etb_cant_block_batch_maps_definitions`
- `mechanic_registry::etb_cant_block_batch::etb_cant_block_batch_fingerprints_match_the_pinned_oracle_text`

The three maps cover the Oracle line and reference the shared actual-card scenarios plus the
registry checks.

## Focused evidence

- Intended canary red: `build/verification-logs/20261116-041919-790-queue-v22-batchB-etb_cant_block_batch-canary-red.log` (missing goblin_shortcutter, exit 101).
- Focused canary green: `build/verification-logs/20261116-041927-549-queue-v22-batchB-etb_cant_block_batch-canary-green.log`, exit 0.
- Focused scenario green: `build/verification-logs/20261116-043748-372-queue-v22-batchB-etb-cant-block-scenario.log`, exit 0 (5 tests passed).
- Focused registry green: `build/verification-logs/20261116-041857-980-queue-v22-batchB-etb_cant_block_batch-registry.log`, exit 0 (3 tests passed).
- Review maps: `scripts/check-card-evidence.ps1 -Preparation` batch B PASS (9 maps) at
  `build/verification-logs/card-evidence-bdf44cef-7b6a-4d2c-8321-2369b396cacc`.

## Review and gate

Frozen in `build/campaign/review/queue-v22-batch-b-v2`. Independent read-only review approved the
family (with delta approval after the material findings were corrected). Batch queue-v22 batch B
full gate `verify-a6c0f2d5-b70e-4603-9634-2f38427de0dc` passed exit 0, covering
04-etb-edict..06-etb-draw-lose-life; commit `bd8b308c9bb0ad452a21688d6140662c422fa587`;
remote `origin/card-campaign` verified at the same SHA.
