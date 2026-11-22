# Family: 09-unblockable-power-two (5 cards)

## Selection

Simple single-primitive cohort, 5 cards. Revalidated by Oracle ID and printed name against the
pinned bulk, live registry, maps, `CARDS.md`, `queue-exclusions.txt` and `blocked.tsv`.

## Exact cards

| Oracle ID | Card | Mana | Type line | P/T | Oracle text |
|---|---|---|---|---|---|
| c4020178-71d9-4363-a0bd-b9a7eecd5d02 | Crafty Pathmage | {2}{U} | Creature — Human Wizard | 1/1 | {T}: Target creature with power 2 or less can't be blocked this turn. |
| db130ff4-7525-421f-b7e7-200921ecbdec | Dwarven Nomad | {2}{R} | Creature — Dwarf Nomad | 1/1 | {T}: Target creature with power 2 or less can't be blocked this turn. |
| cfc553cd-3b4c-47a9-bffb-e5790befb32c | Dwarven Warriors | {2}{R} | Creature — Dwarf Warrior | 1/1 | {T}: Target creature with power 2 or less can't be blocked this turn. |
| 1005bc16-8a6a-476a-a647-4ed40d2e96de | Goblin Tunneler | {1}{R} | Creature — Goblin Rogue | 1/1 | {T}: Target creature with power 2 or less can't be blocked this turn. |
| c0247a19-9c8f-422c-88cf-9f298c3a8c96 | Pathmaker Initiate | {1}{R} | Creature — Human Wizard | 2/1 | {T}: Target creature with power 2 or less can't be blocked this turn. |

## Copy-from and differences

`ragged_playmate` and the reviewed `targeted_evasion_batch` for the chosen-creature evasion
restriction; these cards use a tap-only activation and restrict the target to power 2 or less. The
printed bodies and mana costs differ.

## Rules evidence

CR 115.1 (targeting), CR 509.1a-b (evasion is checked as blockers are declared), CR 608.2b (a target
whose power is above 2 on resolution is illegal and the ability does not resolve) and CR 611.2a.
The pinned Pathmaker Initiate and Goblin Tunneler rulings restate the resolution and
post-resolution cases. Resolution-time power revalidation is exercised by the existing primitive test
`blocking_restrictions::goblin_smuggler_uses_derived_power_and_revalidates_its_target`; the
actual-card tests cover pre-resolution target filtering, post-resolution pump persistence and the
non-undo of an existing block.

## Canaries

- Simplest: Goblin Tunneler (`{1}{R}` 1/1).
- Hardest: Crafty Pathmage (`{2}{U}` 1/1) for the blue cost.
- Every card is executed by the all-card unblockable, illegal-target, later-pump, existing-block and
  tapped-source tests.

## Semantic evidence

- `scenario unblockable_power_two_batch::unblockable_power_two_batch_canary_cards_make_a_power_two_creature_unblockable`
- `scenario unblockable_power_two_batch::unblockable_power_two_batch_cards_cast_with_exact_cost_and_body`
- `scenario unblockable_power_two_batch::unblockable_power_two_batch_cards_reject_a_target_above_power_two`
- `scenario unblockable_power_two_batch::unblockable_power_two_batch_cards_keep_the_restriction_after_a_later_pump`
- `scenario unblockable_power_two_batch::unblockable_power_two_batch_cards_do_not_undo_an_existing_block`
- `scenario unblockable_power_two_batch::unblockable_power_two_batch_cards_reject_activation_when_the_source_is_tapped`
- `mechanic_registry::unblockable_power_two_batch::unblockable_power_two_batch_maps_definitions`
- `mechanic_registry::unblockable_power_two_batch::unblockable_power_two_batch_fingerprints_match_the_pinned_oracle_text`

## Focused evidence

- Intended canary red: `build/verification-logs/20261116-160522-713-Rust-scenario-red-unblockable_power_two-canary.log`
  (MissingCard crafty_pathmage, exit 101).
- Focused canary green: `build/verification-logs/20261116-160608-033-Rust-scenario-green-unblockable_power_two-canary.log`, exit 0.
- Focused all-card green: `build/verification-logs/20261116-160833-569-Rust-scenario-green-unblockable_power_two-all.log`, exit 0.
- Registry green: `build/verification-logs/20261116-161015-817-Rust-registry-green-unblockable_power_two.log`, exit 0.
- Review maps: `scripts/check-card-evidence.ps1 -Preparation` PASS (1884 maps).
- Final focused green after review corrections: `build/verification-logs/20261116-165116-521-Rust-scenario-final-unblockable_power_two.log`
  and `build/verification-logs/20261116-165131-610-Rust-registry-final-unblockable_power_two.log`, exit 0.
- Metadata refresh: only the 15 expected batch C fingerprint rows and their `CARDS.md` checkboxes changed.

## Review and gate

Frozen in `build/campaign/review/queue-v24-batch-c-v3` (patch SHA-256
`becad7ab49cde35d7497f6db7b0cdf8368e517b44ffb85e99949fee95485d534`). Independent read-only review approved the family for reviewed semantics; material
findings (real power-3 target rejection, a non-targeting-spell case, an observable Sorin's Thirst
fizzle, resolution-scope evidence, CR 603.3/608.1 and 509.1b/509.1h citations) were corrected and
delta-approved. Batch queue-v24 C full gate `verify-6c12deb5-91a1-4aa0-9668-1777fc8916b2` passed exit 0,
covering 07-becomes-target-sacrifice..09-unblockable-power-two; commit `5a9717003252d7cb9f489a08036c74535c4be472`;
`origin/card-campaign` verified at the same SHA.
