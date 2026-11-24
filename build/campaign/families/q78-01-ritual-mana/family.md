# 01-ritual-mana — Fixed-output ritual spells

- Queue: `build/campaign/queue-v78` (expanded after queue-v77 was delivered).
- Cards: 4.
- Band: parameter family of one-instruction spells.
- Status: **verified**; independent review APPROVED; batch gate pass.
- Pinned bulk: SHA-256 `a9a2fd3b550cc5cff4cec9bbc82aaf5fb8e3f781b4b95a896a0d94c15dce7853`.
- Pinned rulings: SHA-256 `3ee07dbf91347f7940b4067ab1c01e0d5097da8f6c27ef620c202ed4262aaf1f`.

## Typed shape and analogue

Every card is one `AddMana` instruction with a fixed `ManaAmount` and `ManaRetention::EndOfStep`.
Closest shipped support: Narset's Rebuke (`AddMana` in a resolving spell, including a multi-color
amount), plus Waste Not and Flamecache Gecko (`ManaRetention::EndOfStep`). `AddMana` is explicitly
the stack-using sibling of the mana-ability `ProduceMana`, so the blocked #521 activation-payment
contract does not apply.

Differences from the analogue: these cards have no other effect, no target, no cost modifier and
no card-specific resolution. `Channel the Suns` is a sorcery; the other three are instants.

## Cards

| Card | Oracle ID | Cost | Type | Oracle text | RON | Map |
|---|---|---|---|---|---|---|
| Dark Ritual | `53f7c868-b03e-4fc2-8dcf-a75bbfa3272b` | `{B}` | Instant | Add {B}{B}{B}. | `tricerules/tricerules-cards/data/dark_ritual.ron` | `authoring/review-maps/dark_ritual.json` |
| Pyretic Ritual | `86ef6474-613f-41fb-931c-d4279b03ed99` | `{1}{R}` | Instant | Add {R}{R}{R}. | `.../pyretic_ritual.ron` | `.../pyretic_ritual.json` |
| Seething Song | `64bf8929-f5f2-4d50-8667-13b1d007bcfc` | `{2}{R}` | Instant | Add {R}{R}{R}{R}{R}. | `.../seething_song.ron` | `.../seething_song.json` |
| Channel the Suns | `d1b1be96-b2aa-4386-97d2-eb0a8cc47210` | `{3}{G}` | Sorcery | Add {W}{U}{B}{R}{G}. | `.../channel_the_suns.ron` | `.../channel_the_suns.json` |

## Rulings

Pyretic Ritual has the pinned 2010-08-15 rulings: it is not a mana ability and does not behave like
one; it is cast as a spell, uses the stack, can be responded to and can be countered, and cannot be
cast in the middle of casting another spell. The same timing follows from CR 601.2 for all four
cards. CR 106.1a/106.4 add the printed mana to the controller's pool on resolution; CR 500.4 empties
unused mana at the end of the step. No other card in the family has a pinned ruling.

## Scenario evidence

Module: `tricerules-core/tests/scenario/q78_ritual_mana.rs` (registered in `tests/scenario.rs`).

- `q78_ritual_mana_canary_simplest_dark_ritual_adds_three_black_on_resolution`
- `q78_ritual_mana_canary_hardest_channel_the_suns_adds_one_of_each_color`
- `q78_ritual_mana_cards_add_only_their_printed_mana`
- `q78_ritual_mana_rejects_underfunded_casts_without_spending_mana`
- `q78_ritual_mana_empties_at_the_end_of_the_step`
- `q78_ritual_mana_pyretic_ritual_is_countered_and_adds_no_mana`
- `mechanic_registry::q78_ritual_mana::q78_ritual_mana_map_definitions`
- `mechanic_registry::q78_ritual_mana::q78_ritual_mana_fingerprints_match_the_pinned_oracle_text`

Expected outcomes are printed Oracle values: the exact printed cost leaves the pool empty at cast,
the exact printed color/count is present only after resolution, the spell object reaches the
graveyard, a countered Pyretic Ritual adds no mana, and an underfunded cast is rejected with the card retained and no mana spent. The
mid-cast timing half of the pinned ruling is enforced by the atomic CastSpell command path
(existing generic cast-procedure coverage); the response/counter half is asserted directly. The
controller is the only player affected (untargeted, no opponent choice, no hidden information);
only the activating player's pool is inspected. There are no tokens, counters or delayed effects.

## Presentation

No separately identified ability nodes; spell effects are presented by the physical card face.
`presentation-audit --check` reports 0 unresolved or invalid nodes. Fingerprints are the 11 new rows
added by the single Refresh; no unrelated fingerprint drift was produced.

## Open items

Independent review: initial bundle `q78-batch-a` (patch `8a13808a…`), corrected bundle
`q78-batch-a-v2` (patch `aac90a85…`) and final bundle `q78-batch-a-v3` (patch `b59f9d8c…`); the
final delta verdict was APPROVED. Full batch gate `verify-691906e1-74ff-48be-a2f1-4edb3e60a191`
exit 0 on the final product bytes; see `status.tsv` for the product and remote commits.
