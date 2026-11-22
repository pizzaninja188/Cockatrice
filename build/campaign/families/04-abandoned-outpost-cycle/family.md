# Family: 04-abandoned-outpost-cycle (5 cards)

## Selection

Queue-v25 Simple single-primitive cohort, 5 cards. Revalidated by Oracle ID and printed name against the pinned bulk,
live registry, maps, `CARDS.md`, `queue-exclusions.txt` and `blocked.tsv`; see
`build/campaign/queue-v25/revalidation.tsv`.

Stable typed shape: Entered-tapped land with one `{T}` single-color mana ability and `{T}, Sacrifice this land: Add one mana of any color`.

## Exact cards

| Oracle ID | Card | Mana | Type line | P/T | Oracle text |
|---|---|---|---|---|---|
| 5a70ccfa-d12d-4e62-a1a4-f05cda2fd442 | Abandoned Outpost | — | Land |  | This land enters tapped. / {T}: Add {W}. / {T}, Sacrifice this land: Add one mana of any color. |
| bce32541-3bc4-4553-ace2-784a58f164aa | Bog Wreckage | — | Land |  | This land enters tapped. / {T}: Add {B}. / {T}, Sacrifice this land: Add one mana of any color. |
| 68919931-b995-4aa0-9d2a-e80eae2f335f | Ravaged Highlands | — | Land |  | This land enters tapped. / {T}: Add {R}. / {T}, Sacrifice this land: Add one mana of any color. |
| eef64810-4187-40c1-8e59-1cb8773ebea3 | Seafloor Debris | — | Land |  | This land enters tapped. / {T}: Add {U}. / {T}, Sacrifice this land: Add one mana of any color. |
| 29c7f059-2eeb-40f9-8f50-03eba2d0d5e0 | Timberland Ruins | — | Land |  | This land enters tapped. / {T}: Add {G}. / {T}, Sacrifice this land: Add one mana of any color. |

## Copy-from and differences

Compose `secluded_steppe`'s entered-tapped single-color shape with `chromatic_star`'s sacrifice mana ability and `alloy_myr`'s five-option any-color output.

## Rules evidence

CR 614.1d/614.12 (enters tapped), 605.1a/605.3b (mana ability), 118.3 and 701.21a (tap and sacrifice costs) and 106.1a (the five any-color options). The cycle has no pinned ruling.

Pinned rulings are saved per Oracle ID under
`build/campaign/queue-v25/families/04-abandoned-outpost-cycle/rulings/`.

## Canaries

- Simplest: Abandoned Outpost (`abandoned_outpost`).
- Hardest: Timberland Ruins (`timberland_ruins`).
- Every card in the family is executed by the shared all-card scenarios below.

## Semantic evidence

- `scenario abandoned_outpost_batch::abandoned_outpost_batch_cards_enter_tapped_and_produce_their_color`
- `scenario abandoned_outpost_batch::abandoned_outpost_batch_cards_sacrifice_for_any_printed_color`
- `scenario abandoned_outpost_batch::abandoned_outpost_batch_cards_reject_the_sacrifice_while_tapped`
- `mechanic_registry::abandoned_outpost_batch::abandoned_outpost_batch_maps_definitions`
- `mechanic_registry::abandoned_outpost_batch::abandoned_outpost_batch_fingerprints_match_the_pinned_oracle_text`

## Focused evidence

- Intended canary red: `build/verification-logs/20261116-182731-402-Rust-scenario-red-abandoned-outpost-canary.log` (MissingCard or missing typed characteristic, exit 101).
- Focused all-card green: `build/verification-logs/20261116-195356-431-Rust-scenario-final-abandoned_outpost_batch.log` (exit 0).
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
