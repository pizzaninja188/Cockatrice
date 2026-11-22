# Family: 05-desert-cycling (5 cards)

## Selection

Queue-v25 Multi-clause composition, 5 cards. Revalidated by Oracle ID and printed name against the pinned bulk,
live registry, maps, `CARDS.md`, `queue-exclusions.txt` and `blocked.tsv`; see
`build/campaign/queue-v25/revalidation.tsv`.

Stable typed shape: Entered-tapped Desert with one `{T}` single-color mana ability and colored `{1}{X}` cycling.

## Exact cards

| Oracle ID | Card | Mana | Type line | P/T | Oracle text |
|---|---|---|---|---|---|
| 311f38a6-f68f-4d30-bc4e-62339f1e0d88 | Desert of the Fervent | — | Land — Desert |  | This land enters tapped. / {T}: Add {R}. / Cycling {1}{R} ({1}{R}, Discard this card: Draw a card.) |
| 190e664b-9875-4335-af46-353886ed18ff | Desert of the Glorified | — | Land — Desert |  | This land enters tapped. / {T}: Add {B}. / Cycling {1}{B} ({1}{B}, Discard this card: Draw a card.) |
| 852d4dc3-404d-4565-99e9-1eac8f6eca5e | Desert of the Indomitable | — | Land — Desert |  | This land enters tapped. / {T}: Add {G}. / Cycling {1}{G} ({1}{G}, Discard this card: Draw a card.) |
| 508f9e7e-2ff7-4593-b0a9-0612d7b5d646 | Desert of the Mindful | — | Land — Desert |  | This land enters tapped. / {T}: Add {U}. / Cycling {1}{U} ({1}{U}, Discard this card: Draw a card.) |
| 2672e0ca-8d5c-449f-8483-35d0e697fbb2 | Desert of the True | — | Land — Desert |  | This land enters tapped. / {T}: Add {W}. / Cycling {1}{W} ({1}{W}, Discard this card: Draw a card.) |

## Copy-from and differences

Copy `secluded_steppe`; add the printed Desert subtype and use the printed `{1}{X}` cycling cost instead of `{X}`.

## Rules evidence

CR 614.1d/614.12 (enters tapped), 605.1a/605.3b (mana ability), 702.29a-b (cycling as a hand-zone activated ability) and 118.3 (the printed colored cycling cost). The pinned ruling confirms Desert is a subtype with no intrinsic mana ability.

Pinned rulings are saved per Oracle ID under
`build/campaign/queue-v25/families/05-desert-cycling/rulings/`.

## Canaries

- Simplest: Desert of the Fervent (`desert_of_the_fervent`).
- Hardest: Desert of the True (`desert_of_the_true`).
- Every card in the family is executed by the shared all-card scenarios below.

## Semantic evidence

- `scenario desert_cycling_batch::desert_cycling_batch_cards_enter_tapped_and_produce_their_color`
- `scenario desert_cycling_batch::desert_cycling_batch_cards_reject_cycling_without_the_exact_colored_cost`
- `scenario desert_cycling_batch::desert_cycling_batch_cards_discard_then_draw_one`
- `scenario desert_cycling_batch::desert_cycling_batch_cards_cycle_only_from_hand`
- `mechanic_registry::desert_cycling_batch::desert_cycling_batch_maps_definitions`
- `mechanic_registry::desert_cycling_batch::desert_cycling_batch_fingerprints_match_the_pinned_oracle_text`

## Focused evidence

- Intended canary red: `build/verification-logs/20261116-181227-285-Rust-scenario-red-desert-cycling-canary.log` (MissingCard or missing typed characteristic, exit 101).
- Focused all-card green: `build/verification-logs/20261116-195358-548-Rust-scenario-final-desert_cycling_batch.log` (exit 0).
- Registry green: `build/verification-logs/20261116-195258-911-Rust-registry-green-queue-v25-full.log` (full 893-test suite, exit 0).
- Review maps: `scripts/check-card-evidence.ps1 -Preparation` PASS for all 43 queue-v25 maps (card-evidence `417f2d7f-117f-48ee-9a38-b42e162da7c8`).
- Formatting: `cargo fmt --all -- --check` exit 0.
- Focused lint: `cargo clippy -p tricerules-cards -p tricerules-core --all-targets -- -D warnings` exit 0.
- Metadata refresh: only the 43 expected fingerprint rows and their `CARDS.md` checkboxes changed; `Savage Lands` was ejected to #517 and left unchecked.

- Conformance baseline: reviewed `unsupported: ability not offered by generic battlefield fixture` rows for the five hand-zone cycling abilities; `registry_execution_matches_reviewed_baseline` exit 0.
- Hand-only evidence: `desert_cycling_batch_cards_cycle_only_from_hand` exit 0.

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
