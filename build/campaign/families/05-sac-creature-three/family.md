# Family: 05-sac-creature-three (5 cards)

## Selection

Simple single-primitive cohort, 5 cards. Revalidated by Oracle ID and printed name against the
pinned bulk, live registry, maps, `CARDS.md`, `queue-exclusions.txt` and `blocked.tsv`.

## Exact cards

| Oracle ID | Card | Type line | P/T | Ability |
|---|---|---|---|---|
| 6223b1c0-bfe0-490c-b2d4-28537b05f571 | Crosis's Attendant | Artifact Creature — Golem | 3/3 | {1}, Sacrifice this creature: Add {U}{B}{R}. |
| c0ad16d7-2550-47e0-8581-0e22b27cb0d0 | Darigaaz's Attendant | Artifact Creature — Golem | 3/3 | {1}, Sacrifice this creature: Add {B}{R}{G}. |
| 91d36a9d-3a3f-4cb7-837d-b1e8f6730512 | Dromar's Attendant | Artifact Creature — Golem | 3/3 | {1}, Sacrifice this creature: Add {W}{U}{B}. |
| 781542b7-155b-4a66-aa52-eec0ebb29bb2 | Rith's Attendant | Artifact Creature — Golem | 3/3 | {1}, Sacrifice this creature: Add {R}{G}{W}. |
| ff7d927f-0a01-4243-ad00-c65686ee86bb | Treva's Attendant | Artifact Creature — Golem | 3/3 | {1}, Sacrifice this creature: Add {G}{W}{U}. |

## Copy-from and differences

`chromatic_star` for `Mana + SacrificeSelf` producing mana, `azorius_signet` for a fixed multi-color
`ProduceMana` option, and `coal_golem`/`satyr_hedonist` as the closer `SacrificeSelf` comparisons.
The only printed difference is the three produced colors.

## Rules evidence

CR 605.1a/605.3a-b (the activated mana ability resolves immediately), CR 118.3 and 701.21a
(sacrifice as a cost), CR 106.1a-b (the produced mana's colors and types) and CR 205.2b (an object with more than one
card type). No card in this cohort has a pinned ruling.

## Canaries

- Simplest: Treva's Attendant ({G}{W}{U}).
- Hardest: Crosis's Attendant ({U}{B}{R}) for the remaining shard order.
- Every card is executed by the all-card sacrifice-destination, exact-mana, missing-cost and
  stale-source tests.

## Semantic evidence

- `scenario sac_creature_three_batch::sac_creature_three_batch_canary_cards_cast_with_the_printed_body`
- `scenario sac_creature_three_batch::sac_creature_three_batch_canary_cards_sacrifice_for_the_printed_mana`
- `scenario sac_creature_three_batch::sac_creature_three_batch_cards_sacrifice_for_the_printed_mana`
- `scenario sac_creature_three_batch::sac_creature_three_batch_cards_reject_the_activation_without_one_mana`
- `scenario sac_creature_three_batch::sac_creature_three_batch_cards_cannot_reactivate_a_sacrificed_attendant`
- `mechanic_registry::sac_creature_three_batch::sac_creature_three_batch_maps_definitions`
- `mechanic_registry::sac_creature_three_batch::sac_creature_three_batch_fingerprints_match_the_pinned_oracle_text`

## Focused evidence

- Intended canary red: `build/verification-logs/20261116-144345-446-Rust-scenario-red-sac_creature_three-canary.log`
  (MissingCard crosiss_attendant, exit 101).
- Focused canary green: `build/verification-logs/20261116-144643-904-Rust-scenario-green-sac_creature_three-canary.log`, exit 0.
- Focused all-card green: `build/verification-logs/20261116-144653-060-Rust-scenario-green-sac_creature_three-all.log`, exit 0.
- Registry green: `build/verification-logs/20261116-144940-216-Rust-registry-green-sac_creature_three.log`, exit 0.
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
