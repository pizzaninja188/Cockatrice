# Family: 03-lockets (10 cards)

## Selection

Queue-v26 Parameter family, 10 cards. Revalidated by Oracle ID and printed name against the pinned bulk,
live registry, maps, `CARDS.md`, `queue-exclusions.txt` and `blocked.tsv`; see
`build/campaign/queue-v26/revalidation.tsv`.

Stable typed shape: `{3}` artifact with `{T}: Add {X} or {Y}` and four hybrid symbols, `{T}, Sacrifice this artifact: Draw two cards`.

## Exact cards

| Oracle ID | Card | Mana | Type line | P/T | Oracle text |
|---|---|---|---|---|---|
| ca13dda2-fbb0-4b6b-9126-3c8df04dee2b | Azorius Locket | {3} | Artifact | — | {T}: Add {W} or {U}. / {W/U}{W/U}{W/U}{W/U}, {T}, Sacrifice this artifact: Draw two cards. |
| b044b4fd-d504-4a11-80bf-eb8fb0b6e21e | Boros Locket | {3} | Artifact | — | {T}: Add {R} or {W}. / {R/W}{R/W}{R/W}{R/W}, {T}, Sacrifice this artifact: Draw two cards. |
| 3ae4b679-2e3c-4f60-bcf4-b279917cdc54 | Dimir Locket | {3} | Artifact | — | {T}: Add {U} or {B}. / {U/B}{U/B}{U/B}{U/B}, {T}, Sacrifice this artifact: Draw two cards. |
| dba6e24e-7875-46b9-b51d-4cd71568ecfa | Golgari Locket | {3} | Artifact | — | {T}: Add {B} or {G}. / {B/G}{B/G}{B/G}{B/G}, {T}, Sacrifice this artifact: Draw two cards. |
| 7018ccd0-96e6-458f-b412-197edcb2bafd | Gruul Locket | {3} | Artifact | — | {T}: Add {R} or {G}. / {R/G}{R/G}{R/G}{R/G}, {T}, Sacrifice this artifact: Draw two cards. |
| 7ee07546-2c2e-4679-b671-7ea509b1d3ea | Izzet Locket | {3} | Artifact | — | {T}: Add {U} or {R}. / {U/R}{U/R}{U/R}{U/R}, {T}, Sacrifice this artifact: Draw two cards. |
| 8aaf248e-66cf-447b-b062-9aa2ab1ec864 | Orzhov Locket | {3} | Artifact | — | {T}: Add {W} or {B}. / {W/B}{W/B}{W/B}{W/B}, {T}, Sacrifice this artifact: Draw two cards. |
| ab784412-0722-4501-9aa0-fd68d307f6fd | Rakdos Locket | {3} | Artifact | — | {T}: Add {B} or {R}. / {B/R}{B/R}{B/R}{B/R}, {T}, Sacrifice this artifact: Draw two cards. |
| 9ecd98d5-8d66-4cb0-bada-23dfa0faacc0 | Selesnya Locket | {3} | Artifact | — | {T}: Add {G} or {W}. / {G/W}{G/W}{G/W}{G/W}, {T}, Sacrifice this artifact: Draw two cards. |
| c567fed8-df00-4579-901e-8754079105bf | Simic Locket | {3} | Artifact | — | {T}: Add {G} or {U}. / {G/U}{G/U}{G/U}{G/U}, {T}, Sacrifice this artifact: Draw two cards. |

## Copy-from and differences

Copy `mind_stone`'s sacrifice-draw ability with `Draw(count: 2)` and the shipped hybrid payment contract (`unmake`, `hovel_hurler`); only the two produced colors and the hybrid pair differ.

## Rules evidence

CR 107.4e (each hybrid symbol may be paid with either half), 605.1a/605.3a-b (artifact mana ability and immediate resolution), 118.3 and 701.21a (tap and sacrifice costs) and 121.1 (draw two). The cycle has no pinned ruling.

Pinned rulings are saved per Oracle ID under
`build/campaign/queue-v26/families/03-lockets/rulings/`.

## Canaries

- Simplest: Azorius Locket (`azorius_locket`).
- Hardest: Simic Locket (`simic_locket`).
- Every card in the family is executed by the shared all-card scenarios below.

## Semantic evidence

- `scenario lockets_batch::lockets_batch_cards_produce_each_printed_color_and_tap`
- `scenario lockets_batch::lockets_batch_cards_pay_four_hybrid_symbols_and_draw_two`
- `scenario lockets_batch::lockets_batch_cards_reject_the_draw_with_too_little_mana`
- `mechanic_registry::lockets_batch::lockets_batch_maps_definitions`
- `mechanic_registry::lockets_batch::lockets_batch_fingerprints_match_the_pinned_oracle_text`

## Focused evidence

- Intended simplest canary red: `build/verification-logs/20261116-213646-957-red-simplest-lockets.log` (missing definition, exit 101).
- Intended hardest canary red: `build/verification-logs/20261116-213840-016-red-hardest-lockets.log` (missing definition at the hardest card, exit 101).
- Focused all-card green: `build/verification-logs/20261116-221551-069-scenario-postfix-lockets.log` (exit 0).
- Registry green: `build/verification-logs/20261116-215415-217-registry-full-queue-v26.log` (full 911-test suite, exit 0).
- Review maps: `scripts/check-card-evidence.ps1 -Preparation` PASS for all 1,974 review maps (including all 47 queue-v26 maps) (card-evidence `2f26da74-67cc-4014-8ae0-ee05190d84f4`).
- Metadata refresh: only the 47 expected fingerprint rows and their `CARDS.md` checkboxes plus the `Implemented:` total changed.
- Formatting and focused lint: pending the batch preflight.

## Review and gate

Independent read-only review approved the family in the first pass; no material findings.

The batch queue-v26 combined full gate `verify-00668ab2-d089-4ca9-85bb-c6a48f810a49` passed exit 0 on the unchanged nine-family tree, covering `01-pain-taplands` through `09-mill-two`. Code commit `b01d2c8e9e8f497e0cbb8418e94a2f7f628cefa3`. Metadata refresh changed only the 47 expected fingerprint rows, their `CARDS.md` checkboxes and the `Implemented:` total.
