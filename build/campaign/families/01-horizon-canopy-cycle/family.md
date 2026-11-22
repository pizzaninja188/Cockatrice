# Family: 01-horizon-canopy-cycle (5 cards)

## Selection

Queue-v25 Multi-clause composition, 5 cards. Revalidated by Oracle ID and printed name against the pinned bulk,
live registry, maps, `CARDS.md`, `queue-exclusions.txt` and `blocked.tsv`; see
`build/campaign/queue-v25/revalidation.tsv`.

Stable typed shape: Land with `{T}, Pay 1 life` two-option mana and `{1}, {T}, Sacrifice this land: Draw a card`.

## Exact cards

| Oracle ID | Card | Mana | Type line | P/T | Oracle text |
|---|---|---|---|---|---|
| 026f4a4b-eedd-44e1-9d37-ca4fb8d6db98 | Fiery Islet | — | Land |  | {T}, Pay 1 life: Add {U} or {R}. / {1}, {T}, Sacrifice this land: Draw a card. |
| 262a5d83-506c-4781-9bc9-1a2b5d83955c | Horizon Canopy | — | Land |  | {T}, Pay 1 life: Add {G} or {W}. / {1}, {T}, Sacrifice this land: Draw a card. |
| 8ed932ff-986c-4592-ad70-53b3fac80d69 | Nurturing Peatland | — | Land |  | {T}, Pay 1 life: Add {B} or {G}. / {1}, {T}, Sacrifice this land: Draw a card. |
| fd45063f-c83c-431c-9104-f139c497ec0d | Silent Clearing | — | Land |  | {T}, Pay 1 life: Add {W} or {B}. / {1}, {T}, Sacrifice this land: Draw a card. |
| f97fd068-b83a-4621-bf8c-cc96e880ce90 | Sunbaked Canyon | — | Land |  | {T}, Pay 1 life: Add {R} or {W}. / {1}, {T}, Sacrifice this land: Draw a card. |

## Copy-from and differences

Copy `waterlogged_grove` exactly; only the printed name and the two produced colors differ.

## Rules evidence

CR 605.1a/605.3a-b (mana ability, immediate resolution), 119.4 (life payment), 118.3 and 701.21a (tap and sacrifice costs), 113.7a (source independence), 121.1 (draw). The cycle has no pinned ruling.

Pinned rulings are saved per Oracle ID under
`build/campaign/queue-v25/families/01-horizon-canopy-cycle/rulings/`.

## Canaries

- Simplest: Horizon Canopy (`horizon_canopy`).
- Hardest: Fiery Islet (`fiery_islet`).
- Every card in the family is executed by the shared all-card scenarios below.

## Semantic evidence

- `scenario horizon_canopy_cycle_batch::horizon_canopy_cycle_batch_cards_pay_one_life_for_each_printed_color`
- `scenario horizon_canopy_cycle_batch::horizon_canopy_cycle_batch_cards_reject_the_life_payment_at_zero_life`
- `scenario horizon_canopy_cycle_batch::horizon_canopy_cycle_batch_cards_draw_after_sacrificing_the_source`
- `mechanic_registry::horizon_canopy_cycle_batch::horizon_canopy_cycle_batch_maps_definitions`
- `mechanic_registry::horizon_canopy_cycle_batch::horizon_canopy_cycle_batch_fingerprints_match_the_pinned_oracle_text`

## Focused evidence

- Intended canary red: `build/verification-logs/20261116-180208-182-Rust-scenario-red-horizon-canopy-canary.log` (MissingCard or missing typed characteristic, exit 101).
- Focused all-card green: `build/verification-logs/20261116-200839-327-Rust-scenario-final2-horizon_canopy_cycle_batch.log` (exit 0).
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
