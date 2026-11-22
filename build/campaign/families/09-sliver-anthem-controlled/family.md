# Family: 09-sliver-anthem-controlled (5 cards)

## Selection

Queue-v25 Simple single-primitive cohort, 5 cards. Revalidated by Oracle ID and printed name against the pinned bulk,
live registry, maps, `CARDS.md`, `queue-exclusions.txt` and `blocked.tsv`; see
`build/campaign/queue-v25/revalidation.tsv`.

Stable typed shape: Sliver with a controller-scoped `AnthemPt` for the Sliver subtype, including itself.

## Exact cards

| Oracle ID | Card | Mana | Type line | P/T | Oracle text |
|---|---|---|---|---|---|
| d513e5df-31b0-4756-bf47-403460d0068e | Battle Sliver | {4}{R} | Creature — Sliver | 3/3 | Sliver creatures you control get +2/+0. |
| 9b33059c-c862-4452-8609-14a0e2551fb3 | Cleaving Sliver | {3}{R} | Creature — Sliver | 2/2 | Sliver creatures you control get +2/+0. |
| 52e8ec03-23ed-43a0-bce2-ad0cd5c4d737 | Megantic Sliver | {5}{G} | Creature — Sliver | 3/3 | Sliver creatures you control get +3/+3. |
| 6b589e70-9c1d-48dd-a597-479959e692bf | Predatory Sliver | {1}{G} | Creature — Sliver | 1/1 | Sliver creatures you control get +1/+1. |
| ae58e71c-b617-404f-a5dc-5e6a487f2675 | Steelform Sliver | {2}{W} | Creature — Sliver | 2/2 | Sliver creatures you control get +0/+1. |

## Copy-from and differences

Compose `corsair_captain`'s controller-scoped subtype anthem with the shipped Sliver anthems; `exclude_self` stays false because these print no "other".

## Rules evidence

CR 604.1/611.3a (continuous static ability), 613.4c (layer 7c) and 109.2 (subtype description plus controller scope). The pinned rulings confirm cumulativity and that a creature that stops being a Sliver stops being affected.

Pinned rulings are saved per Oracle ID under
`build/campaign/queue-v25/families/09-sliver-anthem-controlled/rulings/`.

## Canaries

- Simplest: Predatory Sliver (`predatory_sliver`).
- Hardest: Megantic Sliver (`megantic_sliver`).
- Every card in the family is executed by the shared all-card scenarios below.

## Semantic evidence

- `scenario sliver_anthem_controlled_batch::sliver_anthem_batch_cards_pump_only_your_slivers_including_self`
- `scenario sliver_anthem_controlled_batch::sliver_anthem_batch_cards_stack_with_a_second_copy`
- `mechanic_registry::sliver_anthem_controlled_batch::sliver_anthem_controlled_batch_maps_definitions`
- `mechanic_registry::sliver_anthem_controlled_batch::sliver_anthem_controlled_batch_fingerprints_match_the_pinned_oracle_text`

## Focused evidence

- Intended canary red: `build/verification-logs/20261116-182145-215-Rust-scenario-red-sliver-anthem-canary.log` (MissingCard or missing typed characteristic, exit 101).
- Focused all-card green: `build/verification-logs/20261116-195406-922-Rust-scenario-final-sliver_anthem_controlled_batch.log` (exit 0).
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
