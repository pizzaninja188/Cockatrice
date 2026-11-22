# Family: 08-cameo-artifacts (5 cards)

## Selection

Queue-v25 Simple single-primitive cohort, 5 cards. Revalidated by Oracle ID and printed name against the pinned bulk,
live registry, maps, `CARDS.md`, `queue-exclusions.txt` and `blocked.tsv`; see
`build/campaign/queue-v25/revalidation.tsv`.

Stable typed shape: `{3}` artifact with one `{T}` mana ability offering two printed colors.

## Exact cards

| Oracle ID | Card | Mana | Type line | P/T | Oracle text |
|---|---|---|---|---|---|
| 1ce6ae30-33c3-4f05-9286-69b0871b1c2d | Bloodstone Cameo | {3} | Artifact |  | {T}: Add {B} or {R}. |
| 8fbdec25-4222-4b96-aeea-82a4b5b8b80e | Drake-Skull Cameo | {3} | Artifact |  | {T}: Add {U} or {B}. |
| 383f6020-c26d-43e8-bb07-566886626d74 | Seashell Cameo | {3} | Artifact |  | {T}: Add {W} or {U}. |
| d2be289e-e560-405d-9728-d8a4ee9cbf56 | Tigereye Cameo | {3} | Artifact |  | {T}: Add {G} or {W}. |
| 98e042de-05f4-4e2e-b12f-375b905e6600 | Troll-Horn Cameo | {3} | Artifact |  | {T}: Add {R} or {G}. |

## Copy-from and differences

Compose `deathcap_glade`'s two-option mana shape with `alloy_myr`'s artifact mana ability; the artifact has no other text.

## Rules evidence

CR 605.1a/605.3a-b (artifact mana ability, immediate resolution) and 118.3 (tap cost). The cycle has no pinned ruling.

Pinned rulings are saved per Oracle ID under
`build/campaign/queue-v25/families/08-cameo-artifacts/rulings/`.

## Canaries

- Simplest: Bloodstone Cameo (`bloodstone_cameo`).
- Hardest: Troll-Horn Cameo (`troll-horn_cameo`).
- Every card in the family is executed by the shared all-card scenarios below.

## Semantic evidence

- `scenario cameo_artifacts_batch::cameo_artifacts_batch_cards_produce_each_printed_color_and_tap`
- `scenario cameo_artifacts_batch::cameo_artifacts_batch_cards_reject_a_second_activation_while_tapped`
- `mechanic_registry::cameo_artifacts_batch::cameo_artifacts_batch_maps_definitions`
- `mechanic_registry::cameo_artifacts_batch::cameo_artifacts_batch_fingerprints_match_the_pinned_oracle_text`

## Focused evidence

- Intended canary red: `build/verification-logs/20261116-182009-351-Rust-scenario-red-cameo-artifacts-canary.log` (MissingCard or missing typed characteristic, exit 101).
- Focused all-card green: `build/verification-logs/20261116-195405-227-Rust-scenario-final-cameo_artifacts_batch.log` (exit 0).
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
