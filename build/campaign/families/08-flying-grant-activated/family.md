# Family: 08-flying-grant-activated (5 cards)

## Selection

Parameter family, 5 cards. Revalidated by Oracle ID and printed name against the pinned bulk, live
registry, maps, `CARDS.md`, `queue-exclusions.txt` and `blocked.tsv`.

## Exact cards

| Oracle ID | Card | Mana | Type line | P/T | Activation |
|---|---|---|---|---|---|
| 1e50af57-e49d-4b94-a370-44846d9f6b33 | Cobalt Golem | {4} | Artifact Creature — Golem | 2/3 | {1}{U} |
| 298a71d4-6dc1-444e-9057-0d25aef7fb64 | Kor Sky Climber | {2}{W} | Creature — Kor Soldier Ally | 3/2 | {1}{W} |
| 1b5c2ba0-3df7-4394-a3f1-8e8330d7ac81 | Leaping Master | {1}{R} | Creature — Human Monk | 2/1 | {2}{W} |
| c2f2fd39-4a21-489b-893e-4258f8d2aa63 | Roofstalker Wight | {1}{B} | Creature — Zombie | 2/1 | {1}{U} |
| f53f33ed-5c8c-4835-bc5f-65a8acfa3f5b | Steeple Creeper | {2}{G} | Creature — Frog Snake | 4/2 | {3}{U} |

All five Oracle texts are "{cost}: This creature gains flying until end of turn."

## Copy-from and differences

`dukhara_peafowl` and the reviewed `blue_gain_flying_batch` for
`GrantKeywords(subject: Source, keywords: [Flying])` on a mana-only activated ability. The
activation cost and printed body differ; the grant is always exactly flying.

## Rules evidence

CR 602.1-2 (activated ability and printed cost), CR 702.9a-b (flying evasion), CR 611.2a
(until-end-of-turn grant), CR 509.1b and 509.1h (a grant after blocking does not undo the block) and CR 514.2
(cleanup ends the grant). The pinned Leaping Master ruling restates the post-block case.

## Canaries

- Simplest: Cobalt Golem (`{4}` 2/3; `{1}{U}`).
- Hardest: Steeple Creeper (`{2}{G}` 4/2; `{3}{U}`) for the largest activation cost.
- Every card is executed by the all-card grant, blocking, cleanup and rejected-cost tests.

## Semantic evidence

- `scenario flying_grant_activated_batch::flying_grant_activated_batch_canary_cards_gain_flying_only_after_activation`
- `scenario flying_grant_activated_batch::flying_grant_activated_batch_cards_lose_flying_at_cleanup`
- `scenario flying_grant_activated_batch::flying_grant_activated_batch_cards_cast_with_exact_printed_characteristics`
- `scenario flying_grant_activated_batch::flying_grant_activated_batch_cards_gain_flying_for_the_exact_activation_cost`
- `scenario flying_grant_activated_batch::flying_grant_activated_batch_cards_allow_a_ground_blocker_only_before_activation`
- `scenario flying_grant_activated_batch::flying_grant_activated_batch_cards_reject_activation_without_the_exact_cost`
- `mechanic_registry::flying_grant_activated_batch::flying_grant_activated_batch_maps_definitions`
- `mechanic_registry::flying_grant_activated_batch::flying_grant_activated_batch_fingerprints_match_the_pinned_oracle_text`

## Focused evidence

- Intended canary red: `build/verification-logs/20261116-160521-016-Rust-scenario-red-flying_grant_activated-canary.log`
  (MissingCard cobalt_golem, exit 101).
- Focused canary green: `build/verification-logs/20261116-160606-342-Rust-scenario-green-flying_grant_activated-canary.log`, exit 0.
- Focused all-card green: `build/verification-logs/20261116-160800-825-Rust-scenario-green-flying_grant_activated-all.log`, exit 0.
- Registry green: `build/verification-logs/20261116-161014-197-Rust-registry-green-flying_grant_activated.log`, exit 0.
- Review maps: `scripts/check-card-evidence.ps1 -Preparation` PASS (1884 maps).
- Final focused green after review corrections: `build/verification-logs/20261116-165114-295-Rust-scenario-final-flying_grant_activated.log`
  and `build/verification-logs/20261116-165130-007-Rust-registry-final-flying_grant_activated.log`, exit 0.
- Metadata refresh: only the 15 expected batch C fingerprint rows and their `CARDS.md` checkboxes changed.

## Review and gate

Frozen in `build/campaign/review/queue-v24-batch-c-v3` (patch SHA-256
`becad7ab49cde35d7497f6db7b0cdf8368e517b44ffb85e99949fee95485d534`). Independent read-only review approved the family for reviewed semantics; material
findings (real power-3 target rejection, a non-targeting-spell case, an observable Sorin's Thirst
fizzle, resolution-scope evidence, CR 603.3/608.1 and 509.1b/509.1h citations) were corrected and
delta-approved. Batch queue-v24 C full gate `verify-6c12deb5-91a1-4aa0-9668-1777fc8916b2` passed exit 0,
covering 07-becomes-target-sacrifice..09-unblockable-power-two; commit `5a9717003252d7cb9f489a08036c74535c4be472`;
`origin/card-campaign` verified at the same SHA.
