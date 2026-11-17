# Family: 07-sac-creature-self-pump (4 cards)

## Selection

Queue-v25 Simple single-primitive cohort, 4 cards. Revalidated by Oracle ID and printed name against the pinned bulk,
live registry, maps, `CARDS.md`, `queue-exclusions.txt` and `blocked.tsv`; see
`build/campaign/queue-v25/revalidation.tsv`.

Stable typed shape: Creature with `Sacrifice a creature: This creature gets +2/+2 until end of turn`.

## Exact cards

| Oracle ID | Card | Mana | Type line | P/T | Oracle text |
|---|---|---|---|---|---|
| aa76cc50-5be0-4844-b3a4-fb33fca3cdd3 | Bloodthrone Vampire | {1}{B} | Creature — Vampire | 1/1 | Sacrifice a creature: This creature gets +2/+2 until end of turn. |
| 0dcefc00-9425-43e3-bd47-1e34fff8b0e2 | Nantuko Husk | {2}{B} | Creature — Zombie Insect | 2/2 | Sacrifice a creature: This creature gets +2/+2 until end of turn. |
| 748c2723-dc59-4aa7-9667-d4679448c8b6 | Phyrexian Ghoul | {2}{B} | Creature — Phyrexian Zombie | 2/2 | Sacrifice a creature: This creature gets +2/+2 until end of turn. |
| c14739b0-5c19-4873-b027-314225a10f7b | Vampire Aristocrat | {2}{B} | Creature — Vampire Rogue Noble | 2/2 | Sacrifice a creature: This creature gets +2/+2 until end of turn. |

## Copy-from and differences

Compose `frozen_shade`'s `PumpTarget(subject: Source)` with `vampiric_rites`' `SacrificePermanent(filter: (kind: Creature, controller: You))` cost.

## Rules evidence

CR 602.1a/602.2 (activated ability and cost), 701.21a (sacrifice as a cost), 611.2a (until-end-of-turn effect) and 514.2 (cleanup). The pinned rulings confirm that the source may sacrifice itself but is then not on the battlefield to get the bonus.

Pinned rulings are saved per Oracle ID under
`build/campaign/queue-v25/families/07-sac-creature-self-pump/rulings/`.

## Canaries

- Simplest: Bloodthrone Vampire (`bloodthrone_vampire`).
- Hardest: Vampire Aristocrat (`vampire_aristocrat`).
- Every card in the family is executed by the shared all-card scenarios below.

## Semantic evidence

- `scenario sac_creature_self_pump_batch::sac_creature_self_pump_batch_cards_sacrifice_another_creature_and_pump`
- `scenario sac_creature_self_pump_batch::sac_creature_self_pump_batch_cards_can_sacrifice_themselves_without_a_bonus`
- `scenario sac_creature_self_pump_batch::sac_creature_self_pump_batch_cards_need_no_tap_and_ignore_summoning_sickness`
- `mechanic_registry::sac_creature_self_pump_batch::sac_creature_self_pump_batch_maps_definitions`
- `mechanic_registry::sac_creature_self_pump_batch::sac_creature_self_pump_batch_fingerprints_match_the_pinned_oracle_text`

## Focused evidence

- Intended canary red: `build/verification-logs/20261116-181833-461-Rust-scenario-red-sac-self-pump-canary.log` (MissingCard or missing typed characteristic, exit 101).
- Focused all-card green: `build/verification-logs/20261116-200917-458-Rust-scenario-final2-sac_creature_self_pump_batch.log` (exit 0).
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
