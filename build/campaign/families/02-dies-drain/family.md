# Family: 02-dies-drain (5 cards)

## Selection

Multi-clause composition, 5 cards. Revalidated by Oracle ID and printed name against the
pinned bulk, live registry, maps, `CARDS.md`, `queue-exclusions.txt` and `blocked.tsv`; see
`build/campaign/queue-v22/revalidation.tsv`.

## Exact cards

| Oracle ID | Card | Mana | Type line | P/T |
|---|---|---|---|---|
| 0c766a86-9c70-465e-874b-9d9f3b4eb090 | Brotherhood Patriarch | {3}{B} | Creature — Human Assassin | 4/1 |
| 15323e45-8260-43dc-b72b-efd661a58d75 | Spirit of Malevolence | {1}{B} | Creature — Spirit | 2/1 |
| 7c3ff360-461a-44f3-a8f2-eb061b149cf5 | Symbiote Spawn | {2}{B} | Creature — Symbiote Villain | 3/2 |
| e2bc064e-4739-4eb1-ba44-1fd92b2829d7 | Etched Familiar | {2}{B} | Artifact Creature — Phyrexian Fox | 3/2 |
| f71bbaf2-02ea-4fc0-9a39-49d6689b36cd | Nocturnal Feeder | {2}{B} | Creature — Vampire Rogue | 2/1 |

## Copy-from and differences

`vampire_neonate` for the fixed `LoseLife(EachOpponent)` plus fixed `GainLife` pair; `shroudstomper`
for the same ordered pair on an entry trigger.

N is 1 or 2 and the gain is the fixed printed amount; Nocturnal Feeder also has flying. Kokusho
is not an analogue: it gains life equal to the total lost, which is a different wording and a
different primitive. The two-effect list keeps each opponent's loss and the controller's gain
independent in three- and four-player games.

## Rules evidence

CR 700.4 (dies), 603.6c (leaves-the-battlefield trigger), 119.3 (life loss and life gain) and
608.2c (the source's controller follows the instruction). Pinned rulings are saved per card under
`rulings/`; Spirit of Malevolence and Nocturnal Feeder have two rulings each, and the other three
cards have none.

## Canaries

- Simplest: Spirit of Malevolence (1 life).
- Hardest: Nocturnal Feeder (flying plus 2 life).
- Both are exercised by the shared canary scenario; every card is executed by the all-card loops,
  including the three-player drain check.

## Semantic evidence

- `scenario dies_drain_batch::dies_drain_batch_canary_cards_drain_each_opponent_and_credit_their_controller`
- `scenario dies_drain_batch::dies_drain_batch_cards_use_their_printed_amounts`
- `scenario dies_drain_batch::dies_drain_batch_cards_hit_every_opponent_in_a_three_player_game`
- `scenario dies_drain_batch::dies_drain_batch_cards_do_not_trigger_when_exiled`
- `scenario dies_drain_batch::dies_drain_batch_cards_can_kill_an_opponent_at_low_life`
- `mechanic_registry::dies_drain_batch::dies_drain_batch_maps_definitions`
- `mechanic_registry::dies_drain_batch::dies_drain_batch_fingerprints_match_the_pinned_oracle_text`

The five maps cover every normalized Oracle line and reference the shared actual-card scenarios
plus the registry checks. All five cards are executed by the all-card loops; the three-player test
proves each opponent is drained rather than two-player arithmetic.

## Focused evidence

- Intended canary red: `build/verification-logs/20261116-032400-806-queue-v22-batchA-dies-drain-canary-red.log` (missing spirit_of_malevolence, exit 101).
- Focused canary green: `build/verification-logs/20261116-032416-565-queue-v22-batchA-dies-drain-canary-green.log`, exit 0.
- Focused scenario green: `build/verification-logs/20261116-032220-924-queue-v22-batchA-dies-drain-scenario.log`, exit 0 (5 tests passed).
- Focused registry green: `build/verification-logs/20261116-033149-793-queue-v22-batchA-dies-drain-registry.log`, exit 0 (3 tests passed).
- Review maps: `scripts/check-card-evidence.ps1 -Preparation` batch A PASS (14 maps) at
  `build/verification-logs/card-evidence-5d19c6de-5ded-45de-9b5d-055c4e593879`.

Independent review caught the original `EachOpponentLosesLifeYouGainEqual` wording mismatch; the family now uses the fixed `LoseLife(EachOpponent)` plus fixed
`GainLife` pair, which passes in two- and three-player games.

## Review and gate

Frozen in `build/campaign/review/queue-v22-batch-a-v3`. Independent read-only review and the
batch-A full gate are recorded in the batch gate summary below.
