# Family: 07-becomes-target-sacrifice (5 cards)

## Selection

Simple single-primitive cohort, 5 cards. Revalidated by Oracle ID and printed name against the
pinned bulk, live registry, maps, `CARDS.md`, `queue-exclusions.txt` and `blocked.tsv`.

## Exact cards

| Oracle ID | Card | Mana | Type line | P/T | Oracle text |
|---|---|---|---|---|---|
| 54a22814-c0d5-4637-9aa9-a6bc96401914 | Frost Walker | {1}{U} | Creature — Elemental | 4/1 | When this creature becomes the target of a spell or ability, sacrifice it. |
| 780da643-8d14-4f1a-a298-5b7454e9c4df | Phantasmal Bear | {U} | Creature — Bear Illusion | 2/2 | When this creature becomes the target of a spell or ability, sacrifice it. |
| 13ca7c14-5db2-4d17-912a-fdeabcdcd9f8 | Phantom Beast | {3}{U} | Creature — Illusion Beast | 4/5 | When this creature becomes the target of a spell or ability, sacrifice it. |
| 4178ae56-2449-4617-aff8-f59d8fb96c66 | Skulking Fugitive | {2}{B} | Creature — Horror Mercenary | 3/4 | When this creature becomes the target of a spell or ability, sacrifice it. |
| 05a7ca83-e820-433f-b9e9-151e817d3708 | Tar Pit Warrior | {2}{B} | Creature — Cyclops Warrior | 3/4 | When this creature becomes the target of a spell or ability, sacrifice it. |

## Copy-from and differences

The shipped Illusion token `illusion_u_2_2_spell_target_sacrifice` for the exact trigger/effect
pair. Oracle says "a spell or ability" with no controller restriction, so the trigger uses
`source_controller: AnyPlayer`, unlike the shipped token's spell-only, any-player trigger. Bodies
and costs differ per card.

## Rules evidence

CR 603.2 (trigger on becoming a target), CR 603.3 and 608.1 (the trigger becomes the topmost stack object and resolves before the
targeting spell or ability), CR 701.21a (sacrifice) and CR 608.2b (a spell or ability whose only target has left is
countered on resolution). The pinned Frost Walker and Phantasmal Bear rulings restate this ordering. The canary uses Sorin's
Thirst so the fizzle is observable through the caster's unchanged life total.

## Canaries

- Simplest: Phantasmal Bear (`{U}` 2/2).
- Hardest: Phantom Beast (`{3}{U}` 4/5).
- Every card is executed by the all-card spell-target, ability-target, own-controller and
  no-target tests.

## Semantic evidence

- `scenario becomes_target_sacrifice_batch::becomes_target_sacrifice_batch_canary_cards_sacrifice_to_a_targeting_spell`
- `scenario becomes_target_sacrifice_batch::becomes_target_sacrifice_batch_cards_cast_with_exact_cost_and_body`
- `scenario becomes_target_sacrifice_batch::becomes_target_sacrifice_batch_cards_sacrifice_to_a_targeting_ability`
- `scenario becomes_target_sacrifice_batch::becomes_target_sacrifice_batch_cards_trigger_for_their_own_controller`
- `scenario becomes_target_sacrifice_batch::becomes_target_sacrifice_batch_cards_do_not_trigger_without_a_target`
- `mechanic_registry::becomes_target_sacrifice_batch::becomes_target_sacrifice_batch_maps_definitions`
- `mechanic_registry::becomes_target_sacrifice_batch::becomes_target_sacrifice_batch_fingerprints_match_the_pinned_oracle_text`

## Focused evidence

- Intended canary red: `build/verification-logs/20261116-160519-270-Rust-scenario-red-becomes_target_sacrifice-canary.log`
  (MissingCard phantasmal_bear, exit 101).
- Focused canary green: `build/verification-logs/20261116-160621-785-Rust-scenario-green-becomes_target-canary.log`, exit 0.
- Focused all-card green: `build/verification-logs/20261116-160705-497-Rust-scenario-green-becomes_target-all.log`, exit 0.
- Registry green: `build/verification-logs/20261116-160933-937-Rust-registry-green-becomes_target_sacrifice.log`, exit 0.
- Review maps: `scripts/check-card-evidence.ps1 -Preparation` PASS (1884 maps).
- Final focused green after review corrections: `build/verification-logs/20261116-170622-342-Rust-scenario-final2-becomes_target.log`;
  earlier delta run `build/verification-logs/20261116-165042-446-Rust-scenario-final-becomes_target_sacrifice.log`
  and `build/verification-logs/20261116-165121-964-Rust-registry-final-becomes_target_sacrifice.log`, exit 0.
- Metadata refresh: only the 15 expected batch C fingerprint rows and their `CARDS.md` checkboxes changed.

## Review and gate

Frozen in `build/campaign/review/queue-v24-batch-c-v3` (patch SHA-256
`becad7ab49cde35d7497f6db7b0cdf8368e517b44ffb85e99949fee95485d534`). Independent read-only review approved the family for reviewed semantics; material
findings (real power-3 target rejection, a non-targeting-spell case, an observable Sorin's Thirst
fizzle, resolution-scope evidence, CR 603.3/608.1 and 509.1b/509.1h citations) were corrected and
delta-approved. Batch queue-v24 C full gate `verify-6c12deb5-91a1-4aa0-9668-1777fc8916b2` passed exit 0,
covering 07-becomes-target-sacrifice..09-unblockable-power-two; commit `5a9717003252d7cb9f489a08036c74535c4be472`;
`origin/card-campaign` verified at the same SHA.
