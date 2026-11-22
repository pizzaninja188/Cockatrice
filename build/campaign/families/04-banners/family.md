# Family: 04-banners (5 cards)

## Selection

Queue-v26 Multi-clause composition, 5 cards. Revalidated by Oracle ID and printed name against the pinned bulk,
live registry, maps, `CARDS.md`, `queue-exclusions.txt` and `blocked.tsv`; see
`build/campaign/queue-v26/revalidation.tsv`.

Stable typed shape: `{3}` artifact with `{T}: Add {X}, {Y}, or {Z}` and `{X}{Y}{Z}, {T}, Sacrifice this artifact: Draw a card`.

## Exact cards

| Oracle ID | Card | Mana | Type line | P/T | Oracle text |
|---|---|---|---|---|---|
| 46535f8e-1bcd-4588-ac6c-a4bc89c379c8 | Abzan Banner | {3} | Artifact | — | {T}: Add {W}, {B}, or {G}. / {W}{B}{G}, {T}, Sacrifice this artifact: Draw a card. |
| fda10ef2-f431-4d9b-96a4-4db4c98d2b0c | Jeskai Banner | {3} | Artifact | — | {T}: Add {U}, {R}, or {W}. / {U}{R}{W}, {T}, Sacrifice this artifact: Draw a card. |
| 9805b7b3-086a-4c91-810f-52c8f8f95f6a | Mardu Banner | {3} | Artifact | — | {T}: Add {R}, {W}, or {B}. / {R}{W}{B}, {T}, Sacrifice this artifact: Draw a card. |
| 93b139a5-2678-46a5-b8eb-45344ee3290a | Sultai Banner | {3} | Artifact | — | {T}: Add {B}, {G}, or {U}. / {B}{G}{U}, {T}, Sacrifice this artifact: Draw a card. |
| 8a35f23d-f59a-4677-9736-035c60c22a2b | Temur Banner | {3} | Artifact | — | {T}: Add {G}, {U}, or {R}. / {G}{U}{R}, {T}, Sacrifice this artifact: Draw a card. |

## Copy-from and differences

Copy `mind_stone`'s sacrifice-draw ability and a three-option mana output; only the printed shard colors differ.

## Rules evidence

CR 605.1a/605.3a-b (artifact mana ability and immediate resolution), 118.3 and 701.21a (tap and sacrifice costs), 107.4/202.1 (colored mana in an artifact's cost) and 121.1 (draw). The cycle has no pinned ruling.

Pinned rulings are saved per Oracle ID under
`build/campaign/queue-v26/families/04-banners/rulings/`.

## Canaries

- Simplest: Abzan Banner (`abzan_banner`).
- Hardest: Temur Banner (`temur_banner`).
- Every card in the family is executed by the shared all-card scenarios below.

## Semantic evidence

- `scenario banners_batch::banners_batch_cards_produce_each_printed_color_and_tap`
- `scenario banners_batch::banners_batch_cards_sacrifice_for_exactly_one_card`
- `scenario banners_batch::banners_batch_cards_reject_the_draw_without_all_three_colors`
- `mechanic_registry::banners_batch::banners_batch_maps_definitions`
- `mechanic_registry::banners_batch::banners_batch_fingerprints_match_the_pinned_oracle_text`

## Focused evidence

- Intended simplest canary red: `build/verification-logs/20261116-213648-662-red-simplest-banners.log` (missing definition, exit 101).
- Intended hardest canary red: `build/verification-logs/20261116-213841-843-red-hardest-banners.log` (missing definition at the hardest card, exit 101).
- Focused all-card green: `build/verification-logs/20261116-221552-962-scenario-postfix-banners.log` (exit 0).
- Registry green: `build/verification-logs/20261116-215415-217-registry-full-queue-v26.log` (full 911-test suite, exit 0).
- Review maps: `scripts/check-card-evidence.ps1 -Preparation` PASS for all 1,974 review maps (including all 47 queue-v26 maps) (card-evidence `2f26da74-67cc-4014-8ae0-ee05190d84f4`).
- Metadata refresh: only the 47 expected fingerprint rows and their `CARDS.md` checkboxes plus the `Implemented:` total changed.
- Formatting and focused lint: pending the batch preflight.

## Review and gate

Review required an exact-color illegal path; the {C}{C}{C} rejection was added and delta-approved.

The batch queue-v26 combined full gate `verify-00668ab2-d089-4ca9-85bb-c6a48f810a49` passed exit 0 on the unchanged nine-family tree, covering `01-pain-taplands` through `09-mill-two`. Code commit `b01d2c8e9e8f497e0cbb8418e94a2f7f628cefa3`. Metadata refresh changed only the 47 expected fingerprint rows, their `CARDS.md` checkboxes and the `Implemented:` total.
