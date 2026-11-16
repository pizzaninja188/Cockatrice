# Family: 01-sac-lands-cc (10 cards)

## Selection

Simple single-primitive cohort, 10 cards. Revalidated by Oracle ID and printed name against the
pinned bulk, live registry, maps, `CARDS.md`, `queue-exclusions.txt` and `blocked.tsv`; see
`build/campaign/queue-v24/revalidation.tsv` and the packet under
`build/campaign/queue-v24/families/01-sac-lands-cc/`.

## Exact cards

| Oracle ID | Card | Single mana | Sacrifice pair |
|---|---|---|---|
| 2160849a-184f-4a49-8931-fd021e16f7cb | Ancient Spring | {U} | {W}{B} |
| cbef6ad5-717f-4b02-9c62-e5deba407ad1 | Dwarven Ruins | {R} | {R}{R} |
| 4d4b9512-d29d-4b99-9249-d37e1199a1f5 | Ebon Stronghold | {B} | {B}{B} |
| 83f9d84a-a011-4fa9-a3a4-51eb39159c54 | Geothermal Crevice | {R} | {B}{G} |
| 18cbb47a-85b1-48f6-a024-8c3bbffa0d87 | Havenwood Battleground | {G} | {G}{G} |
| e0c3c87b-83e3-4cff-b05f-78a192e2bba4 | Irrigation Ditch | {W} | {G}{U} |
| ad7b610c-e276-4c71-9b53-1ccb6d2dafd2 | Ruins of Trokair | {W} | {W}{W} |
| bc2906cd-9e0c-4aa8-b656-fb1045d810ac | Sulfur Vent | {B} | {U}{R} |
| 5578cf33-62f5-456c-a58c-f744a25df79b | Svyelunite Temple | {U} | {U}{U} |
| af0ad159-8264-4526-9a95-eddd32c0a13f | Tinder Farm | {G} | {R}{W} |

## Copy-from and differences

`sun-blessed_peak` for the entered-tapped land with two tap abilities, `chromatic_star` for a
`Mana + SacrificeSelf` mana ability and the signets for a fixed multi-color `ProduceMana` option.
Per-card difference: the printed single color and the sacrificed pair; five cards produce two mana
of one color, five a two-color pair. Every card is a normal-layout `Land` with no other text.

## Rules evidence

CR 614.1d/614.12 (enters tapped), CR 605.1a/605.3a-b (mana abilities resolve immediately),
CR 118.3 and 701.21a (sacrifice as a cost), CR 106.1a-b (the produced mana's colors and types).
No card in this cohort has a pinned ruling.

## Canaries

- Simplest: Dwarven Ruins (`{T}: Add {R}`; `{T}, Sacrifice: Add {R}{R}`).
- Hardest: Ancient Spring (`{T}: Add {U}`; `{T}, Sacrifice: Add {W}{B}`).
- Every card is executed by the all-card enter-tapped, both-mana-option, sacrifice-destination and
  tapped-source rejection tests.

## Semantic evidence

- `scenario sac_lands_cc_batch::sac_lands_cc_batch_canary_cards_enter_tapped_and_tap_for_the_single_color`
- `scenario sac_lands_cc_batch::sac_lands_cc_batch_canary_cards_sacrifice_for_the_printed_pair`
- `scenario sac_lands_cc_batch::sac_lands_cc_batch_cards_enter_tapped_and_reject_immediate_tap`
- `scenario sac_lands_cc_batch::sac_lands_cc_batch_cards_tap_for_exactly_the_printed_single_color`
- `scenario sac_lands_cc_batch::sac_lands_cc_batch_cards_sacrifice_for_exactly_the_printed_pair`
- `scenario sac_lands_cc_batch::sac_lands_cc_batch_cards_do_not_sacrifice_when_the_tap_cost_is_unavailable`
- `mechanic_registry::sac_lands_cc_batch::sac_lands_cc_batch_maps_definitions`
- `mechanic_registry::sac_lands_cc_batch::sac_lands_cc_batch_fingerprints_match_the_pinned_oracle_text`

## Focused evidence

- Intended canary red: `build/verification-logs/20261116-130038-094-Rust-scenario-red-sac_lands_cc-canary.log`
  (MissingCard ancient_spring, exit 101).
- Focused canary green: `build/verification-logs/20261116-130110-265-Rust-scenario-green-sac_lands_cc-canary.log`, exit 0.
- Focused all-card green: `build/verification-logs/20261116-130226-607-Rust-scenario-green-sac_lands_cc-all.log`, exit 0 (6 tests).
- Registry green: `build/verification-logs/20261116-130411-329-Rust-registry-green-sac_lands_cc.log`, exit 0 (2 tests).
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
