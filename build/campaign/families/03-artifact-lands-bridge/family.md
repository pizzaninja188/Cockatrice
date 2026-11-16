# Family: 03-artifact-lands-bridge (10 cards)

## Selection

Exact-text family, 10 cards. Revalidated by Oracle ID and printed name against the pinned bulk, live
registry, maps, `CARDS.md`, `queue-exclusions.txt` and `blocked.tsv`; see
`build/campaign/queue-v24/revalidation.tsv` and the packet under
`build/campaign/queue-v24/families/03-artifact-lands-bridge/`.

## Exact cards

| Oracle ID | Card | Mana options |
|---|---|---|
| 2065cada-4078-41c4-9e06-2460d2a2e8ee | Darkmoss Bridge | {B} / {G} |
| 44b83535-fdbf-4307-bf53-ca20470a768d | Drossforge Bridge | {B} / {R} |
| c9b7ea9c-3bcb-4538-aa25-cdb82a52037e | Goldmire Bridge | {W} / {B} |
| 33ee23bc-6327-4a54-a704-dfd83be36bb5 | Mistvault Bridge | {U} / {B} |
| 6cb37ac1-dd11-4a8c-bca5-ef44d828059f | Razortide Bridge | {W} / {U} |
| a3faf70d-c034-4692-9e92-1922029e3852 | Rustvale Bridge | {R} / {W} |
| 081bfd50-a436-463b-9d2c-5bc8a32b387c | Silverbluff Bridge | {U} / {R} |
| e040a8e6-b90c-42d1-a1b1-771d954c61ab | Slagwoods Bridge | {R} / {G} |
| 29cd8a7c-108a-43d1-af63-f603a27c24f2 | Tanglepool Bridge | {G} / {U} |
| 99720c65-be96-4220-8ed4-720660bf6928 | Thornglint Bridge | {G} / {W} |

## Copy-from and differences

`darksteel_citadel` for the artifact land with `Indestructible` and a `{T}` mana ability, and
`sun-blessed_peak` for the entered-tapped land shell. Per-card difference: the two printed mana
colors and their order. Every card is `Artifact Land`, enters tapped, is indestructible and has no
other text.

## Rules evidence

CR 614.1d/614.12 (enters tapped), CR 300.2 (an object with more than one card type combines each type's aspects),
CR 702.12b (indestructible permanents are not destroyed by lethal damage or destroy effects) and
CR 605.1a/605.3b (the dual activated mana ability resolves immediately). No card in this cohort has a pinned ruling.

## Canaries

- Simplest: Darkmoss Bridge (`{T}: Add {B} or {G}`).
- Hardest: Thornglint Bridge (`{T}: Add {G} or {W}`), the last printed pair of the cycle.
- Every card is executed by the all-card enter-tapped, both-mana-option, artifact-land identity and
  destroy-survival tests.

## Semantic evidence

- `scenario artifact_lands_bridge_batch::artifact_lands_bridge_batch_canary_cards_enter_tapped_and_tap_for_both_colors`
- `scenario artifact_lands_bridge_batch::artifact_lands_bridge_batch_canary_cards_are_indestructible_artifacts`
- `scenario artifact_lands_bridge_batch::artifact_lands_bridge_batch_cards_enter_tapped_and_reject_immediate_tap`
- `scenario artifact_lands_bridge_batch::artifact_lands_bridge_batch_cards_tap_for_each_printed_option_in_order`
- `scenario artifact_lands_bridge_batch::artifact_lands_bridge_batch_cards_keep_artifact_land_identity_and_indestructible`
- `scenario artifact_lands_bridge_batch::artifact_lands_bridge_batch_cards_survive_a_destroy_effect`
- `mechanic_registry::artifact_lands_bridge_batch::artifact_lands_bridge_batch_maps_definitions`
- `mechanic_registry::artifact_lands_bridge_batch::artifact_lands_bridge_batch_fingerprints_match_the_pinned_oracle_text`

The destroy-survival test casts the shipped `shatter` at the bridge, asserts the exact cost is
consumed, the spell resolves to the graveyard and the indestructible land stays on the battlefield.

## Focused evidence

- Intended canary red: `build/verification-logs/20261116-132028-280-Rust-scenario-red-artifact_lands_bridge-canary.log`
  (MissingCard darkmoss_bridge, exit 101). The preceding `132001` run exposed a compile-time test
  author error (`is_land` does not exist; fixed to `has_type("Land")`) before the intended red.
- Focused canary green: `build/verification-logs/20261116-132152-525-Rust-scenario-green-artifact_lands_bridge-canary.log`, exit 0.
- Focused all-card green: `build/verification-logs/20261116-132225-015-Rust-scenario-green-artifact_lands_bridge-all.log`, exit 0 (6 tests).
- Registry green: `build/verification-logs/20261116-132337-015-Rust-registry-green-artifact_lands_bridge.log`, exit 0 (2 tests).
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
