# 03-basic-land-pair-search — three "up to two basic lands onto the battlefield tapped" sorceries

- Queue: `build/campaign/queue-v91`.
- Cards: 3.
- Band: multi-clause composition (3).
- Status: **verified**. Independent read-only review APPROVED (bundle rev 4, manifest
  `cb2fc8c79ae4d9adc97dc7d63df9f6e10fa97af77fd75d62a5080380b944b1ec`); batch gate
  `verify-647f2fb9-0ebe-4355-9459-6653327b653a` exit 0 covering all three queue-v91 families.
- Pinned bulk: SHA-256 `1cee09ee1ac70977c46171f4abf5cdfe170ab43bc32232af9138404e27524ce6`.
- Pinned rulings: SHA-256 `84f4c793e6d4f1256206b915da007111e1a445fe7cad99147802086454e8a95d`.

## Typed shape and analogue

All three print `Search your library for up to two basic land cards, put them onto the battlefield
tapped, then shuffle.` and differ only in what else the card prints. The payload is the shipped
direct `SearchLibrary(count: 2, filter: Some((card_type: Some(BasicLand))), destination:
Battlefield(tapped: true))` shape that **Circuitous Route** ships without bounds; the engine derives
`min: 0` for any filtered search, which is the printed "up to two" (CR 701.23b). The family's own scenarios assert `(min, max) == (0, 2)`, a legal zero-card selection
with basics still in the library, and exactly one public shuffle in both the select-two and
select-zero paths.

| Card | Oracle ID | Card cost | Rider | RON |
|---|---|---|---|---|
| Explosive Vegetation | `a0abd957-01a0-4aa9-8fc8-0e6840d21606` | `{3}{G}` | none | `data/explosive_vegetation.ron` |
| Migration Path | `db4907f3-4071-47c4-9329-96b4813435be` | `{3}{G}` | Cycling `{2}` | `data/migration_path.ron` |
| Nissa's Expedition | `653010bc-45af-427e-a294-e0cc28d31821` | `{4}{G}` | Convoke | `data/nissas_expedition.ron` |

Migration Path's cycling is the shipped hand-zone ability (`source_zone: Hand`,
`costs: [Mana("{2}"), DiscardSelf]`, `effect: [Draw(count: 1)]`) that Barren Moor and Censor ship; the
scenario activates it with an explicit `AbilitySourceZone::Hand` and a generation receipt, which is
what the engine requires for a non-battlefield source. Nissa's Expedition's Convoke is the shipped
`keywords: [Convoke]` spell keyword (Appeal to Eirdu, Arboretum Elemental).

These three were selected **instead of** the `Explosive Vegetation` / `Kodama's Reach` / `Cultivate`
pair-up because Kodama's Reach and Cultivate are already registered and print
"reveal those cards, put one onto the battlefield tapped and the other into your hand" — a different
destination contract (`BattlefieldTappedThenHand`). Grouping by the typed shape keeps
`Battlefield(tapped: true)` apart from `BattlefieldTappedThenHand`.

## Exact Oracle text (pinned bulk)

- Explosive Vegetation — `Search your library for up to two basic land cards, put them onto the
  battlefield tapped, then shuffle.`
- Migration Path — the same search line plus `Cycling {2} ({2}, Discard this card: Draw a card.)`.
- Nissa's Expedition — `Convoke (...)` plus the same search line.

All three are Commander-legal, normal layout, single-face, uniquely named in both pins, present in
`build/campaign/source/cards-validation.xml`, absent from the live registry and review maps,
unchecked in `CARDS.md` at admission (the refresh then flips all three to `[x]`), and absent from
`blocked.tsv` and `queue-exclusions.txt`. See `build/campaign/queue-v91/revalidation.tsv`.

## Pinned rulings

- Nissa's Expedition (2024-01-12, six rulings): Convoke applies after the total cost is computed, it
  does not change the mana cost or mana value, and it may tap a creature not controlled since the
  turn began. The scenario asserts the printed mana cost is `{4}{G}` and that no Convoke payment is
  forced.
- Explosive Vegetation and Migration Path have no pinned ruling.

## Copy-from and differences

Copy `circuitous_route` for the direct `count: 2 / Battlefield(tapped: true)` search (it is the same
shape with a wider filter) and `new_generations_technique` for the same BasicLand filter (it differs
only in its printed `reveal: true` and its Sneak cost); copy
`barren_moor` / `censor` for the cycling ability and `appeal_to_eirdu` for
`keywords: [Convoke]`. Differences: the filter is exactly `BasicLand` (Circuitous Route admits Gates
too), and two of the three add a rider line that the search does not read.

## Rules evidence

CR 701.23a-b (a search with a stated quality may find zero, one or two of them), CR 701.24a (shuffle
after the search), CR 614.1d/614.12 (the found lands enter tapped), CR 702.29a (cycling is a hand-zone
activated ability that discards the card as a cost) and CR 702.51a-c (convoke).

## Semantic evidence

- `scenario q91_batch_a::q91_basic_land_search_canary_finds_two_tapped_lands_and_shuffles`
- `scenario q91_batch_a::q91_basic_land_search_canary_hardest_finds_zero`
- `scenario q91_batch_a::q91_basic_land_search_cards_apply_their_printed_cost_and_keep_their_keywords`
- `scenario q91_batch_a::q91_basic_land_search_cards_have_only_the_printed_surfaces`
- `scenario q91_batch_a::q91_migration_path_cycles_from_hand_for_a_card`
- `mechanic_registry::q91_batch_a::q91_basic_land_search_maps_definitions`
- `mechanic_registry::q91_batch_a::q91_batch_fingerprints_match_the_pinned_oracle_text`

## Canaries

- Simplest: Explosive Vegetation (`{3}{G}`, no rider).
- Hardest: Nissa's Expedition (`{4}{G}` plus Convoke) — `q91_basic_land_search_canary_hardest_finds_zero`
  is its canary. Three findable basics are in the library, selecting none is accepted, and the
  shuffle still fires.

## Focused evidence

- Focused scenario green (the whole `q91_batch_a` module, 18 tests; 5 of them are this family):
  `build/campaign/queue-v91/logs/green-q91-scenario.log`.
- Focused registry green (6 tests): `build/campaign/queue-v91/logs/green-q91-registry.log`.
- Non-vacuity probes, each restored and re-verified green:
  - `build/campaign/queue-v91/logs/probe-fingerprint.log` — a mutated fingerprint fails.
  - `build/campaign/queue-v91/logs/probe-shuffle.log` — adding `shuffle: false` to Explosive
    Vegetation fails the canary, so the "then shuffle" clause is genuinely asserted.

## Delivery

- Batch gate `verify-647f2fb9-0ebe-4355-9459-6653327b653a` Pass, exit 0 (batch queue-v91 A, covering 01-one-mana-regenerate,
  02-two-mana-regenerate and 03-basic-land-pair-search, 10 cards).
- Product commit `6a6cebd59`; remote `6a6cebd59496d7307344f22f2a25297e96d86070` on
  `origin/card-campaign`, verified by `git fetch` + `git rev-parse`.
- No MTG rules surface area beyond the per-card CR citations above; no engine, protobuf, relay
  or client contract changed, so those surfaces are N/A.

