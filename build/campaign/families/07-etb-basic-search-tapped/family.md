# Family: 07-etb-basic-search-tapped (3 cards)

## Selection

Multi-clause composition, 3 cards. Revalidated by Oracle ID and printed name against the pinned
bulk, live registry, maps, `CARDS.md`, `queue-exclusions.txt` and `blocked.tsv`; see
`build/campaign/queue-v22/revalidation.tsv`.

## Exact cards

| Oracle ID | Card | Mana | Type line | P/T |
|---|---|---|---|---|
| 4ce2357f-93e6-40ca-beca-8f4e15adc464 | Farhaven Elf | {2}{G} | Creature — Elf Druid | 1/1 |
| ed8b2887-15fa-4e62-83ac-55973ac6f76c | Ondu Giant | {3}{G} | Creature — Giant Druid | 2/4 |
| ff16f889-9203-4874-919b-42e540cadcbc | Wild Wanderer | {3}{G} | Creature — Elf Druid | 3/2 |

## Copy-from and differences

`larval_scoutlander` for the optional typed search program with `Battlefield(tapped: true)`;
`wood_elves` for the basic-land battlefield search and `sylvan_ranger` for the optional private
search.

The search is optional, finds any basic land, the land enters tapped, and nothing goes to hand.
There is no sacrifice or other cost.

## Rules evidence

CR 603.6a (entry trigger), 701.23 (optional search; a filtered search may fail to find) and
701.24 (shuffle). None of the three cards has a pinned ruling.

## Canaries

- Simplest: Farhaven Elf ({2}{G} 1/1).
- Hardest: Ondu Giant ({3}{G} 2/4).
- Both are exercised by the shared canary scenario; every card is executed by the all-card loops.

## Semantic evidence

- `scenario etb_basic_search_tapped_batch::etb_basic_search_tapped_batch_canary_cards_find_a_basic_land_tapped`
- `scenario etb_basic_search_tapped_batch::etb_basic_search_tapped_batch_cards_search_every_printed_card`
- `scenario etb_basic_search_tapped_batch::etb_basic_search_tapped_batch_cards_may_decline`
- `scenario etb_basic_search_tapped_batch::etb_basic_search_tapped_batch_cards_shuffle_after_searching`
- `mechanic_registry::etb_basic_search_tapped_batch::etb_basic_search_tapped_batch_maps_definitions`
- `mechanic_registry::etb_basic_search_tapped_batch::etb_basic_search_tapped_batch_fingerprints_match_the_pinned_oracle_text`

The three maps cover the Oracle line, the optional Choose branch and the typed search. Every card
is executed by the all-card loops; the decline and shuffle tests prove the private-search paths.

## Focused evidence

- Intended canary red: `build/verification-logs/20261116-050816-430-queue-v22-batchC-etb_basic_search_tapped_batch-canary-red.log` (missing farhaven_elf, exit 101).
- Focused canary green: `build/verification-logs/20261116-050824-180-queue-v22-batchC-etb_basic_search_tapped_batch-canary-green.log`, exit 0.
- Focused scenario green: `build/verification-logs/20261116-054514-715-queue-v22-batchC-etb_basic_search_tapped_batch-scenario.log`, exit 0 (4 tests passed).
- Focused registry green: `build/verification-logs/20261116-050804-409-queue-v22-batchC-etb_basic_search_tapped_batch-registry.log`, exit 0 (3 tests passed).
- Review maps: `scripts/check-card-evidence.ps1 -Preparation` batch C PASS (10 maps) at
  `build/verification-logs/card-evidence-25a519cc-372f-441e-90ad-799c14bbe2bd`.

## Review and gate

Frozen in `build/campaign/review/queue-v22-batch-c-v3`. Independent read-only review approved the
family (with delta approval after the material findings were corrected). Batch queue-v22 batch C
full gate `verify-e9c4e926-ffde-4a0d-992d-f4faccf7780f` passed exit 0, covering
07-etb-basic-search-tapped..09-aura-pump-haste; commit `f286fae8019d2f53f2404234a360fb5d6b57fa6e`;
remote `origin/card-campaign` verified at the same SHA.
