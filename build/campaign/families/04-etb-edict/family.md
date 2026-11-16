# Family: 04-etb-edict (3 cards)

## Selection

Multi-clause composition, 3 cards. Revalidated by Oracle ID and printed name against the pinned
bulk, live registry, maps, `CARDS.md`, `queue-exclusions.txt` and `blocked.tsv`; see
`build/campaign/queue-v22/revalidation.tsv`.

## Exact cards

| Oracle ID | Card | Mana | Type line | P/T |
|---|---|---|---|---|
| 4b1bf05e-753e-4350-a913-894cf3cecc0c | Fleshbag Marauder | {2}{B} | Creature — Zombie Warrior | 3/1 |
| c3c45d50-9038-41df-bb2f-9bc40071845b | Merciless Executioner | {2}{B} | Creature — Orc Warrior | 3/1 |
| 0cf742c1-feb5-4108-9976-37fa58528212 | Slum Reaper | {3}{B} | Creature — Horror | 4/2 |

## Copy-from and differences

`portal_to_phyrexia` and `professor_onyx` for the `ChooseAndSacrifice` frame; the primitive's own
contract names Fleshbag Marauder's EachPlayer/1/Creature shape.

The recipient is EachPlayer rather than EachOpponent, count 1, creature only. Each player chooses
their own sacrificed creature (CR 101.4); the controller is included, so the source itself may be
the only legal choice and is then sacrificed.

## Rules evidence

CR 603.6a (entry trigger), 101.4/101.4b (each player chooses), 701.21a (simultaneous sacrifice).
Pinned rulings are saved per card under `rulings/`: Fleshbag Marauder has two, Merciless
Executioner one and Slum Reaper three; they confirm that each player chooses and that the source
may sacrifice itself.

## Canaries

- Simplest: Fleshbag Marauder ({2}{B} 3/1).
- Hardest: Slum Reaper ({3}{B} 4/2).
- Both are exercised by the shared canary scenario; every card is executed by the all-card loops,
  including the three-player check.

## Semantic evidence

- `scenario etb_edict_batch::etb_edict_batch_canary_each_player_sacrifices_their_own_chosen_creature`
- `scenario etb_edict_batch::etb_edict_batch_cards_make_each_player_choose_independently`
- `scenario etb_edict_batch::etb_edict_batch_controller_sacrifices_the_source_when_it_is_the_only_creature`
- `scenario etb_edict_batch::etb_edict_batch_cards_include_every_player_in_a_three_player_game`
- `mechanic_registry::etb_edict_batch::etb_edict_batch_maps_definitions`
- `mechanic_registry::etb_edict_batch::etb_edict_batch_fingerprints_match_the_pinned_oracle_text`

The three maps cover the Oracle line and reference the shared actual-card scenarios plus the
registry checks. The engine auto-sacrifices a player's only candidate, which the controller-only
and three-player tests exercise explicitly.

## Focused evidence

- Intended canary red: `build/verification-logs/20261116-041904-576-queue-v22-batchB-etb_edict_batch-canary-red.log` (missing fleshbag_marauder, exit 101).
- Focused canary green: `build/verification-logs/20261116-041912-242-queue-v22-batchB-etb_edict_batch-canary-green.log`, exit 0.
- Focused scenario green: `build/verification-logs/20261116-041652-207-queue-v22-batchB-etb-edict-scenario.log`, exit 0 (4 tests passed).
- Focused registry green: `build/verification-logs/20261116-041847-738-queue-v22-batchB-etb_edict_batch-registry.log`, exit 0 (3 tests passed).
- Review maps: `scripts/check-card-evidence.ps1 -Preparation` batch B PASS (9 maps) at
  `build/verification-logs/card-evidence-bdf44cef-7b6a-4d2c-8321-2369b396cacc`.

## Review and gate

Frozen in `build/campaign/review/queue-v22-batch-b-v2`. Independent read-only review and the
batch-B full gate are recorded in the batch gate summary below.
