# 02-any-color-tap-creature — the three "{T}, Tap an untapped creature you control" mana creatures

- Queue: `build/campaign/queue-v90`.
- Cards: 3.
- Band: simple single-primitive cohort (3).
- Status: **verified**; independent read-only review approved the reviewed semantics (APPROVED at
  bundle revision 10) and the queue-v90 consolidated batch gate passed exit 0.
- Pinned bulk: SHA-256 `1cee09ee1ac70977c46171f4abf5cdfe170ab43bc32232af9138404e27524ce6`.
- Pinned rulings: SHA-256 `84f4c793e6d4f1256206b915da007111e1a445fe7cad99147802086454e8a95d`.

## Typed shape and analogue

Each card's only activated ability is
`{T}, Tap an untapped creature you control: Add one mana of any color.` That is
`costs: [Tap, TapPermanents(constraint: ExactCount(1), filter: (kind: Creature, controller: You),
exclude_source: true)]` with
`effect: [ProduceMana(options: [(w: 1), (u: 1), (b: 1), (r: 1), (g: 1)])]`.

Shipped analogues: **Springleaf Drum** (`bdeb440d-a714-40e8-9038-f651e6ae45fb`), which ships
exactly this cost/effect pair on an artifact, and **Scene of the Crime**, which ships it as the
second ability of an artifact land. `#499` already generalised the storage-counter X payments, and
`#190` the restricted-mana any-colour paths; neither is touched here.

The three cards differ in body and in one printed keyword, so the family asserts those per card:
Loam Dryad 1/2 with no keyword, Saruli Caretaker 0/3 with Defender, Jaspera Sentinel 1/2 with Reach.
The keyword is a `keywords:` list entry, **not** a creature subtype, and the family's registry test
asserts the full type line separately from the keyword line.

## Cards

| Card | Oracle ID | Body | Keyword | RON | Map |
|---|---|---|---|---|---|
| Loam Dryad | `1420b7db-859b-4ad5-9844-10413cc70478` | 1/2 | — | `data/loam_dryad.ron` | `review-maps/loam_dryad.json` |
| Saruli Caretaker | `5e2736e6-b812-43a4-bd79-6f6095344fe1` | 0/3 | Defender | `data/saruli_caretaker.ron` | `review-maps/saruli_caretaker.json` |
| Jaspera Sentinel | `e2f98813-4f39-4135-b05b-407e9eb797f6` | 1/2 | Reach | `data/jaspera_sentinel.ron` | `review-maps/jaspera_sentinel.json` |

## Pinned rulings

All three carry the same late ruling (2016-04-08 / 2019-01-25 / 2021-02-05):

> You can tap any untapped creature you control, including one you haven't controlled continuously
> since the beginning of your most recent turn, to pay the cost … You must have controlled ⟨this
> creature⟩ continuously since the beginning of your most recent turn, however.

That is the asymmetry between the source (a `{T}` activation cost, so CR 302.6 summoning sickness
applies to it) and the second creature (a `TapPermanents` *cost selection*, CR 602.1a, not a `{T}`
activation and so not summoning-sick). `q90_any_color_checks_source_and_payer_summoning_sickness`
asserts both halves: a summoning-sick **source** is refused, and a summoning-sick **payer** is
accepted and tapped.

## Copy-from and differences

Copy the reviewed Springleaf Drum shape verbatim; only identity, body and keyword differ per card.

Differences from Springleaf Drum: these are creatures, so the source itself is summoning-sick until
its controller's next turn and cannot pay its own `{T}` cost; Springleaf Drum is an artifact with no
summoning sickness. The family asserts the source-side summoning-sickness refusal in
`q90_any_color_checks_source_and_payer_summoning_sickness`.

## Rules evidence

CR 602.1a/602.2 (the printed costs are all part of one activation), CR 118.3 (mana payment), CR
701.26a (the tap action), CR 605.1a (a mana ability resolves without using the stack), CR 302.6 (the
source is summoning-sick; the selected payer is not), CR 115.1c (a cost is not a target).

## Semantic evidence

- `scenario q90_batch_a::q90_any_color_canary_taps_a_second_creature_for_one_chosen_color`
- `scenario q90_batch_a::q90_any_color_requires_a_second_untapped_creature`
- `scenario q90_batch_a::q90_any_color_requires_exactly_one_own_creature`
- `scenario q90_batch_a::q90_any_color_refuses_a_tapped_helper_or_a_tapped_source`
- `scenario q90_batch_a::q90_any_color_checks_source_and_payer_summoning_sickness`
- `scenario q90_batch_a::q90_any_color_creatures_offer_all_five_colors_and_keep_their_keywords`
- `mechanic_registry::q90_batch_a::q90_any_color_tap_creature_maps_definitions`
- `mechanic_registry::q90_batch_a::q90_batch_fingerprints_match_the_pinned_oracle_text`

## Canaries

- Simplest: Loam Dryad (no keyword, 1/2).
- Hardest: Jaspera Sentinel (Reach plus the same ability, so a dropped keyword fails) and Saruli
  Caretaker (Defender, 0/3).

## Focused evidence

- Intended red: `build/campaign/queue-v90/logs/red-q90-registry.log` (3 failed / 2 passed) and
  `red-q90-scenario.log` (16 failed / 0 passed) with all seventeen RONs held out.
- Focused green: `build/campaign/queue-v90/logs/green-q90-registry.log` (5 passed) and
  `green-q90-scenario.log` (16 passed).
- Non-vacuity probe: `build/campaign/queue-v90/logs/probe-anycolor-count.log` mutates Jaspera
  Sentinel's `ExactCount(1)` to `AggregateMinimum(minimum: 1, CurrentPower)`; the scenario fails
  5 passed / 1 failed on "the cost is exactly one creature, not two". Restored and hash-verified.
- Format, Clippy and `git diff --check`: exit 0 each.
- Card evidence preparation: PASS over 3566 maps.

## Defects this family's own tests caught before freeze

1. The first generated RONs put the printed keyword in `types` (`["Creature", "Reach"]`), which
   would have made Reach a creature subtype. The registry test's exact type-line assertion caught
   it; the keyword moved to `keywords`.
2. `exclude_source` looked like the guard that stops the source paying its own tap cost, and a
   scenario-only probe mutating it to `false` passed. It is *not* the operative guard in the
   self-payment case: `AbilityCost::Tap` has already debited the source, so
   `revalidate_cost_transaction` refuses the second tap with "one object cannot pay two consuming or
   tap costs" (`tricerules-core/src/engine/payment/transaction.rs`). `exclude_source` only removes
   the *candidate* from the tap selection (`.../payment/components.rs`, `excluded_incarnation`), and
   the registry test asserts the flag itself, so that mutation cannot pass the full suite — only the
   scenario subset. The probe was therefore replaced with one that mutates the *authored* contract —
   the `ExactCount(1)` bound — which the scenario does catch.

## Batch gate

`build/verification-logs/verify-8c1fef97-ec11-4987-9219-5a25ff4870a8` — Status Pass, exit 0,
covering `01-planeswalker-untap-lands`..`03-keyword-tap-target` (17 cards). Environment, diff
check, CardData, dependency boundary, five package format checks, Clippy, and the full Rust
suite; all 21 q90 tests (5 registry, 16 scenario) were observed passing in that run.
Product commit `f01e5bd20`.
