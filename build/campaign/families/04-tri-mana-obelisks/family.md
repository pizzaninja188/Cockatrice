# Family: 04-tri-mana-obelisks (7 cards)

## Selection

Parameter family, 7 cards. Revalidated by Oracle ID and printed name against the pinned bulk, live
registry, maps, `CARDS.md`, `queue-exclusions.txt` and `blocked.tsv`; see
`build/campaign/queue-v24/revalidation.tsv` and the packet beside this file.

## Exact cards

| Oracle ID | Card | Mana | Type line | P/T | Ability |
|---|---|---|---|---|---|
| 3f96dfd3-975a-4595-95c1-d2f04a86cb37 | Obelisk of Bant | {3} | Artifact | — | {T}: Add {G}, {W}, or {U}. |
| 9da963f6-734b-4d3c-a3be-dd5cca2c7b19 | Obelisk of Esper | {3} | Artifact | — | {T}: Add {W}, {U}, or {B}. |
| e193ae18-91a0-4870-b8a1-f8dedf744708 | Obelisk of Grixis | {3} | Artifact | — | {T}: Add {U}, {B}, or {R}. |
| eb0be178-55f9-4743-8128-34bc86f134ec | Obelisk of Jund | {3} | Artifact | — | {T}: Add {B}, {R}, or {G}. |
| 02e81bb8-ba80-4483-9b9f-973dcae0fc9d | Obelisk of Naya | {3} | Artifact | — | {T}: Add {R}, {G}, or {W}. |
| 8d7933c4-4c74-492f-a9cd-50b339a4b29e | Druid of the Anima | {1}{G} | Creature — Elf Druid | 1/1 | {T}: Add {R}, {G}, or {W}. |
| 9c382817-5c62-44ff-8f29-d283e65712a1 | Urborg Elf | {1}{G} | Creature — Elf Druid | 1/1 | {T}: Add {B}, {G}, or {U}. |

## Copy-from and differences

`arcane_sanctum` for three single-color `ProduceMana` options and `azorius_signet` for an artifact
body with a mana ability. Five cards are `{3}` artifacts; two are `{1}{G}` 1/1 Elf Druids, so the
creature tap ability is subject to CR 302.6 summoning sickness while the artifacts are not.

## Rules evidence

CR 605.1a/605.3a-b (targetless activated mana ability resolves immediately), CR 106.1a-b (one mana
of the chosen printed color), CR 202.1 (printed mana cost) and CR 302.6/602.5a (a summoning-sick
creature cannot pay a tap cost). No card in this cohort has a pinned
ruling.

## Canaries

- Simplest: Obelisk of Bant (artifact body, `{T}` for one of {G}, {W}, {U}).
- Hardest: Druid of the Anima (creature body with `{1}{G}`, so summoning sickness blocks the tap
  ability until the controller's next untap step).
- Every card is executed by the all-card option-order, tapped-source and summoning-sickness tests.

## Semantic evidence

- `scenario tri_mana_obelisks_batch::tri_mana_obelisks_batch_canary_cards_cast_and_offer_each_printed_color`
- `scenario tri_mana_obelisks_batch::tri_mana_obelisks_batch_cards_tap_for_each_printed_option_in_order`
- `scenario tri_mana_obelisks_batch::tri_mana_obelisks_batch_cards_reject_activation_when_already_tapped`
- `scenario tri_mana_obelisks_batch::tri_mana_obelisks_batch_artifacts_ignore_summoning_sickness_but_creatures_do_not`
- `mechanic_registry::tri_mana_obelisks_batch::tri_mana_obelisks_batch_maps_definitions`
- `mechanic_registry::tri_mana_obelisks_batch::tri_mana_obelisks_batch_fingerprints_match_the_pinned_oracle_text`

## Focused evidence

- Intended canary red: `build/verification-logs/20261116-144110-996-Rust-scenario-red-tri_mana_obelisks-canary.log`
  (MissingCard obelisk_of_bant, exit 101).
- Focused canary green: `build/verification-logs/20261116-144605-697-Rust-scenario-green-tri_mana_obelisks-canary.log`, exit 0.
- Focused all-card green: `build/verification-logs/20261116-144651-072-Rust-scenario-green-tri_mana_obelisks-all.log`, exit 0.
- Registry green: `build/verification-logs/20261116-144900-489-Rust-registry-green-tri_mana_obelisks.log`, exit 0.
- Review maps: `scripts/check-card-evidence.ps1 -Preparation` PASS (1869 maps).
- Delta rechecks after citation corrections: `build/verification-logs/20261116-151636-814-Rust-scenario-recheck-tri_mana_obelisks.log`,
  `build/verification-logs/20261116-151708-399-Rust-scenario-recheck-sac_creature_three.log`,
  `build/verification-logs/20261116-151710-101-Rust-scenario-recheck-blocks_pump.log` and the three
  matching registry rechecks, all exit 0.
- Metadata refresh: only the 17 expected batch B fingerprint rows and their `CARDS.md` checkboxes changed.

## Review and gate

Frozen in `build/campaign/review/queue-v24-batch-b-v2` (patch SHA-256
`61022a780ac5d3882ff2c1ba7bce42059a576de1aeefb06bd0173ee14d70c9ec`). Independent read-only review approved the family for reviewed semantics; the
optional citation-precision notes (CR 509.3a/509.3b, 205.2b, 302.6/602.5a) were corrected and
delta-approved. Batch queue-v24 B full gate `verify-91c38a90-a9fa-4ac3-b814-5982714130b3` passed exit 0,
covering 04-tri-mana-obelisks..06-blocks-pump; commit `361204474b06feffc78330b16da44f088f3fc5f6`; `origin/card-campaign` verified at
the same SHA.
