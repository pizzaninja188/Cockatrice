# Family: 07-etb-lose-life-flyers (4 cards)

## Selection

Queue-v26 Multi-clause composition, 4 cards. Revalidated by Oracle ID and printed name against the pinned bulk,
live registry, maps, `CARDS.md`, `queue-exclusions.txt` and `blocked.tsv`; see
`build/campaign/queue-v26/revalidation.tsv`.

Stable typed shape: Flying creature with a mandatory entry trigger: its controller loses the printed amount of life.

## Exact cards

| Oracle ID | Card | Mana | Type line | P/T | Oracle text |
|---|---|---|---|---|---|
| 25c04be3-f2e0-4f45-a1ca-9e75ffffa9b5 | Foul Imp | {B}{B} | Creature — Imp | 2/2 | Flying / When this creature enters, you lose 2 life. |
| 93f37df7-e1c4-457b-bc22-00c49b0c4f79 | Necrogen Scudder | {2}{B} | Creature — Phyrexian Horror | 3/3 | Flying / When this creature enters, you lose 3 life. |
| bd73ab86-0ac9-4ce0-be41-f4ad257e74f6 | Dread Reaper | {3}{B}{B}{B} | Creature — Horror | 6/5 | Flying / When this creature enters, you lose 5 life. |
| dbd1fc9c-6316-4c89-a971-f2e08ca291a4 | Vampiric Spirit | {2}{B}{B} | Creature — Spirit | 4/3 | Flying / When this creature enters, you lose 4 life. |

## Copy-from and differences

Copy `dusk_legion_zealot`'s `WhenSelfEntersBattlefield` + `LoseLife(amount: Fixed(N), who: Controller)` and add the printed flying keyword; only the printed amount and body differ.

## Rules evidence

CR 702.9a-b (flying), 603.6a (enters-the-battlefield trigger), 603.3d/608.2c (stack and resolution order), 119.3 (effect life loss adjusts the total), 120.3a (damage is a distinct life-loss cause) and 119.4 (paying life is a cost). Foul Imp's pinned ruling confirms the spell can be cast below the lost amount.

Pinned rulings are saved per Oracle ID under
`build/campaign/queue-v26/families/07-etb-lose-life-flyers/rulings/`.

## Canaries

- Simplest: Foul Imp (`foul_imp`).
- Hardest: Vampiric Spirit (`vampiric_spirit`).
- Every card in the family is executed by the shared all-card scenarios below.

## Semantic evidence

- `scenario etb_lose_life_flyers_batch::etb_lose_life_flyers_batch_cards_lose_the_printed_life_and_have_flying`
- `scenario etb_lose_life_flyers_batch::etb_lose_life_flyers_batch_cards_scope_the_loss_to_their_controller`
- `scenario etb_lose_life_flyers_batch::etb_lose_life_flyers_batch_cards_lose_the_game_at_exactly_the_printed_life`
- `scenario etb_lose_life_flyers_batch::foul_imp_can_be_cast_below_its_entry_life_loss`
- `mechanic_registry::etb_lose_life_flyers_batch::etb_lose_life_flyers_batch_maps_definitions`
- `mechanic_registry::etb_lose_life_flyers_batch::etb_lose_life_flyers_batch_fingerprints_match_the_pinned_oracle_text`

## Focused evidence

- Intended simplest canary red: `build/verification-logs/20261116-213653-706-red-simplest-etb_lose_life_flyers.log` (missing definition, exit 101).
- Intended hardest canary red: `build/verification-logs/20261116-213847-477-red-hardest-etb_lose_life_flyers.log` (missing definition at the hardest card, exit 101).
- Focused all-card green: `build/verification-logs/20261116-221558-987-scenario-postfix-etb_lose_life_flyers.log` (exit 0).
- Registry green: `build/verification-logs/20261116-215415-217-registry-full-queue-v26.log` (full 911-test suite, exit 0).
- Review maps: `scripts/check-card-evidence.ps1 -Preparation` PASS for all 1,974 review maps (including all 47 queue-v26 maps) (card-evidence `2f26da74-67cc-4014-8ae0-ee05190d84f4`).
- Metadata refresh: only the 47 expected fingerprint rows and their `CARDS.md` checkboxes plus the `Implemented:` total changed.
- Formatting and focused lint: pending the batch preflight.

## Review and gate

Review required pre-resolution, lethal-life and Foul Imp cast-path evidence; added, the Foul Imp-only fixture was scoped in the other maps, and the delta was approved.

The batch queue-v26 combined full gate `verify-00668ab2-d089-4ca9-85bb-c6a48f810a49` passed exit 0 on the unchanged nine-family tree, covering `01-pain-taplands` through `09-mill-two`. Code commit `b01d2c8e9e8f497e0cbb8418e94a2f7f628cefa3`. Metadata refresh changed only the 47 expected fingerprint rows, their `CARDS.md` checkboxes and the `Implemented:` total.
