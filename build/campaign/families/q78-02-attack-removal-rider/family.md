# 02-attack-removal-rider — Attacking-creature removal with a printed rider

- Queue: `build/campaign/queue-v78` (expanded after queue-v77 was delivered).
- Cards: 3.
- Band: multi-clause composition (target plus one rider).
- Status: **verified**; independent review APPROVED; batch gate pass.
- Pinned bulk: SHA-256 `a9a2fd3b550cc5cff4cec9bbc82aaf5fb8e3f781b4b95a896a0d94c15dce7853`.
- Pinned rulings: SHA-256 `3ee07dbf91347f7940b4067ab1c01e0d5097da8f6c27ef620c202ed4262aaf1f`.

## Typed shape and analogue

One `Destroy` or `Exile` instruction with `EffectSubject::Chosen(TargetFilter { kind: Creature,
combat_role: Some(Attacking) })`, followed by one `Draw(1)` or `GainLife(3)`. One target group
(`min: 1, max: 1`, `effect_indices: [0]`). Closest shipped support: Immolating Glare, Rebuke and
Kill Shot (target-attacking destruction), Not on My Watch (target-attacking exile), Slice in Twain
(removal plus draw sharing one target group) and Soul Warden (`GainLife`).

Differences: the rider is unconditional on a legal target, but a stale sole target fizzles the whole
spell, so the rider is not applied. Silverstrike's rider is life gain rather than a draw.

## Cards

| Card | Oracle ID | Cost | Type | Oracle text | RON | Map |
|---|---|---|---|---|---|---|
| Bright Reprisal | `1de2c216-0921-45eb-9613-a68d18fbee77` | `{4}{W}` | Instant | Destroy target attacking creature. Draw a card. | `.../bright_reprisal.ron` | `authoring/review-maps/bright_reprisal.json` |
| Second Thoughts | `41bfef9f-6eb7-49c7-9b90-ff9f385ba670` | `{4}{W}` | Instant | Exile target attacking creature. Draw a card. | `.../second_thoughts.ron` | `.../second_thoughts.json` |
| Silverstrike | `8652fa06-33dc-4fe5-90cb-83cbfe5c681b` | `{3}{W}` | Instant | Destroy target attacking creature. You gain 3 life. | `.../silverstrike.ron` | `.../silverstrike.json` |

## Rulings

- Bright Reprisal, pinned 2017-09-29: an illegal target on resolution means the entire spell does
  not resolve and you do not draw; a legal indestructible target survives the destroy instruction
  but you still draw.
- Second Thoughts, pinned 2016-06-08: an illegal target on resolution means the entire spell does
  not resolve and you do not draw.
- Silverstrike has no card-specific ruling; CR 119.3 applies the 3 life only if the spell resolves.

CR 115.1a/601.2c make "attacking" part of target legality, CR 608.2b rechecks it, and CR 701.8/701.13
apply the removal.

## Scenario evidence

Module: `tricerules-core/tests/scenario/q78_attack_removal_rider.rs`.

- `q78_attack_removal_rider_canary_simplest_silverstrike_destroys_and_gains_three`
- `q78_attack_removal_rider_canary_hardest_bright_reprisal_draws_after_a_legal_destroy`
- `q78_attack_removal_rider_cards_apply_their_printed_removal_and_rider`
- `q78_attack_removal_rider_cards_reject_a_nonattacking_creature`
- `q78_attack_removal_rider_cards_fizzle_without_the_rider_on_a_stale_target`
- `q78_attack_removal_rider_legal_indestructible_target_still_draws`
- `q78_attack_removal_rider_cards_leave_a_creature_that_stopped_attacking`
- `q78_attack_removal_rider_silverstrike_still_gains_three_against_an_indestructible_attacker`
- `mechanic_registry::q78_attack_removal_rider::{...}_map_definitions`
- `mechanic_registry::q78_attack_removal_rider::{...}_fingerprints_match_the_pinned_oracle_text`

The scenarios assert exact printed mana, the creature's zone (graveyard or exile), the exact hand
count (`+1` only after a legal resolution), the exact life total (`20 + 3` for Silverstrike), and
no draw/life on a stale target or on a creature that stopped attacking at resolution; a legal
indestructible attacker still gets its rider (Bright Reprisal draws; Silverstrike gains 3 life). The illegal-target case asserts no mana is spent and the spell is
not put on the stack. Attacker/defender roles and both players' zones are checked; there are no
tokens, counters, hidden choices or delayed effects.

## Presentation

Bright Reprisal and Second Thoughts have two normalized Oracle lines: line 1 maps the removal and
target group, line 2 maps the rider. Silverstrike is one line and maps both effects plus targeting
to line 1. `presentation-audit --check` reports 0 unresolved or invalid nodes.

## Open items

Independent review: initial bundle `q78-batch-a` (patch `8a13808a…`), corrected bundle
`q78-batch-a-v2` (patch `aac90a85…`) and final bundle `q78-batch-a-v3` (patch `b59f9d8c…`); the
final delta verdict was APPROVED. Full batch gate `verify-691906e1-74ff-48be-a2f1-4edb3e60a191`
exit 0 on the final product bytes; see `status.tsv` for the product and remote commits.
