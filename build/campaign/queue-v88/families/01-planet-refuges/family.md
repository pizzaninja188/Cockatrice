# 01-planet-refuges — ten Planet dual lands with the refuge entry package

- Queue: `build/campaign/queue-v88`.
- Cards: 10.
- Band: parameter family (the full five-color allied/enemy pair cycle).
- Status: **verified**; independent review approved and the queue-v88 batch gate passed
  exit 0.
- Pinned bulk: SHA-256 `1cee09ee1ac70977c46171f4abf5cdfe170ab43bc32232af9138404e27524ce6`
  (`oracle-cards.jsonl.gz`, the 2026-11-26 sync the presentation registry was regenerated from).
- Pinned rulings: SHA-256 `84f4c793e6d4f1256206b915da007111e1a445fe7cad99147802086454e8a95d`.

## Typed shape and analogue

Each card is `This land enters tapped. / When this land enters, you gain 1 life. /
{T}: Add {C1} or {C2}.` — the reviewed refuge package already shipped on Akoum Refuge,
Graypelt Refuge, Jwar Isle Refuge, Kazandu Refuge and Sejiri Refuge (`refuge_lands_batch`):
`StaticAbilityDef::EntersTapped { affected: Self_ }`,
`TriggerCondition::WhenSelfEntersBattlefield → GainLife(1, Controller)` and one tap mana
ability with `ProduceMana(options: [C1, C2])`.

The only difference from the analogue is the type line: these print `Land — Planet` instead of
plain `Land`. The pinned 2026-10-07 ruling on every one of the ten states "Planet is a land
subtype with no special meaning. It doesn't grant the land any intrinsic abilities." The engine
already knows `Planet` as a land subtype (`LAND_SUBTYPES` in
`tricerules-core/src/engine/characteristics.rs` and the shared-land-type search path in
`zones.rs`), and `Kavaron, Memorial World` is a shipped `["Land", "Planet"]` card, so the type
line needs no new support. All ten print no other text, so the type line is the whole semantic
difference and the family proves the subtype does not perturb the refuge package.

The cycle covers all ten two-color pairs exactly once, so the printed color pair is a real
parameter rather than an incidental: Andoria {U}{R}, Bajor {R}{W}, Cardassia Prime {W}{B},
Earth Sector 001 {G}{U}, Ferenginar {B}{G}, Orion {R}{G}, Qo'noS {B}{R}, Risa {G}{W},
Romulus {U}{B}, Vulcan {W}{U}. That set is the complete ten-pair matrix, so a transposed or
duplicated pair fails by construction.

## Cards

| Card | Oracle ID | Card id | Type | Colors | RON | Map |
|---|---|---|---|---|---|---|
| Andoria | `79134f5e-94db-4a40-ba75-2569b839cf35` | `andoria` | Land — Planet | {U} or {R} | `data/andoria.ron` | `review-maps/andoria.json` |
| Bajor | `93d7f6e7-b6b0-48f3-891a-73970826af3b` | `bajor` | Land — Planet | {R} or {W} | `data/bajor.ron` | `review-maps/bajor.json` |
| Cardassia Prime | `aff720b4-b8ef-46ea-9a3d-e101999e6526` | `cardassia_prime` | Land — Planet | {W} or {B} | `data/cardassia_prime.ron` | `review-maps/cardassia_prime.json` |
| Earth, Sector 001 | `c75d87b0-7de9-4c50-b781-9749d67c1e07` | `earth,_sector_001` | Land — Planet | {G} or {U} | `data/earth,_sector_001.ron` | `review-maps/earth,_sector_001.json` |
| Ferenginar | `2e0753ad-7f76-4d26-8e87-6f201378f333` | `ferenginar` | Land — Planet | {B} or {G} | `data/ferenginar.ron` | `review-maps/ferenginar.json` |
| Orion | `47e3aa22-8806-408f-b93e-0d0b800866f6` | `orion` | Land — Planet | {R} or {G} | `data/orion.ron` | `review-maps/orion.json` |
| Qo'noS | `cab718af-8e6f-4766-856e-3a61ba98f94d` | `qonos` | Land — Planet | {B} or {R} | `data/qonos.ron` | `review-maps/qonos.json` |
| Risa | `365a4644-74f4-4af9-81ab-0d328ca5edce` | `risa` | Land — Planet | {G} or {W} | `data/risa.ron` | `review-maps/risa.json` |
| Romulus | `14c4ad24-7f38-409b-8ef0-341f233982da` | `romulus` | Land — Planet | {U} or {B} | `data/romulus.ron` | `review-maps/romulus.json` |
| Vulcan | `c7b6bc1f-9161-415e-a097-d9c8cd81d4ae` | `vulcan` | Land — Planet | {W} or {U} | `data/vulcan.ron` | `review-maps/vulcan.json` |

