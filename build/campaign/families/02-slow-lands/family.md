# Family: 02-slow-lands (5 cards)

## Selection

Queue-v25 Simple single-primitive cohort, 5 cards. Revalidated by Oracle ID and printed name against the pinned bulk,
live registry, maps, `CARDS.md`, `queue-exclusions.txt` and `blocked.tsv`; see
`build/campaign/queue-v25/revalidation.tsv`.

Stable typed shape: Land whose `EntersTapped` condition counts at most one other land you control, then a two-option `{T}` mana ability.

## Exact cards

| Oracle ID | Card | Mana | Type line | P/T | Oracle text |
|---|---|---|---|---|---|
| f0ec8681-da50-466b-8cdd-1dc710deccd9 | Deserted Beach | — | Land |  | This land enters tapped unless you control two or more other lands. / {T}: Add {W} or {U}. |
| e2a37967-4212-4553-9f77-bcb613405807 | Haunted Ridge | — | Land |  | This land enters tapped unless you control two or more other lands. / {T}: Add {B} or {R}. |
| 709d2f10-1585-48c3-9058-ddd5f62f0452 | Overgrown Farmland | — | Land |  | This land enters tapped unless you control two or more other lands. / {T}: Add {G} or {W}. |
| 185c70c1-8403-4ae5-b45d-3679d4ee092a | Rockfall Vale | — | Land |  | This land enters tapped unless you control two or more other lands. / {T}: Add {R} or {G}. |
| 5f42b67f-87fd-4f98-a0e8-0c8313f4bbc8 | Shipwreck Marsh | — | Land |  | This land enters tapped unless you control two or more other lands. / {T}: Add {U} or {B}. |

## Copy-from and differences

Copy `deathcap_glade` exactly; only the printed name and the two produced colors differ.

## Rules evidence

CR 614.1d/614.12 (enters-tapped replacement effect), 605.1a/605.3b (mana ability), 109.2 (the source is not an "other land" for itself) and 305.6. The pinned ruling states that lands entering at the same time are not counted.

Pinned rulings are saved per Oracle ID under
`build/campaign/queue-v25/families/02-slow-lands/rulings/`.

## Canaries

- Simplest: Deserted Beach (`deserted_beach`).
- Hardest: Shipwreck Marsh (`shipwreck_marsh`).
- Every card in the family is executed by the shared all-card scenarios below.

## Semantic evidence

- `scenario slow_lands_batch::slow_lands_batch_cards_enter_tapped_with_zero_or_one_other_land`
- `scenario slow_lands_batch::slow_lands_batch_cards_enter_untapped_with_two_other_lands`
- `scenario slow_lands_batch::slow_lands_batch_cards_produce_each_printed_color`
- `scenario slow_lands_batch::slow_lands_batch_cards_ignore_lands_controlled_by_opponents`
- `mechanic_registry::slow_lands_batch::slow_lands_batch_maps_definitions`
- `mechanic_registry::slow_lands_batch::slow_lands_batch_fingerprints_match_the_pinned_oracle_text`

## Focused evidence

- Intended canary red: `build/verification-logs/20261116-180613-878-Rust-scenario-red-slow-lands-canary.log` (MissingCard or missing typed characteristic, exit 101).
- Focused all-card green: `build/verification-logs/20261116-195352-944-Rust-scenario-final-slow_lands_batch.log` (exit 0).
- Registry green: `build/verification-logs/20261116-195258-911-Rust-registry-green-queue-v25-full.log` (full 893-test suite, exit 0).
- Review maps: `scripts/check-card-evidence.ps1 -Preparation` PASS for all 43 queue-v25 maps (card-evidence `417f2d7f-117f-48ee-9a38-b42e162da7c8`).
- Formatting: `cargo fmt --all -- --check` exit 0.
- Focused lint: `cargo clippy -p tricerules-cards -p tricerules-core --all-targets -- -D warnings` exit 0.
- Metadata refresh: only the 43 expected fingerprint rows and their `CARDS.md` checkboxes changed; `Savage Lands` was ejected to #517 and left unchecked.

- Engine simultaneous-entry test: `lib engine::replacement::entry_layers_tests::conditional_slow_land_ignores_lands_entering_in_the_same_batch` exit 0 (`build/verification-logs/20261116-202500-Rust-lib-simultaneous-land-entry-queue-v25.log`).

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
