# 02-sac-draw-creatures — sacrifice the source for one card

- Queue: `build/campaign/queue-v88`.
- Cards: 3.
- Band: multi-clause composition (3).
- Status: **verified**; independent review approved and the queue-v88 batch gate passed
  exit 0.
- Pinned bulk: SHA-256 `1cee09ee1ac70977c46171f4abf5cdfe170ab43bc32232af9138404e27524ce6`.
- Pinned rulings: SHA-256 `84f4c793e6d4f1256206b915da007111e1a445fe7cad99147802086454e8a95d`.

## Typed shape and analogue

Each card prints `{2}, Sacrifice this creature: Draw a card.` — one activated ability with
`costs: [Mana("{2}"), SacrificeSelf]` and `effect: [Draw(count: 1)]`, shipped on Illvoi Galeblade
(`data/generated/i/illvoi_galeblade.ron`). The family varies only
the printed body and one optional keyword, so the shared cost/effect pair is proved rather than a
hard-coded single card.

Two of the three are artifacts and one is a Clue. `Parcel Myr`'s Clue subtype is a real rules
subtype that other "sacrifice a Clue" costs can use, so it is authored as a normal type-line
entry rather than dropped. The pinned 2021-06-18 rulings say the subtype does not change this
card's own sacrifice cost and that the permanent is not a created token, so no subtype-specific
branch is needed.

## Cards

| Card | Oracle ID | Cost | Type | P/T | Extra | Line | RON | Map |
|---|---|---|---|---|---|---|---|---|
| Brass Secretary | `0d8c2d7b-7cce-4115-a3f8-18c23731544e` | `{3}` | Artifact Creature — Construct | 2/1 | — | 1 | `data/brass_secretary.ron` | `review-maps/brass_secretary.json` |
| Parcel Myr | `9e660f66-0a94-4694-95e0-25296799b193` | `{1}{U}` | Artifact Creature — Clue Myr | 2/1 | — | 1 | `data/parcel_myr.ron` | `review-maps/parcel_myr.json` |
| Slinking Skirge | `82875793-b264-4ceb-8525-ef3b6d086072` | `{3}{B}` | Creature — Phyrexian Imp | 2/1 | Flying | 2 | `data/slinking_skirge.ron` | `review-maps/slinking_skirge.json` |

## Pinned rulings

- Brass Secretary and Slinking Skirge: none.
- Parcel Myr: two 2021-06-18 rulings. One states you cannot sacrifice it to its own ability and to
  another Clue/artifact sacrifice cost simultaneously; the other states it is not a created Clue
  token despite the subtype. Neither adds a rules requirement beyond the printed cost.

Packets: `build/campaign/queue-v88/families/02-sac-draw-creatures/packets/`.

## Canaries

- Simplest: Brass Secretary — no keyword and a plain artifact body.
- Hardest: Slinking Skirge — a printed keyword plus a colored body, so a cost or keyword mix-up
  shows up in the same activation the canary exercises.

## Semantic evidence

- `scenario q88_batch_a::q88_sac_draw_canary_simplest_sacrifices_the_source_and_draws_one`
- `scenario q88_batch_a::q88_sac_draw_canary_hardest_keeps_its_keyword_and_sacrifices_itself`
- `scenario q88_batch_a::q88_sac_draw_cards_require_the_printed_mana_and_their_controller`
- `scenario q88_batch_a::q88_sac_draw_cards_draw_one_and_leave_the_graveyard_alone`
- `mechanic_registry::q88_batch_a::q88_sac_draw_cards_map_definitions`
- `mechanic_registry::q88_batch_a::q88_batch_a_fingerprints_match_the_pinned_oracle_text`

## Focused evidence

- Intended red: `build/campaign/queue-v88/logs/red-q88.log` (produced before the timing and
  draw assertions were hardened).
- Registry green: `build/campaign/queue-v88/logs/green-q88-registry.log` (4 passed, shared).
- Scenario green: `build/campaign/queue-v88/logs/green-q88-scenario.log` (12 passed, shared).
- `cargo fmt --all -- --check` exit 0; `cargo clippy -D warnings` exit 0; `git diff --check`
  exit 0 (`build/campaign/queue-v88/logs/preflight-q88.log`).
- Non-vacuity probes: changing Brass Secretary's draw to two fails the per-card draw loop. The
  controller test hands priority to the foreign player, asserts she holds it, and asserts the
  exact controller/zone rejection, so it cannot pass on an earlier priority refusal.
- CR 701.21a (sacrifice), not 701.16 (Investigate).

## Conformance and scope

Pure data plus tests, maps and metadata. No engine change. Affected side: Rust and CardData.

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

CR 602.1/602.2 (activating an activated ability and paying its printed cost), CR 118.3a
(costs are paid at activation, before the effect), CR 701.21a (sacrificing a permanent as a cost; the
permanent is in the graveyard before the ability resolves) and CR 121.1 (drawing a card). No
Comprehensive Rules deviation.
