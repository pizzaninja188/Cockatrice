# Family: 06-blocks-pump (5 cards)

## Selection

Simple single-primitive cohort, 5 cards. Revalidated by Oracle ID and printed name against the
pinned bulk, live registry, maps, `CARDS.md`, `queue-exclusions.txt` and `blocked.tsv`.

## Exact cards

| Oracle ID | Card | Mana | Type line | P/T | Pump |
|---|---|---|---|---|---|
| 86002526-ed3d-47e5-9f2a-55c4fa220ce8 | Folk of An-Havva | {G} | Creature — Human | 1/1 | +2/+0 |
| a4109138-5d69-4510-a228-7c1f88c0c8c0 | Giant Badger | {1}{G}{G} | Creature — Badger | 2/2 | +2/+2 |
| e31510d5-b74e-4a8d-94e8-8bd098cac3c7 | Royal Trooper | {2}{W} | Creature — Human Soldier | 2/2 | +2/+2 |
| 0b6625cb-4177-45bd-ba99-2af43dfd6f46 | Shu Defender | {2}{W} | Creature — Human Soldier | 2/2 | +0/+2 |
| ec1ac4df-b031-43b4-a2de-ca58a1599053 | Town Sentry | {2}{W} | Creature — Human Soldier | 2/2 | +0/+2 |

## Copy-from and differences

`ezuris_archers` and `high-rise_sawjack` for `WheneverSelfBlocksCreature` plus a self `PumpTarget`.
The attacker filter is empty because the Oracle line has no restriction; the printed pump values
differ per card.

## Rules evidence

CR 509.3a (whenever this creature blocks) and 509.3b (the typed per-edge occurrence, equivalent
for these single-block creatures),
CR 611.2a and 514.2 (until-end-of-turn duration) and CR 613.4c (layer 7c self pump). No card in this
cohort has a pinned ruling. Known limitation carried from the shipped analogue: the typed trigger
fires per attacker-blocker edge, which matches the printed once-per-block wording for creatures
that can block only one attacker.

## Canaries

- Simplest: Folk of An-Havva (`{G}` 1/1, +2/+0).
- Hardest: Giant Badger (`{1}{G}{G}` 2/2, +2/+2).
- Every card is executed by the all-card block-pump, no-trigger-on-attack and exact-characteristics
  tests.

## Semantic evidence

- `scenario blocks_pump_batch::blocks_pump_batch_canary_cards_cast_with_exact_cost_and_body`
- `scenario blocks_pump_batch::blocks_pump_batch_canary_cards_pump_when_blocking`
- `scenario blocks_pump_batch::blocks_pump_batch_cards_cast_with_exact_printed_characteristics`
- `scenario blocks_pump_batch::blocks_pump_batch_cards_pump_when_blocking_every_printed_card`
- `scenario blocks_pump_batch::blocks_pump_batch_cards_do_not_pump_when_attacking_and_blocked`
- `mechanic_registry::blocks_pump_batch::blocks_pump_batch_maps_definitions`
- `mechanic_registry::blocks_pump_batch::blocks_pump_batch_fingerprints_match_the_pinned_oracle_text`

## Focused evidence

- Intended canary red: `build/verification-logs/20261116-144347-189-Rust-scenario-red-blocks_pump-canary.log`
  (MissingCard folk_of_an-havva, exit 101).
- Focused canary green: `build/verification-logs/20261116-144645-531-Rust-scenario-green-blocks_pump-canary.log`, exit 0.
- Focused all-card green: `build/verification-logs/20261116-144715-718-Rust-scenario-green-blocks_pump-all.log`, exit 0.
- Registry green: `build/verification-logs/20261116-144941-788-Rust-registry-green-blocks_pump.log`, exit 0.
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
