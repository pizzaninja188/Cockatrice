# 01-planeswalker-untap-lands — the ten "unless you control a planeswalker" duals

- Queue: `build/campaign/queue-v90`.
- Cards: 10.
- Band: parameter family (10).
- Status: **verified**; independent read-only review approved the reviewed semantics (APPROVED at
  bundle revision 10) and the queue-v90 consolidated batch gate passed exit 0.
- Pinned bulk: SHA-256 `1cee09ee1ac70977c46171f4abf5cdfe170ab43bc32232af9138404e27524ce6`.
- Pinned rulings: SHA-256 `84f4c793e6d4f1256206b915da007111e1a445fe7cad99147802086454e8a95d`.

## Typed shape and analogue

Each card prints `This land enters tapped unless you control a planeswalker.` on line 1 and
`{T}: Add {X} or {Y}.` on line 2. That is one static
`EntersTapped(affected: Self_, condition: Some(GameCondition::BattlefieldAggregate { filter:
(controllers: Controller, matching: (kind: AnyPermanent, permanent_types: [Planeswalker])),
aggregate: Count, max: Some(0) }))` plus one tap `ProduceMana(options: [two entries])`.

`max: Some(0)` is the encoded form of "unless you control a planeswalker": the condition holds only
at a zero count, so the replacement applies then and the land enters tapped; at one or more
planeswalkers the condition fails and the land enters untapped. This mirrors the shipped
`Spirebluff Canal`
(`min: Some(3)`, three or more *lands*) and `Stormcarved Coast` (`max: Some(1)`, at most one
*land*) which use the same `BattlefieldAggregate` leaf.

This is **not** the blocked `#519` "enters tapped unless you have two or more opponents"
condition. That one reads a live *player* count through `PlayerCount`; this one reads a
battlefield aggregate of *permanents*. They are different typed leaves, so this family adds no
support for the blocked condition and does not retry it.

`PermanentTypeFilter::Planeswalker` is an existing variant (`tricerules-card-model/src/primitives/
abilities.rs`) with 49 root-file data consumers (56 including `data/generated/`), e.g.
`Ichormoon Gauntlet`, `Deadly Derision` and `Bitter Triumph`. (`Chandra's Embercat` uses the *different* `CardTypeFilter::Planeswalker` leaf.)

## Cards

| Card | Oracle ID | Mana pair | Activation line | RON | Map |
|---|---|---|---|---|---|
| Theorix Annex | `00ad5200-5179-4d1b-8bb3-a09e1b31be58` | `{U}` / `{B}` | 1 | `data/theorix_annex.ron` | `review-maps/theorix_annex.json` |
| Stingerquill Annex | `26e20ae5-4059-4d63-9db5-6e12421d1aba` | `{B}` / `{R}` | 1 | `data/stingerquill_annex.ron` | `review-maps/stingerquill_annex.json` |
| Konstrari Annex | `31ea4f3f-392a-4319-a95b-eb28790a80e6` | `{R}` / `{G}` | 1 | `data/konstrari_annex.ron` | `review-maps/konstrari_annex.json` |
| Formidable Commons | `3e51c060-1e86-4a0e-8fee-8b7273161468` | `{B}` / `{G}` | 1 | `data/formidable_commons.ron` | `review-maps/formidable_commons.json` |
| Innovative Commons | `6812e4d3-0034-4fc8-9ee9-520f76a0ea94` | `{U}` / `{R}` | 1 | `data/innovative_commons.ron` | `review-maps/innovative_commons.json` |
| Meticulous Commons | `7fdd471f-ec95-4aa7-8f8a-ad8946de5ab4` | `{W}` / `{B}` | 1 | `data/meticulous_commons.ron` | `review-maps/meticulous_commons.json` |
| Transformative Commons | `8ec56e2a-380f-4de7-9fb7-52de803008fc` | `{G}` / `{U}` | 1 | `data/transformative_commons.ron` | `review-maps/transformative_commons.json` |
| Fatehold Annex | `d35876a3-e891-43d3-a50c-69e8469497e0` | `{W}` / `{U}` | 1 | `data/fatehold_annex.ron` | `review-maps/fatehold_annex.json` |
| Dedicated Commons | `d4e0749a-e179-4c19-8536-bf8f1a198549` | `{R}` / `{W}` | 1 | `data/dedicated_commons.ron` | `review-maps/dedicated_commons.json` |
| Vigorbloom Annex | `fcb35d42-bcd2-427c-9640-ff3602b79912` | `{G}` / `{W}` | 1 | `data/vigorbloom_annex.ron` | `review-maps/vigorbloom_annex.json` |

The printed pair order is the mana option order. Meticulous Commons is `{W}`/`{B}` while its
neighbour Fatehold Annex is `{W}`/`{U}`, so a copy-pasted option list is visible.

## Pinned rulings

All ten carry the same single 2026-08-21 ruling:

> If this land enters at the same time as a planeswalker, that planeswalker isn't considered when
> determining if this land enters the battlefield tapped or untapped.

