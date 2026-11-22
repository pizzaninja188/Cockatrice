# Family: 02-cluestones (10 cards)

## Selection

Queue-v26 Parameter family, 10 cards. Revalidated by Oracle ID and printed name against the pinned bulk,
live registry, maps, `CARDS.md`, `queue-exclusions.txt` and `blocked.tsv`; see
`build/campaign/queue-v26/revalidation.tsv`.

Stable typed shape: `{3}` artifact with `{T}: Add {X} or {Y}` and `{X}{Y}, {T}, Sacrifice this artifact: Draw a card`.

## Exact cards

| Oracle ID | Card | Mana | Type line | P/T | Oracle text |
|---|---|---|---|---|---|
| 27e04c41-f42c-4d60-8a71-ec2d7c326f64 | Azorius Cluestone | {3} | Artifact | — | {T}: Add {W} or {U}. / {W}{U}, {T}, Sacrifice this artifact: Draw a card. |
| f6c3b420-9aca-4fab-b7bb-1814bd2d93f6 | Boros Cluestone | {3} | Artifact | — | {T}: Add {R} or {W}. / {R}{W}, {T}, Sacrifice this artifact: Draw a card. |
| 62f834c7-4765-4b6f-812c-7ce14e13ea0d | Dimir Cluestone | {3} | Artifact | — | {T}: Add {U} or {B}. / {U}{B}, {T}, Sacrifice this artifact: Draw a card. |
| e917ad3d-df8e-442b-918c-3aa7a10f3cc4 | Golgari Cluestone | {3} | Artifact | — | {T}: Add {B} or {G}. / {B}{G}, {T}, Sacrifice this artifact: Draw a card. |
| 80911151-1e72-4d10-b00a-9fcff2dd131a | Gruul Cluestone | {3} | Artifact | — | {T}: Add {R} or {G}. / {R}{G}, {T}, Sacrifice this artifact: Draw a card. |
| 3c76bb90-92fb-428b-8e94-def4cf4b6f2d | Izzet Cluestone | {3} | Artifact | — | {T}: Add {U} or {R}. / {U}{R}, {T}, Sacrifice this artifact: Draw a card. |
| 09650a76-05c4-40a0-b861-d146b3ba87d6 | Orzhov Cluestone | {3} | Artifact | — | {T}: Add {W} or {B}. / {W}{B}, {T}, Sacrifice this artifact: Draw a card. |
| b4f0bda0-1051-4ab8-b254-891f427b33ad | Rakdos Cluestone | {3} | Artifact | — | {T}: Add {B} or {R}. / {B}{R}, {T}, Sacrifice this artifact: Draw a card. |
| 792d4afd-b3de-4be2-b68a-03a5be7b9627 | Selesnya Cluestone | {3} | Artifact | — | {T}: Add {G} or {W}. / {G}{W}, {T}, Sacrifice this artifact: Draw a card. |
| 0b65050d-a9cd-4b6c-9114-af89bfa6682f | Simic Cluestone | {3} | Artifact | — | {T}: Add {G} or {U}. / {G}{U}, {T}, Sacrifice this artifact: Draw a card. |

## Copy-from and differences

Copy `mind_stone`'s sacrifice-draw ability and the shipped cameo two-option mana ability; only the two produced colors and the two colored cost symbols differ.

## Rules evidence

CR 605.1a/605.3a-b (artifact mana ability and immediate resolution), 118.3 and 701.21a (tap and sacrifice costs), 107.4/202.1 (colored mana in an artifact's cost) and 121.1 (draw). The cycle has no pinned ruling.

Pinned rulings are saved per Oracle ID under
`build/campaign/queue-v26/families/02-cluestones/rulings/`.

## Canaries

- Simplest: Azorius Cluestone (`azorius_cluestone`).
- Hardest: Simic Cluestone (`simic_cluestone`).
- Every card in the family is executed by the shared all-card scenarios below.

## Semantic evidence

- `scenario cluestones_batch::cluestones_batch_cards_produce_each_printed_color_and_tap`
- `scenario cluestones_batch::cluestones_batch_cards_sacrifice_for_exactly_one_card`
- `scenario cluestones_batch::cluestones_batch_cards_reject_the_draw_without_the_exact_colored_cost`
- `mechanic_registry::cluestones_batch::cluestones_batch_maps_definitions`
- `mechanic_registry::cluestones_batch::cluestones_batch_fingerprints_match_the_pinned_oracle_text`

## Focused evidence

- Intended simplest canary red: `build/verification-logs/20261116-213645-248-red-simplest-cluestones.log` (missing definition, exit 101).
- Intended hardest canary red: `build/verification-logs/20261116-213838-201-red-hardest-cluestones.log` (missing definition at the hardest card, exit 101).
- Focused all-card green: `build/verification-logs/20261116-221549-222-scenario-postfix-cluestones.log` (exit 0).
- Registry green: `build/verification-logs/20261116-215415-217-registry-full-queue-v26.log` (full 911-test suite, exit 0).
- Review maps: `scripts/check-card-evidence.ps1 -Preparation` PASS for all 1,974 review maps (including all 47 queue-v26 maps) (card-evidence `2f26da74-67cc-4014-8ae0-ee05190d84f4`).
- Metadata refresh: only the 47 expected fingerprint rows and their `CARDS.md` checkboxes plus the `Implemented:` total changed.
- Formatting and focused lint: pending the batch preflight.

## Review and gate

Independent read-only review approved the family in the first pass; no material findings.

The batch queue-v26 combined full gate `verify-00668ab2-d089-4ca9-85bb-c6a48f810a49` passed exit 0 on the unchanged nine-family tree, covering `01-pain-taplands` through `09-mill-two`. Code commit `b01d2c8e9e8f497e0cbb8418e94a2f7f628cefa3`. Metadata refresh changed only the 47 expected fingerprint rows, their `CARDS.md` checkboxes and the `Implemented:` total.
