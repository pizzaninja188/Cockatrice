# 02-two-mana-regenerate — three "{1}{colored}: Regenerate this creature" bodies

- Queue: `build/campaign/queue-v91`.
- Cards: 3.
- Band: multi-clause composition (3).
- Status: **verified**. Independent read-only review APPROVED (bundle rev 4, manifest
  `cb2fc8c79ae4d9adc97dc7d63df9f6e10fa97af77fd75d62a5080380b944b1ec`); batch gate
  `verify-647f2fb9-0ebe-4355-9459-6653327b653a` exit 0 covering all three queue-v91 families.
- Pinned bulk: SHA-256 `1cee09ee1ac70977c46171f4abf5cdfe170ab43bc32232af9138404e27524ce6`.
- Pinned rulings: SHA-256 `84f4c793e6d4f1256206b915da007111e1a445fe7cad99147802086454e8a95d`.

## Typed shape and analogue

One printed keyword/characteristic-defining line plus exactly one
`{1}{colored}: Regenerate this creature.` line. This is the same shipped regeneration payload as
family 01 but a **different activation-cost kind**: a generic component plus one colored symbol,
not a single colored symbol. The two cohorts are deliberately kept apart rather than merged on a
normalized `{M}{M}` signature, because the printed cost shapes differ.

| Card | Oracle ID | Card cost | Activation | Static line | Body |
|---|---|---|---|---|---|
| Carrion Wall | `ce8750d4-1b74-4b02-afd8-6b639c1a9a78` | `{1}{B}{B}` | `{1}{B}` | Defender | 3/2 |
| Wolfir Avenger | `83deda94-66e8-4655-9cd5-6d97d809946d` | `{1}{G}{G}` | `{1}{G}` | Flash | 3/3 |
| Skeletal Changeling | `b9d388ca-1a63-4a11-bf33-9f7b87305a52` | `{1}{B}` | `{1}{B}` | Changeling | 1/1 |

Skeletal Changeling's Changeling is a **characteristic-defining ability**, not a keyword
(CR 702.73a), so it is `characteristic_defining_abilities: [..Changeling]` exactly as Chitinous
Graspling, Firdoch Core and Changeling Wayfinder ship it. Its body (1/1) and card cost (`{1}{B}`)
happen to match the activation cost, which is the copy risk: a copy-pasted cost would still pass a
"the pool can pay it" assertion, so the scenario asserts the *exact* pool is consumed and a pool
without the generic is refused.

Wolfir Avenger's Flash is a printed static keyword on a creature (`keywords: [Flash]`), the same
shape the shipped Dueling Rapier and Ambush Viper carry.

## Exact Oracle text (pinned bulk)

- Carrion Wall — `Defender` / `{1}{B}: Regenerate this creature.`
- Wolfir Avenger — `Flash` / `{1}{G}: Regenerate this creature.`
- Skeletal Changeling — `Changeling` / `{1}{B}: Regenerate this creature.`

All three are Commander-legal, normal layout, single-face, uniquely named in both pins, present in
`build/campaign/source/cards-validation.xml`, absent from the live registry and review maps,
unchecked in `CARDS.md` at admission (the refresh then flips all three to `[x]`), and absent from
`blocked.tsv` and `queue-exclusions.txt`. See `build/campaign/queue-v91/revalidation.tsv`.

## Pinned rulings

None of the three has a pinned ruling. The regeneration, the exact-cost refusal and the
characteristic-defining Changeling behaviour are shipped evidence on `q42_regenerate_self_batch` and
`issue_154_changeling`.

## Copy-from and differences

Copy `gorilla_chieftain` for the `{1}{G}` regeneration ability, `lim-duls_high_guard` for the
keyword-plus-regenerate two-line shape, and `chitinous_graspling` for the Changeling CDA. Differences:
three distinct static lines (defender, flash, changeling) over three bodies; the activation is
`{1}{B}` or `{1}{G}`, never a single symbol.

## Rules evidence

CR 701.19a-b (regeneration shield), CR 602.1/602.2 (activation and printed cost), CR 702.3
(defender), CR 702.8a (flash), CR 702.73a (changeling is every creature type in every zone, as a
characteristic-defining ability) and CR 514.2 (cleanup expiry).

## Semantic evidence

- `scenario q91_batch_a::q91_two_mana_regenerate_canary_pays_generic_plus_color`
- `scenario q91_batch_a::q91_two_mana_regenerate_canary_hardest_keeps_changeling`
- `scenario q91_batch_a::q91_two_mana_regenerate_creatures_consume_the_shield_and_keep_their_keyword`
- `scenario q91_batch_a::q91_two_mana_regenerate_refuses_a_pool_without_the_generic`
- `mechanic_registry::q91_batch_a::q91_two_mana_regenerate_maps_definitions`
- `mechanic_registry::q91_batch_a::q91_skeletal_changeling_authors_changeling_as_a_cda`
- `mechanic_registry::q91_batch_a::q91_batch_fingerprints_match_the_pinned_oracle_text`

## Canaries

- Simplest: Carrion Wall (`{1}{B}` on a `{1}{B}{B}` 3/2 Defender).
- Hardest: Skeletal Changeling (card cost and activation cost are the same string, so only the
  exact-pool-and-refusal pair distinguishes them; plus the characteristic-defining Changeling).

## Focused evidence

- Focused scenario green (the whole `q91_batch_a` module, 18 tests; 4 of them are this family):
  `build/campaign/queue-v91/logs/green-q91-scenario.log`.
- Focused registry green (6 tests): `build/campaign/queue-v91/logs/green-q91-registry.log`.
- Non-vacuity probe: `build/campaign/queue-v91/logs/probe-fingerprint.log` — a mutated fingerprint
  fails.

## Delivery

- Batch gate `verify-647f2fb9-0ebe-4355-9459-6653327b653a` Pass, exit 0 (batch queue-v91 A, covering 01-one-mana-regenerate,
  02-two-mana-regenerate and 03-basic-land-pair-search, 10 cards).
- Product commit `6a6cebd59`; remote `6a6cebd59496d7307344f22f2a25297e96d86070` on
  `origin/card-campaign`, verified by `git fetch` + `git rev-parse`.
