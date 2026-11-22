# Family: 06-botanical-plaza-cycle (5 cards)

## Selection

Queue-v25 Multi-clause composition, 5 cards. Revalidated by Oracle ID and printed name against the pinned bulk,
live registry, maps, `CARDS.md`, `queue-exclusions.txt` and `blocked.tsv`; see
`build/campaign/queue-v25/revalidation.tsv`.

Stable typed shape: Entered-tapped land with a two-option `{T}` mana ability and `{2}{X}{Y}, {T}, Sacrifice this land: Draw a card`.

## Exact cards

| Oracle ID | Card | Mana | Type line | P/T | Oracle text |
|---|---|---|---|---|---|
| 19e6ebaf-7148-428f-8c79-1e9e57a4e659 | Botanical Plaza | — | Land |  | This land enters tapped. / {T}: Add {G} or {W}. / {2}{G}{W}, {T}, Sacrifice this land: Draw a card. |
| 7e2eb4d5-22a3-43c1-8cf3-e85723da1b61 | Racers' Ring | — | Land |  | This land enters tapped. / {T}: Add {R} or {G}. / {2}{R}{G}, {T}, Sacrifice this land: Draw a card. |
| 46466819-3c8b-4f7e-82c2-93cb0118a6e6 | Skybridge Towers | — | Land |  | This land enters tapped. / {T}: Add {W} or {U}. / {2}{W}{U}, {T}, Sacrifice this land: Draw a card. |
| e90e519d-023e-4d19-85ad-9972a76df3ba | Tramway Station | — | Land |  | This land enters tapped. / {T}: Add {B} or {R}. / {2}{B}{R}, {T}, Sacrifice this land: Draw a card. |
| 18b0b583-d17b-4c67-bc67-c4d9aad155c7 | Waterfront District | — | Land |  | This land enters tapped. / {T}: Add {U} or {B}. / {2}{U}{B}, {T}, Sacrifice this land: Draw a card. |

## Copy-from and differences

Compose `deathcap_glade`'s `EntersTapped`, `arcane_sanctum`'s two-option mana ability and `waterlogged_grove`'s sacrifice-draw ability, with the printed `{2}{X}{Y}` cost.

## Rules evidence

CR 614.1d/614.12 (enters tapped), 605.1a/605.3b (mana ability), 118.3 and 701.21a (tap and sacrifice costs), 121.1 (draw). The cycle has no pinned ruling.

Pinned rulings are saved per Oracle ID under
`build/campaign/queue-v25/families/06-botanical-plaza-cycle/rulings/`.

## Canaries

- Simplest: Botanical Plaza (`botanical_plaza`).
- Hardest: Waterfront District (`waterfront_district`).
- Every card in the family is executed by the shared all-card scenarios below.

## Semantic evidence

- `scenario botanical_plaza_batch::botanical_plaza_batch_cards_enter_tapped_and_produce_each_printed_color`
- `scenario botanical_plaza_batch::botanical_plaza_batch_cards_reject_the_draw_without_the_exact_colored_cost`
- `scenario botanical_plaza_batch::botanical_plaza_batch_cards_sacrifice_then_draw_one`
- `mechanic_registry::botanical_plaza_batch::botanical_plaza_batch_maps_definitions`
- `mechanic_registry::botanical_plaza_batch::botanical_plaza_batch_fingerprints_match_the_pinned_oracle_text`

## Focused evidence

- Intended canary red: `build/verification-logs/20261116-181556-769-Rust-scenario-red-botanical-plaza-canary.log` (MissingCard or missing typed characteristic, exit 101).
- Focused all-card green: `build/verification-logs/20261116-202514-581-Rust-scenario-final3-botanical_plaza_batch.log` (exit 0).
- Registry green: `build/verification-logs/20261116-195258-911-Rust-registry-green-queue-v25-full.log` (full 893-test suite, exit 0).
- Review maps: `scripts/check-card-evidence.ps1 -Preparation` PASS for all 43 queue-v25 maps (card-evidence `417f2d7f-117f-48ee-9a38-b42e162da7c8`).
- Formatting: `cargo fmt --all -- --check` exit 0.
- Focused lint: `cargo clippy -p tricerules-cards -p tricerules-core --all-targets -- -D warnings` exit 0.
- Metadata refresh: only the 43 expected fingerprint rows and their `CARDS.md` checkboxes changed; `Savage Lands` was ejected to #517 and left unchecked.

## Review and gate

Independent read-only review approved the family's reviewed semantics in the first pass, and each
material finding was corrected and delta-approved: the full 893-test registry target replaced an
invalid zero-test log, focused greens were rerun on the frozen content, simultaneous-entry engine
tests were made order-sensitive, hand-only cycling and the reviewed conformance exception rows
were added, and the Botanical sacrifice destination was asserted.

The batch queue-v25 A+B+C full gate `verify-d0b33461-8f12-4424-84bb-9e1b23ad110a` passed exit 0 on the unchanged nine-family
tree, covering `01-horizon-canopy-cycle` through `09-sliver-anthem-controlled`. Code commit
`7b83c28d3d1a8e5c17b7ae75c5341a00a7a9df27`. Metadata refresh changed only the 43 expected fingerprint rows and their `CARDS.md`
checkboxes; `Savage Lands` was ejected to #517 and left unchecked.