That is the CR 614.1d/614.12 "pre-entry inspection" behaviour. The shipped replacement-layer
scenarios `conditional_slow_land_ignores_lands_entering_in_the_same_batch` and
`conditional_fast_land_ignores_lands_entering_in_the_same_batch`
(`tricerules-core/src/engine/replacement/entry_layers_tests.rs`) drive the same
`GameCondition::BattlefieldAggregate` leaf through the internal simultaneous-entry seam and assert
that same-batch entrants are not counted. Their filters and bounds differ (`[Land]` with
`min: Some(3)` / `max: Some(1)`); what they prove is the leaf and the exclusion mechanism, which is
what this ruling is about.
That seam is crate-private, so the family's own scenario asserts the *live-at-entry* half (a
planeswalker already on the battlefield is seen; one that arrives afterwards does not retroactively
untap) and documents only the *same-batch* half as shipped evidence, rather than claiming it.

The family scenario adds the planeswalker-specific cases the shipped land scenarios cannot cover:
tapped with no planeswalker, untapped with one, untapped only for the *controller's* planeswalker,
and no retroactive untap when the planeswalker arrives later.

## Copy-from and differences

Copy `theorix_annex` (written first, reviewed against `spirebluff_canal` / `stormcarved_coast`) for
the definition; only the printed colour pair and the card identity differ card to card.

Differences from the shipped analogues: those count `Land` and gate on three or more / at most one;
this cycle counts `Planeswalker` and gates on exactly zero. Spirebluff Canal and Stormcarved Coast
carry `exclude_source: true`; this cycle omits it, defaulting to `false`, because a Land can never
satisfy a Planeswalker predicate — the registry test states and asserts exactly that. The mana
ability here is an ordinary `ProduceMana` with an explicit option list (the cycle has no basic land
type), not an `intrinsic_land_mana` ability.

## Rules evidence

CR 305.6 (a land with a printed basic land type has the intrinsic mana ability — these have no
basic land type, so their ability is an ordinary activated mana ability), CR 614.1d/614.12 (the
entry replacement inspects the battlefield before the entrant arrives), CR 602.2 (activate),
CR 605.1a (mana ability), CR 701.26 (tap), CR 302.6 (summoning sickness).

## Semantic evidence

- `scenario q90_batch_a::q90_planeswalker_land_canary_checks_the_controllers_planeswalker`
- `scenario q90_batch_a::q90_planeswalker_land_ignores_an_opponents_planeswalker`
- `scenario q90_batch_a::q90_planeswalker_land_checks_its_condition_only_at_entry`
- `scenario q90_batch_a::q90_planeswalker_lands_apply_their_printed_colors`
- `scenario q90_batch_a::q90_planeswalker_lands_refuse_a_second_activation_while_tapped`
- `mechanic_registry::q90_batch_a::q90_planeswalker_untap_lands_map_definitions`
- `mechanic_registry::q90_batch_a::q90_planeswalker_untap_lands_fingerprints_match_the_pinned_oracle_text`

## Canaries

- Simplest: Theorix Annex (no planeswalker → tapped; one → untapped; opponent's → still tapped).
- Hardest: Meticulous Commons (`{W}`/`{B}` at activation, the pair whose neighbour Fatehold Annex is
  `{W}`/`{U}`, so a copy-pasted option list is visible) plus Innovative Commons (`{U}`/`{R}`, whose
  two symbols are two apart on the WUBRG wheel). Every pair is also driven through the shared colour
  loop.

## Focused evidence

- Intended red (all seventeen RONs held out): `build/campaign/queue-v90/logs/red-q90-registry.log`
  (3 failed / 2 passed) and `red-q90-scenario.log` (16 failed / 0 passed).
- Focused green: `build/campaign/queue-v90/logs/green-q90-registry.log` (5 passed) and
  `green-q90-scenario.log` (16 passed) — the counts include all three families.
- Non-vacuity probe: `build/campaign/queue-v90/logs/probe-land-pair.log` mutates Meticulous
  Commons' second mana option from black to blue; the registry test fails 4 passed / 1 failed.
  Restored and hash-verified afterwards.
- Format: `cargo fmt --all -- --check` exit 0.
- Clippy: `cargo clippy --all-targets -- -D warnings` exit 0.
- `git diff --check` exit 0.
- Card evidence preparation: `check-card-evidence.ps1 -Preparation` PASS over 3566 maps.

## Known limits recorded rather than claimed

`q90_planeswalker_land_checks_its_condition_only_at_entry` cannot observe the *same-batch* half of
the ruling (a planeswalker entering simultaneously). That half is covered by the shipped
entry-layer scenarios named above, not by a new assertion, and this record says so instead of
claiming otherwise. The live-at-entry half — a planeswalker already present is seen — *is* asserted,
by the canary (`q90_planeswalker_land_canary_checks_the_controllers_planeswalker`).

## Batch gate

`build/verification-logs/verify-8c1fef97-ec11-4987-9219-5a25ff4870a8` — Status Pass, exit 0,
covering `01-planeswalker-untap-lands`..`03-keyword-tap-target` (17 cards). Environment, diff
check, CardData, dependency boundary, five package format checks, Clippy, and the full Rust
suite; all 21 q90 tests (5 registry, 16 scenario) were observed passing in that run.
Product commit `f01e5bd20`.
