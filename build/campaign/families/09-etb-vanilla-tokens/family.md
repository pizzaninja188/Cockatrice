# Family: 09-etb-vanilla-tokens (12 cards)

## Selection

Parameter family, 12 cards. Revalidated by Oracle ID and printed name against the pinned bulk,
live registry, maps, `CARDS.md`, `queue-exclusions.txt` and `blocked.tsv`; see
`build/campaign/queue-v23/revalidation.tsv`.

## Exact cards

| Oracle ID | Card | Mana | Type line | P/T | Token | Count |
|---|---|---|---|---|---|---|
| 8b022754-6d16-470e-b754-4df6e4f4709e | Goblin Instigator | {1}{R} | Creature — Goblin Rogue | 1/1 | goblin_r_1_1 | 1 |
| 79d99305-b3dd-4687-af69-bf4b28704aae | Goblin Gang Leader | {2}{R}{R} | Creature — Goblin Warrior | 2/2 | goblin_r_1_1 | 2 |
| f5c5f64c-6911-430c-a825-b32b96d39c7d | Beetleback Chief | {2}{R}{R} | Creature — Goblin Warrior | 2/2 | goblin_r_1_1 | 2 |
| 4045c24e-098f-4582-8c50-576a94c7b705 | Ambassador Oak | {3}{G} | Creature — Treefolk Warrior | 3/3 | elf_warrior_g_1_1 | 1 |
| 3795fc31-2369-47d0-b9c9-120f44b8f114 | Elderleaf Mentor | {3}{G} | Creature — Elf Warrior | 3/2 | elf_warrior_g_1_1 | 1 |
| 42a1954f-774e-4d80-98b3-a736f23e2060 | Yavimaya Sapherd | {2}{G} | Creature — Fungus | 2/2 | saproling_g_1_1 | 1 |
| 193f9b12-ee34-40c2-9315-6794416a4f62 | Clarion Cathars | {3}{W} | Creature — Human Knight | 3/3 | human_w_1_1 | 1 |
| 8239c63d-097a-449a-953a-cd480e594aa8 | Watchful Giant | {5}{W} | Creature — Giant Soldier | 3/6 | human_w_1_1 | 1 |
| 42de2d46-c98d-4ae5-a53d-97531592529b | Daysquad Marshal | {3}{W} | Creature — Human Soldier | 3/3 | human_soldier_w_1_1 | 1 |
| cf6bb391-4fde-43c4-a0c5-990630d0ddcf | Protector of Gondor | {3}{W} | Creature — Human Soldier | 3/3 | human_soldier_w_1_1 | 1 |
| ad11ff81-acaa-4529-ba15-718dadbea259 | Aviation Pioneer | {2}{U} | Creature — Human Artificer | 1/2 | thopter_c_1_1_flying | 1 |
| 710dc9b1-7d32-4330-95a8-3e91a668202a | Ghirapur Gearcrafter | {2}{R} | Creature — Human Artificer | 2/1 | thopter_c_1_1_flying | 1 |

## Copy-from and differences

`elder_auntie` and `underfoot_underdogs` for `CreateTokens` on `WhenSelfEntersBattlefield`; token
definitions are the shipped `goblin_r_1_1`, `elf_warrior_g_1_1`, `saproling_g_1_1`,
`human_w_1_1`, `human_soldier_w_1_1` and `thopter_c_1_1_flying` templates.

Token identity, count and body differ by card: red Goblin (one or two), green Elf Warrior, green
Saproling, white Human, white Human Soldier, and colorless Thopter with flying. Every trigger is
mandatory and controller-scoped.

## Rules evidence

CR 603.6a (entry trigger), 111.1 (tokens), 608.2c (the controller follows the instruction), 704.5
(token state-based actions) and 603.3b/616.1f (simultaneous token entry order). No card in this
cohort has a pinned ruling.

## Canaries

- Simplest: Goblin Instigator ({1}{R}, one 1/1 Goblin).
- Hardest: Aviation Pioneer ({2}{U}, flying Thopter artifact token).
- Every card is executed by the all-card token-identity, count, cast, repeat-entry and
  controller-scope tests; count-two cards explicitly answer the simultaneous-entry-order choice.

## Semantic evidence

- `scenario etb_vanilla_tokens_batch::etb_vanilla_tokens_batch_canary_cards_create_the_printed_token`
- `scenario etb_vanilla_tokens_batch::etb_vanilla_tokens_batch_cards_create_only_their_printed_token`
- `scenario etb_vanilla_tokens_batch::etb_vanilla_tokens_batch_cards_cast_with_exact_cost_and_body`
- `scenario etb_vanilla_tokens_batch::etb_vanilla_tokens_batch_cards_create_tokens_once_per_entry`
- `scenario etb_vanilla_tokens_batch::etb_vanilla_tokens_batch_cards_source_effect_is_controller_scoped`
- `scenario etb_vanilla_tokens_batch::etb_vanilla_tokens_batch_count_two_cards_present_simultaneous_entry_order`
- `mechanic_registry::etb_vanilla_tokens_batch::etb_vanilla_tokens_batch_maps_definitions`
- `mechanic_registry::etb_vanilla_tokens_batch::etb_vanilla_tokens_batch_fingerprints_match_the_pinned_oracle_text`

The twelve maps cover the single entry trigger, its token node and the token dependency. The tests
assert registered token identity, controller, 1/1 body, Thopter flying, one-shot trigger behavior
and the simultaneous-entry choice for Goblin Gang Leader and Beetleback Chief.

## Focused evidence

- Intended canary red: `build/campaign/queue-v23/tmp/redC-etb_vanilla_tokens_batch.log` (missing goblin_instigator, exit 101).
- Focused canary green: `build/campaign/queue-v23/tmp/greenC-etb_vanilla_tokens_batch.log`, exit 0.
- Focused registry green: `build/campaign/queue-v23/tmp/regC5detail-etb_vanilla_tokens_batch.log`, exit 0 (3 tests).
- Focused scenario green: `build/campaign/queue-v23/tmp/scnC5detail-etb_vanilla_tokens_batch.log`, exit 0 (6 tests).
- Card data check: `build/campaign/queue-v23/tmp/carddata-c2.log`, exit 0.
- Review maps: `scripts/check-card-evidence.ps1 -Preparation` PASS (1822 maps) at
  `build/campaign/queue-v23/tmp/evidence-c.log`.

## Review and gate

Frozen in `build/campaign/review/queue-v23-batch-c-v2` (patch SHA-256
`a671cd9841c7ece680f401ac05036964b7d2fbc11b633a450f11e92fc17a1fb7`). Independent read-only review
approved the family (with delta approval after material findings were corrected). Batch queue-v23
batch C full gate `verify-991ead11-4b55-4081-a1d2-d7a4c600524e` passed exit 0, covering 07-damage-creature-lifegain..
09-etb-vanilla-tokens; commit `15e0b3a9f810dfeec954d331a81a108f4b04ee17`; remote `origin/card-campaign` verified at the same SHA.
