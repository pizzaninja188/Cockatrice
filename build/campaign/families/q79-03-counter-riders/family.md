# 03-counter-riders — Counter target spell plus one deterministic rider

- Queue: `build/campaign/queue-v79` (expanded after queue-v78 was delivered).
- Cards: 3.
- Band: multi-clause composition (3–5).
- Status: **verified**; independent review APPROVED; batch gate pass.
- Pinned bulk: SHA-256 `a9a2fd3b550cc5cff4cec9bbc82aaf5fb8e3f781b4b95a896a0d94c15dce7853`.
- Pinned rulings: SHA-256 `3ee07dbf91347f7940b4067ab1c01e0d5097da8f6c27ef620c202ed4262aaf1f`.

## Typed shape and analogue

Every card has one `CounterTargetSpell(spell_filter: ())` core and a deterministic, non-targeted
rider except Sokka's Haiku, whose second target is a land:

- Confirm Suspicions: `CreateTokens(token: "clue", count: 3)` (investigate three times).
- Sokka's Haiku: `Draw(1)`, `Mill(3)`, `Untap(Chosen(land))`. Two authored target groups: the stack
  spell at effect 0 and the land at effect 3. CR 608.2b is the reason the two groups are independent.
- Dismissive Denial: a `Hand`-zone activated ability with `Mana("{2}")` and `DiscardSelf` that
  searches the library for a basic land, reveals it to hand and shuffles (basic landcycling).

Closest shipped support: Contradict and Dismiss are counter + draw; Deduce and Auspicious Arrival
create Clue tokens; Absorb Vis and the other basic-landcycling spells use the exact hand-ability
shape. No new primitive or Program is needed.

Differences from the analogues: the counter spell and the rider are independent clauses. Confirm
Suspicions has no "whenever you investigate" observer, so creating three Clues at once is the same
visible result as three investigate actions. Sokka's Haiku is the only family member with two
target groups.

## Cards

| Card | Oracle ID | Cost | Type | Oracle text | RON | Map |
|---|---|---|---|---|---|---|
| Confirm Suspicions | `a30d076b-c942-45e4-b943-3749ce955291` | `{3}{U}{U}` | Instant | Counter target spell. Investigate three times. | `tricerules/tricerules-cards/data/confirm_suspicions.ron` | `authoring/review-maps/confirm_suspicions.json` |
| Sokka's Haiku | `87a97c72-143d-48a8-a1c5-3c44aa8e5a86` | `{3}{U}{U}` | Instant — Lesson | Counter target spell. Draw a card, then mill three cards. Untap target land. | `.../sokkas_haiku.ron` | `.../sokkas_haiku.json` |
| Dismissive Denial | `99eff6e9-5911-4f89-bad1-0888067a7783` | `{2}{U}{U}` | Instant | Counter target spell. Basic landcycling `{2}`. | `.../dismissive_denial.ron` | `.../dismissive_denial.json` |

## Rulings

Confirm Suspicions' 2016-04-08 rulings confirm Clue is an artifact type and that if all of a spell's
targets become illegal it does not resolve and does not investigate; the stale-target scenario
asserts zero Clues. Sokka's Haiku's only pinned ruling is presentation-only (haiku syllable count).
Dismissive Denial has no pinned ruling; basic landcycling is CR 702.29a-b and the search/shuffle is
CR 701.23a-b/701.24.

## Scenario evidence

Module: `tricerules-core/tests/scenario/q79_counter_riders.rs` (registered in `tests/scenario.rs`).

- `q79_counter_riders_canary_simplest_dismissive_denial_counters`
- `q79_counter_riders_canary_hardest_sokkas_haiku_draws_mills_and_untaps`
- `q79_counter_riders_confirm_suspicions_creates_three_clues`
- `q79_counter_riders_confirm_suspicions_stale_target_creates_no_clues`
- `q79_counter_riders_sokkas_haiku_still_resolves_when_the_spell_target_is_gone`
- `q79_counter_riders_dismissive_denial_basic_landcycling_searches_from_hand`
- `q79_counter_riders_cast_exact_printed_costs`
- `mechanic_registry::q79_counter_riders::q79_counter_riders_map_definitions`
- `mechanic_registry::q79_counter_riders::q79_counter_riders_fingerprints_match_the_pinned_oracle_text`

The canaries counter a Bolt (no damage), then seat four known cards on top and prove Sokka's Haiku
draws the first, mills the next three, untaps only the targeted land and leaves another tapped. Confirm Suspicions creates exactly three Clues.
The stale-target row counters the Bolt with a second counter before Confirm resolves and asserts the
fizzled spell creates no Clues. The Sokka row removes the spell target while the land remains legal
and asserts the spell still draws one, mills three and untaps the land. The landcycling row rejects
an unpaid activation without discarding, proves a nonbasic Taiga is not offered beside the basic
Mountain and Forest, and asserts the reveal event and shuffle log after choosing the Forest. The cost row proves all three spells spend exactly the printed mana and
counter the Bolt.

## Presentation

Confirm Suspicions maps the counter and Clue lines; Sokka's Haiku maps its three printed lines and
both target groups; Dismissive Denial maps the counter line and the basic landcycling line. The
metadata refresh added exactly 10 fingerprint rows across the batch (3 in this family) and flipped
the corresponding `CARDS.md` entries; no unrelated fingerprint drift was produced.
`presentation-audit --check` and the review-map batch report 0 unresolved or invalid nodes.

## Open items

Independent review: initial bundle `q79-batch-a` patch `6d8e293c…` (changes required: Dina
sacrifice-payment owner scope and missing evidence); corrected bundle `q79-batch-a-v2` patch
`57c1d846…` (approved for semantics); evidence bundle `q79-batch-a-v3` patch `f88d3795…`
(approved, v2 carries forward). Full batch gate
`verify-d9f48957-0477-404b-9cba-4820d02bbb65` exit 0 on the final product bytes. Batch queue-v79 batch A covering 01-gain-life-drain..03-counter-riders (10 cards) used gate
`verify-d9f48957-0477-404b-9cba-4820d02bbb65` exit 0.
See `status.tsv` for the product and remote commits.
