# 02-cast-damage — Cast triggers that damage each opponent

- Queue: `build/campaign/queue-v79` (expanded after queue-v78 was delivered).
- Cards: 4.
- Band: simple single-primitive (3–10).
- Status: **verified**; independent review APPROVED; batch gate pass.
- Pinned bulk: SHA-256 `a9a2fd3b550cc5cff4cec9bbc82aaf5fb8e3f781b4b95a896a0d94c15dce7853`.
- Pinned rulings: SHA-256 `3ee07dbf91347f7940b4067ab1c01e0d5097da8f6c27ef620c202ed4262aaf1f`.

## Typed shape and analogue

Every card has one `WheneverPlayerCastsSpell` trigger with a printed `SpellCastFilter` and one
`DamagePlayer(amount: Fixed(n), who: EachOpponent)` effect. The filter is
`InstantOrSorcery` for Erebor Flamesmith and Electrostatic Field, and `Noncreature` for Kessig
Flamebreather and Black Waltz No. 3. Printed damage is 1 except Black Waltz's 2. Static keywords are
the only other characteristics: Defender (Electrostatic Field), Flying and Deathtouch (Black Waltz).

Closest shipped support: Guttersnipe is the same instant-or-sorcery trigger with 2 damage to each
opponent; Firebrand Archer is the noncreature watcher. The trigger condition, recipient and damage
effect are unchanged shipped vocabulary.

Differences from the analogues: the cards spend their parameter budget on the spell filter, damage
amount and static keywords; they add no new trigger, target or resolution behavior.

## Cards

| Card | Oracle ID | Cost | Type | Oracle text | RON | Map |
|---|---|---|---|---|---|---|
| Erebor Flamesmith | `6eb93545-a1a6-4447-82c5-421d8e9f023d` | `{1}{R}` | Creature — Dwarf Artificer | Whenever you cast an instant or sorcery spell, this creature deals 1 damage to each opponent. | `tricerules/tricerules-cards/data/erebor_flamesmith.ron` | `authoring/review-maps/erebor_flamesmith.json` |
| Electrostatic Field | `2fa94a07-c932-4f85-b6e0-a97d2b29eb52` | `{1}{R}` | Creature — Wall | Defender; whenever you cast an instant or sorcery spell, this creature deals 1 damage to each opponent. | `.../electrostatic_field.ron` | `.../electrostatic_field.json` |
| Kessig Flamebreather | `eadd9559-6dcb-4b96-8c95-58abddd0e120` | `{1}{R}` | Creature — Human Shaman | Whenever you cast a noncreature spell, this creature deals 1 damage to each opponent. | `.../kessig_flamebreather.ron` | `.../kessig_flamebreather.json` |
| Black Waltz No. 3 | `9c84c681-40c1-4ff3-86e6-48b4ab7491e5` | `{2}{B}{R}` | Legendary Creature — Wizard | Flying, deathtouch; whenever you cast a noncreature spell, Black Waltz No. 3 deals 2 damage to each opponent. | `.../black_waltz_no_3.ron` (id `black_waltz_no._3`) | `.../black_waltz_no_3.json` |

## Rulings

The 2018-10-05 Electrostatic Field rulings and the 2023-06-16 Erebor ruling confirm the trigger
resolves before the spell that caused it. Electrostatic Field's and Black Waltz's rulings add that
the trigger resolves even if that spell is countered; the scenario suite asserts the Black Waltz
case directly. Kessig has no pinned ruling. A noncreature spell is one whose card types exclude
Creature (CR 205.2a), which the `Noncreature` filter implements.

## Scenario evidence

Module: `tricerules-core/tests/scenario/q79_cast_damage.rs` (registered in `tests/scenario.rs`).

- `q79_cast_damage_canary_simplest_kessig_hits_before_the_spell_resolves`
- `q79_cast_damage_canary_hardest_black_waltz_deals_two_and_keeps_keywords`
- `q79_cast_damage_cards_apply_their_printed_damage`
- `q79_cast_damage_cards_ignore_creature_spells`
- `q79_cast_damage_black_waltz_trigger_survives_a_countered_spell`
- `q79_cast_damage_noncreature_artifact_distinguishes_the_filters`
- `q79_cast_damage_does_not_trigger_on_an_opponents_cast`
- `mechanic_registry::q79_cast_damage::q79_cast_damage_map_definitions`
- `mechanic_registry::q79_cast_damage::q79_cast_damage_fingerprints_match_the_pinned_oracle_text`

The canaries assert the trigger is on the stack above the spell, resolves first, deals exactly the
printed damage, and leaves the spell on the stack. The all-cards row repeats the printed damage for
each card and proves the spell still resolves. The creature row proves no cast-damage card triggers
on a creature spell. The counter row proves the trigger survives the spell being countered and the
countered spell does not draw. The artifact row proves a noncreature, non-instant/sorcery spell
triggers Kessig/Black Waltz but not Erebor/Electrostatic. The opponent-cast row proves the default
`Controller` caster scope does not trigger on another player's spell.

## Presentation

Electrostatic Field and Black Waltz map their keyword line and trigger line; Erebor and Kessig map
one trigger line. The metadata refresh added exactly 10 fingerprint rows across the batch (4 in this
family) and flipped the corresponding `CARDS.md` entries; no unrelated fingerprint drift was
produced. `presentation-audit --check` and the review-map batch report 0 unresolved or invalid nodes.

## Open items

Independent review: initial bundle `q79-batch-a` patch `6d8e293c…` (changes required: Dina
sacrifice-payment owner scope and missing evidence); corrected bundle `q79-batch-a-v2` patch
`57c1d846…` (approved for semantics); evidence bundle `q79-batch-a-v3` patch `f88d3795…`
(approved, v2 carries forward). Full batch gate
`verify-d9f48957-0477-404b-9cba-4820d02bbb65` exit 0 on the final product bytes; see `status.tsv`
for the product and remote commits.
