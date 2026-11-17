# Family: 09-mill-two (3 cards)

## Selection

Queue-v26 Multi-clause composition, 3 cards. Revalidated by Oracle ID and printed name against the pinned bulk,
live registry, maps, `CARDS.md`, `queue-exclusions.txt` and `blocked.tsv`; see
`build/campaign/queue-v26/revalidation.tsv`.

Stable typed shape: `{M}, {T}: Target player mills two cards.`

## Exact cards

| Oracle ID | Card | Mana | Type line | P/T | Oracle text |
|---|---|---|---|---|---|
| 3212e47a-5492-4c50-9d4a-6ea562f1a6e1 | Millstone | {2} | Artifact | — | {2}, {T}: Target player mills two cards. |
| 04cc5292-a485-4bb8-a143-9364229d480f | Vedalken Entrancer | {3}{U} | Creature — Vedalken Wizard | 1/4 | {U}, {T}: Target player mills two cards. |
| 590d524f-278d-4cf4-ae3f-efcced4517b1 | Merfolk Mesmerist | {1}{U} | Creature — Merfolk Wizard | 1/2 | {U}, {T}: Target player mills two cards. |

## Copy-from and differences

Copy `codex_shredder`'s activated `MillTargetPlayer` + `Tap` shape; set `count: 2` and add the printed mana cost. Only the printed mana cost, mill count and artifact-versus-creature identity differ.

## Rules evidence

CR 701.17a-b (mill), 602.1a/602.2 (activated ability and printed cost), 115.1/608.2b (target player), 118.3 (tap cost) and 121.1 (mill is not a draw). Millstone's pinned ruling confirms that a short library is milled as far as possible. The supported engine puts multiple milled cards into the graveyard in library order; no in-scope card observes graveyard order, so the owner's CR 404.3 order choice is deferred.

Pinned rulings are saved per Oracle ID under
`build/campaign/queue-v26/families/09-mill-two/rulings/`.

## Canaries

- Simplest: Millstone (`millstone`).
- Hardest: Merfolk Mesmerist (`merfolk_mesmerist`).
- Every card in the family is executed by the shared all-card scenarios below.

## Semantic evidence

- `scenario mill_two_batch::mill_two_batch_cards_mill_the_target_player_for_exactly_two`
- `scenario mill_two_batch::mill_two_batch_cards_can_target_their_controller`
- `scenario mill_two_batch::mill_two_batch_cards_reject_missing_targets`
- `scenario mill_two_batch::mill_two_batch_cards_mill_as_much_as_possible_without_causing_a_loss`
- `scenario mill_two_batch::mill_two_batch_cards_reject_a_second_activation_while_tapped`
- `mechanic_registry::mill_two_batch::mill_two_batch_maps_definitions`
- `mechanic_registry::mill_two_batch::mill_two_batch_fingerprints_match_the_pinned_oracle_text`

## Focused evidence

- Intended simplest canary red: `build/verification-logs/20261116-213657-091-red-simplest-mill_two.log` (missing definition, exit 101).
- Intended hardest canary red: `build/verification-logs/20261116-213851-010-red-hardest-mill_two.log` (missing definition at the hardest card, exit 101).
- Focused all-card green: `build/verification-logs/20261116-221602-825-scenario-postfix-mill_two.log` (exit 0).
- Registry green: `build/verification-logs/20261116-215415-217-registry-full-queue-v26.log` (full 911-test suite, exit 0).
- Review maps: `scripts/check-card-evidence.ps1 -Preparation` PASS for all 1,974 review maps (including all 47 queue-v26 maps) (card-evidence `2f26da74-67cc-4014-8ae0-ee05190d84f4`).
- Metadata refresh: only the 47 expected fingerprint rows and their `CARDS.md` checkboxes plus the `Implemented:` total changed.
- Formatting and focused lint: pending the batch preflight.

## Review and gate

Review required CR 701.17 and copy-from corrections plus activation, top-card and short-library evidence; corrected, the CR 404.3 order choice was recorded as a scoped deferral, and the delta was approved.

The batch queue-v26 combined full gate `verify-00668ab2-d089-4ca9-85bb-c6a48f810a49` passed exit 0 on the unchanged nine-family tree, covering `01-pain-taplands` through `09-mill-two`. Code commit `b01d2c8e9e8f497e0cbb8418e94a2f7f628cefa3`; remote `b01d2c8e9e8f497e0cbb8418e94a2f7f628cefa3`. Metadata refresh changed only the 47 expected fingerprint rows, their `CARDS.md` checkboxes and the `Implemented:` total.
