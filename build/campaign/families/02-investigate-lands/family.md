# Family: 02-investigate-lands (10 cards)

## Selection

Parameter family, 10 cards. Revalidated by Oracle ID and printed name against the pinned bulk, live
registry, maps, `CARDS.md`, `queue-exclusions.txt` and `blocked.tsv`; see
`build/campaign/queue-v24/revalidation.tsv` and the packet under
`build/campaign/queue-v24/families/02-investigate-lands/`.

## Exact cards

| Oracle ID | Card | Mana options | Third line |
|---|---|---|---|
| cb91e842-9f06-4863-a328-2cabe1bcfe27 | Ballroom | {W} / {B} | {4}, {T}: Investigate |
| a4fc174e-7fa6-41a8-ae03-255f226840f9 | Billiard Room | {B} / {R} | {4}, {T}: Investigate |
| 8f88b0bf-81bd-4223-9dad-d8c49ff4b87a | Conservatory | {G} / {W} | {4}, {T}: Investigate |
| 0880461e-8943-443b-90e7-ff84eef46550 | Dining Room | {R} / {G} | {4}, {T}: Investigate |
| 949e455d-6a9e-491a-892f-826cc8be0fd9 | Hall | {R} / {W} | {4}, {T}: Investigate |
| 47f71408-5509-4651-9121-fd0867adae00 | Kitchen | {G} / {U} | {4}, {T}: Investigate |
| 33a7d29a-ebc0-4606-91b6-7381e2a16016 | Library | {U} / {R} | {4}, {T}: Investigate |
| 98a22879-d0b2-441f-bcf6-a67b4552b73c | Lounge | {B} / {G} | {4}, {T}: Investigate |
| c6a46fc2-fc8f-4dcd-bca7-682abfbf303d | Secret Passage | {U} / {B} | {4}, {T}: Investigate |
| 7ffadb3f-0b88-416f-b1a1-876383c22720 | Study | {W} / {U} | {4}, {T}: Investigate |

## Copy-from and differences

`sun-blessed_peak` for the entered-tapped dual land shell and `callous_inspector` /
`air_nomad_legacy` for investigate expressed as `CreateTokens(token: "clue", count: 1)` on the
shipped `clue` token. Per-card difference: the two printed mana colors and their order; every card
creates exactly one Clue and prints the same reminder text.

## Rules evidence

CR 614.1d/614.12 (enters tapped), CR 605.1a/605.3a-b (mana ability resolves immediately), CR 701.16a (investigate creates one
Clue token), CR 111.10f (colorless Clue artifact token) and CR 602.1-2 (activation cost). The pinned
cycle rulings restate Clue artifact-type and sacrifice facts; none changes the typed shape.

## Canaries

- Simplest: Study (`{T}: Add {W} or {U}`).
- Hardest: Ballroom (`{T}: Add {W} or {B}`), the first printed pair of the cycle.
- Every card is executed by the all-card enter-tapped, both-mana-option, one-Clue-per-activation
  and rejected-without-four-mana tests.

## Semantic evidence

- `scenario investigate_lands_batch::investigate_lands_batch_canary_cards_enter_tapped_and_tap_for_both_colors`
- `scenario investigate_lands_batch::investigate_lands_batch_canary_cards_investigate_for_one_clue`
- `scenario investigate_lands_batch::investigate_lands_batch_cards_enter_tapped_and_reject_immediate_activation`
- `scenario investigate_lands_batch::investigate_lands_batch_cards_tap_for_each_printed_option_in_order`
- `scenario investigate_lands_batch::investigate_lands_batch_cards_create_one_clue_per_activation`
- `scenario investigate_lands_batch::investigate_lands_batch_cards_reject_investigate_without_four_mana`
- `mechanic_registry::investigate_lands_batch::investigate_lands_batch_maps_definitions`
- `mechanic_registry::investigate_lands_batch::investigate_lands_batch_fingerprints_match_the_pinned_oracle_text`

The Clue identity assertions cover the token's name, `Artifact` + `Clue` types, empty P/T, absence
of creature identity, controller scope and repeatability.

## Focused evidence

- Intended canary red: `build/verification-logs/20261116-131632-396-Rust-scenario-red-investigate_lands-canary.log`
  (MissingCard ballroom, exit 101).
- Focused canary green: `build/verification-logs/20261116-131721-867-Rust-scenario-green-investigate_lands-canary.log`, exit 0.
- Focused all-card green: `build/verification-logs/20261116-131801-989-Rust-scenario-green-investigate_lands-all.log`, exit 0 (6 tests).
- Registry green: `build/verification-logs/20261116-131924-981-Rust-registry-green-investigate_lands.log`, exit 0 (2 tests).
- Review maps: `scripts/check-card-evidence.ps1 -Preparation` PASS (1852 maps).
- Metadata refresh: only the ten expected fingerprint rows and their `CARDS.md` checkboxes changed.

## Review and gate

Frozen in `build/campaign/review/queue-v24-batch-a-v3` (patch SHA-256
`9101aa6087039f1e53d771f0e109ac9e34a53224eaa0f9efcf6b99cf97b99fca`). Independent read-only review approved the family for reviewed semantics; the material
findings were CR-citation corrections (investigate CR 701.16a and Clue 111.10f, artifact-land
multi-type CR 300.2, immediate mana-ability resolution 605.3a-b) plus two optional evidence
assertions, all corrected and delta-approved. Batch queue-v24 A full gate
`verify-e88116c5-a9d7-41be-bba9-dd225c267ac9` passed exit 0, covering 01-sac-lands-cc..03-artifact-lands-bridge; commit
`bdaa3b7e6324ce044a5ecffc735a54b69d0f0f29`; `origin/card-campaign` verified at the same SHA.