Note the one identity subtlety: `slugify` preserves the printed comma, so the card id is
`earth,_sector_001` (the same shape as the shipped `kavaron,_memorial_world`), while the face id
must remain canonical snake_case `earth_sector_001`. The presentation row is keyed by
`(card id, face id)`, which is why the fingerprint test resolves presentation metadata through the
face id rather than reusing the card id.

## Pinned rulings

Every card carries exactly one ruling, the 2026-10-07 text quoted above. Packets are under
`build/campaign/queue-v88/families/01-planet-refuges/packets/`.

## Canaries

- Simplest: Andoria — a single enemy pair, proving entry, trigger and one color of production
  end to end.
- Second canary: Vulcan — the {W}{U} pair, which is exactly the Sejiri Refuge color pair from the
  analogue, so a stored `Planet` subtype that perturbed mana production or the tap restriction
  fails against known-good behavior.

## Semantic evidence

- `scenario q88_batch_a::q88_planet_refuge_canary_simplest_enters_tapped_gains_life_and_taps`
- `scenario q88_batch_a::q88_planet_refuge_canary_wu_pair_offers_both_printed_colors_and_rejects_a_second_activation`
- `scenario q88_batch_a::q88_planet_refuges_apply_their_printed_colors_and_entry_replacement`
- `scenario q88_batch_a::q88_planet_refuges_do_not_gain_life_when_they_do_not_enter`
- `mechanic_registry::q88_batch_a::q88_planet_refuges_map_definitions`
- `mechanic_registry::q88_batch_a::q88_batch_a_fingerprints_match_the_pinned_oracle_text`

## Focused evidence

- Intended red: `build/campaign/queue-v88/logs/red-q88.log` — 4 failed, 0 passed with the sixteen
  RONs held out of `data/`. That log was produced before the identity and timing assertions were
  added, so the frozen files were re-run afterwards; see the green entries below.
- Registry green: `build/campaign/queue-v88/logs/green-q88-registry.log` (4 passed, 0 failed).
- Scenario green: `build/campaign/queue-v88/logs/green-q88-scenario.log` (12 passed, 0 failed).
- `cargo fmt --all -- --check` exit 0; `cargo clippy -D warnings` exit 0 for both packages;
  `git diff --check` exit 0 (`build/campaign/queue-v88/logs/preflight-q88.log`).
- Non-vacuity probes: replacing Andoria's `{U}` production with `{R}` fails both the registry and
  the scenario color loop; adding a `Tap` cost to Jungle Delver fails the registry cost list;
  changing Brass Secretary's draw to two fails the per-card draw loop.

## Conformance and scope

Pure data plus tests, maps and metadata. No engine, protobuf, relay, client, generator or
presentation-contract change. Affected side: Rust and CardData.

## Final gate

PASS, exit 0. Queue-v88 consolidated batch gate `verify-1175655f-71da-4886-a7bb-f74e3eadc5af`
on the reviewed tree, covering all three families (16 cards): environment, diff check, CardData,
the card-data dependency boundary, all five package format checks, Clippy and the full Rust suite
(including all 16 q88 tests).
Summary: `build/verification-logs/verify-1175655f-71da-4886-a7bb-f74e3eadc5af/summary.json`.

Commit `f7880fa10`; remote `f7880fa10d41238f4922e5e753668a8712124a3a` verified by
`git fetch` and `git rev-parse`.

## Blockers and gaps

No card in this family is blocked. No partial admission.

## MTG applicability

CR 205.3i/305.5 (Planet is a land type; the pinned 2026-10-07 ruling states it grants no
intrinsic ability), CR 614.1d/614.12 (the enters-tapped replacement checks the live state
before the land enters), CR 603.6a/603.6d (the entry trigger), CR 119.3 (gaining one life) and
CR 605.1a/605.3b (the tap mana ability resolves without using the stack). No Comprehensive Rules
deviation.
