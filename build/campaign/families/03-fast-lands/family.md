# Family: 03-fast-lands (4 cards)

## Selection

Queue-v25 Simple single-primitive cohort, 4 cards. Revalidated by Oracle ID and printed name against the pinned bulk,
live registry, maps, `CARDS.md`, `queue-exclusions.txt` and `blocked.tsv`; see
`build/campaign/queue-v25/revalidation.tsv`.

Stable typed shape: Land whose `EntersTapped` condition counts at least three other lands you control, then a two-option `{T}` mana ability.

## Exact cards

| Oracle ID | Card | Mana | Type line | P/T | Oracle text |
|---|---|---|---|---|---|
| 5ad94412-6f79-4c5d-bbd4-4ef5779a7b6d | Blackcleave Cliffs | — | Land |  | This land enters tapped unless you control two or fewer other lands. / {T}: Add {B} or {R}. |
| a2b48695-f7d7-42ce-a8a0-2a723428542a | Darkslick Shores | — | Land |  | This land enters tapped unless you control two or fewer other lands. / {T}: Add {U} or {B}. |
| 94f6c407-e665-4032-be13-a01e40c1f306 | Razorverge Thicket | — | Land |  | This land enters tapped unless you control two or fewer other lands. / {T}: Add {G} or {W}. |
| 9e7a240d-dc33-47ac-9f17-77fab4c1c340 | Seachrome Coast | — | Land |  | This land enters tapped unless you control two or fewer other lands. / {T}: Add {W} or {U}. |

## Copy-from and differences

Copy `blooming_marsh` exactly; only the printed name and the two produced colors differ.

## Rules evidence

CR 614.1d/614.12 (enters-tapped replacement effect), 605.1a/605.3b (mana ability), 109.2 (the source is not an "other land" for itself) and 305.6. The pinned rulings state the zero-to-two-other-lands boundary and that simultaneously entering lands are not counted.

Pinned rulings are saved per Oracle ID under
`build/campaign/queue-v25/families/03-fast-lands/rulings/`.

## Canaries

- Simplest: Blackcleave Cliffs (`blackcleave_cliffs`).
- Hardest: Seachrome Coast (`seachrome_coast`).
- Every card in the family is executed by the shared all-card scenarios below.

## Semantic evidence

- `scenario fast_lands_batch::fast_lands_batch_cards_enter_untapped_with_zero_to_two_other_lands`
- `scenario fast_lands_batch::fast_lands_batch_cards_enter_tapped_with_three_other_lands`
- `scenario fast_lands_batch::fast_lands_batch_cards_produce_each_printed_color`
- `scenario fast_lands_batch::fast_lands_batch_cards_ignore_lands_controlled_by_opponents`
- `mechanic_registry::fast_lands_batch::fast_lands_batch_maps_definitions`
- `mechanic_registry::fast_lands_batch::fast_lands_batch_fingerprints_match_the_pinned_oracle_text`

## Focused evidence

- Intended canary red: `build/verification-logs/20261116-180839-737-Rust-scenario-red-fast-lands-canary.log` (MissingCard or missing typed characteristic, exit 101).
- Focused all-card green: `build/verification-logs/20261116-195354-676-Rust-scenario-final-fast_lands_batch.log` (exit 0).
- Registry green: `build/verification-logs/20261116-195258-911-Rust-registry-green-queue-v25-full.log` (full 893-test suite, exit 0).
- Review maps: `scripts/check-card-evidence.ps1 -Preparation` PASS for all 43 queue-v25 maps (card-evidence `417f2d7f-117f-48ee-9a38-b42e162da7c8`).
- Formatting: `cargo fmt --all -- --check` exit 0.
- Focused lint: `cargo clippy -p tricerules-cards -p tricerules-core --all-targets -- -D warnings` exit 0.
- Metadata refresh: only the 43 expected fingerprint rows and their `CARDS.md` checkboxes changed; `Savage Lands` was ejected to #517 and left unchecked.

- Engine simultaneous-entry test: `lib engine::replacement::entry_layers_tests::conditional_fast_land_ignores_lands_entering_in_the_same_batch` exit 0 (`build/verification-logs/20261116-202500-Rust-lib-simultaneous-land-entry-queue-v25.log`).

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
