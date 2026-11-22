# Family: 06-naturalize-lifegain (6 cards)

## Selection

Multi-clause composition, 6 cards. Revalidated by Oracle ID and printed name against the pinned
bulk, live registry, maps, `CARDS.md`, `queue-exclusions.txt` and `blocked.tsv`; see
`build/campaign/queue-v23/revalidation.tsv`.

## Exact cards

| Oracle ID | Card | Mana | Type line | Life |
|---|---|---|---|---|
| 05b5c0fb-84b2-4a32-bf66-179c4dccb3df | Natural End | {2}{G} | Instant | 3 |
| 14945b80-10ec-4205-98df-cc3e119ca57c | Springsage Ritual | {3}{G} | Instant | 4 |
| f041b37e-25af-4aff-b7e3-f07a4d5c6f9a | Appetite for the Unnatural | {2}{G} | Instant | 2 |
| 87bd0fdb-c5b9-46ea-9858-1a870960351f | Invoke the Divine | {2}{W} | Instant | 4 |
| c804ac8e-7427-43c6-b496-3ee6de192f03 | Sanctify | {1}{W} | Sorcery | 3 |
| aff51c27-fb61-41b3-a7ea-7092cd280c63 | Solemn Offering | {2}{W} | Sorcery | 4 |

## Copy-from and differences

`thorins_last_stand` mode 2 for the exact `Destroy(... permanent_types: [Artifact, Enchantment])`
plus `GainLife` sequence; `disenchant` for the targeting shape.

The life amount is 2, 3 or 4 and the colors differ; Sanctify and Solemn Offering are sorceries, the
other four are instants.

## Rules evidence

CR 115.1 (target), 701.8 (destroy), 119.3 (life gain) and 608.2b (an illegal target makes the whole
spell not resolve, so no life is gained). The Natural End, Springsage Ritual, Invoke the Divine and
Solemn Offering rulings state the same fizzle rule; indestructible targets are still legal and still
gain life.

## Canaries

- Simplest: Appetite for the Unnatural ({2}{G}, gain 2).
- Hardest: Solemn Offering ({2}{W} sorcery, gain 4).
- Every card is executed by the all-card artifact/enchantment destroy, exact-life, indestructible,
  stale-fizzle and creature-rejection tests.

## Semantic evidence

- `scenario naturalize_lifegain_batch::naturalize_lifegain_batch_canary_cards_destroy_an_artifact_and_gain_the_printed_life`
- `scenario naturalize_lifegain_batch::naturalize_lifegain_batch_cards_destroy_an_enchantment_and_gain_exact_life`
- `scenario naturalize_lifegain_batch::naturalize_lifegain_batch_cards_gain_life_when_an_indestructible_target_survives`
- `scenario naturalize_lifegain_batch::naturalize_lifegain_batch_cards_fizzle_a_stale_target_without_gaining_life`
- `scenario naturalize_lifegain_batch::naturalize_lifegain_batch_cards_reject_a_creature_target_without_paying`
- `mechanic_registry::naturalize_lifegain_batch::naturalize_lifegain_batch_maps_definitions`
- `mechanic_registry::naturalize_lifegain_batch::naturalize_lifegain_batch_fingerprints_match_the_pinned_oracle_text`

The six maps cover the single Oracle line, both effects and the target group. The fizzle test
removes the artifact before resolution and asserts no life gain; the indestructible test proves the
ruling that life is still gained when the target survives.

## Focused evidence

- Intended canary red: `build/campaign/queue-v23/tmp/redB-naturalize_lifegain_batch.log` (missing appetite_for_the_unnatural, exit 101).
- Focused canary green: `build/campaign/queue-v23/tmp/greenB-naturalize_lifegain_batch.log`, exit 0.
- Focused registry green: `build/campaign/queue-v23/tmp/regBdetail-naturalize_lifegain_batch.log`, exit 0 (3 tests).
- Focused scenario green: `build/campaign/queue-v23/tmp/scnBdetail-naturalize_lifegain_batch.log`, exit 0 (5 tests).
- Card data check: `build/campaign/queue-v23/tmp/carddata-b.log`, exit 0.
- Review maps: `scripts/check-card-evidence.ps1 -Preparation` PASS (1796 maps) at
  `build/campaign/queue-v23/tmp/evidence-b2.log`.

## Review and gate

Frozen in `build/campaign/review/queue-v23-batch-b-v1` (patch SHA-256
`f34ebb8ebc1852ea93619ca2a24b2ca40f70e2682653f095790aa578d99e43ef`). Independent read-only review
approved the family (with delta approval after map fixture corrections). Batch queue-v23 batch B
full gate `verify-4d575129-898b-4fb8-9819-5bffb0cab0d8` passed exit 0, covering 04-haste-self-pump..06-naturalize-lifegain; commit
`3b4f0f8cecaa53e66008c35eaf97d804c5dc66bc`; remote `origin/card-campaign` verified at the same SHA.
