# 01-sliver-pump-grant — All Sliver creatures gain one activated pump

- Queue: `build/campaign/queue-v80` (expanded after queue-v79 was fully delivered).
- Cards: 3.
- Band: simple static grant (3–10).
- Status: **verified**; independent review APPROVED; batch gate pass.
- Pinned bulk: SHA-256 `a9a2fd3b550cc5cff4cec9bbc82aaf5fb8e3f781b4b95a896a0d94c15dce7853`.
- Pinned rulings: SHA-256 `3ee07dbf91347f7940b4067ab1c01e0d5097da8f6c27ef620c202ed4262aaf1f`.

## Typed shape and analogue

Each card has one `StaticAbilityDef::GrantActivatedAbilityToPermanents` whose filter is
`(kind: Creature, required_subtypes: ["Sliver"])`. The granted child is one `{2}` mana activated
ability with `PumpTarget(subject: Source)`. The three cards differ only in the printed pump:
Armor `+0/+1`, Barbed `+1/+0`, Spectral `+1/+1`.

Closest shipped support: Chromatic Lantern and Ichormoon Gauntlet for
`GrantActivatedAbilityToPermanents`; the queue-v76 all-Sliver anthem cohort for the Sliver scope; and
Kjeldoran Outrider/Nightwing Shade for a source-only pump.

Differences: the pump belongs to every Sliver permanent, including opponents' Slivers, and each
activation is independent and cumulative. The parent Oracle line is one line; the child grant uses
`Fallback` because the complete instruction belongs to the parent.

## Cards

| Card | Oracle ID | Cost | Type | Oracle text | RON | Map |
|---|---|---|---|---|---|---|
| Armor Sliver | `c284b1fd-f238-478a-bcf1-c4b8f32f11f3` | `{2}{W}` | Creature — Sliver | All Sliver creatures have "{2}: This creature gets +0/+1 until end of turn." | `tricerules/tricerules-cards/data/armor_sliver.ron` | `authoring/review-maps/armor_sliver.json` |
| Barbed Sliver | `8fbca9ba-8b83-40ca-9f7e-5c483ba76675` | `{2}{R}` | Creature — Sliver | All Sliver creatures have "{2}: This creature gets +1/+0 until end of turn." | `.../barbed_sliver.ron` | `.../barbed_sliver.json` |
| Spectral Sliver | `b08d4381-1a75-4c59-9551-b3133f7495a3` | `{2}{B}` | Creature — Sliver Spirit | All Sliver creatures have "{2}: This creature gets +1/+1 until end of turn." | `.../spectral_sliver.ron` | `.../spectral_sliver.json` |

## Rulings

CR 113.10 and 602.2 make the quoted ability an ability of each affected permanent. CR 611.3a and
613.4c make the pump cumulative. The pinned 2013-07-01 rulings confirm cumulative grants and that a
creature that stops being a Sliver stops receiving its own grant. All three cards have the same two
pinned rulings.

## Scenario evidence

Module: `tricerules-core/tests/scenario/q80_sliver_pump.rs` (registered in `tests/scenario.rs`).

- `q80_sliver_pump_canary_simplest_armor_sliver_grants_toughness`
- `q80_sliver_pump_canary_hardest_barbed_sliver_pumps_each_sliver_and_is_cumulative`
- `q80_sliver_pump_cards_all_apply_their_printed_pump`
- `q80_sliver_pump_scope_includes_each_players_slivers_but_not_other_creatures`
- `q80_sliver_pump_grant_disappears_when_its_source_leaves`
- `mechanic_registry::q80_sliver_pump::q80_sliver_pump_map_definitions`
- `mechanic_registry::q80_sliver_pump::q80_sliver_pump_fingerprints_match_the_pinned_oracle_text`

The scope test asserts that an opponent's Metallic Sliver publishes the grant while a Grizzly Bears
does not, and that the non-Sliver cannot use the stale slot. The source-leaves test asserts that the
granted slot disappears and its old index is rejected.

## Presentation

Each parent line maps `/faces/0/static_abilities/0`, with primitive references for the grant, the
Sliver subtype filter, the `{2}` mana cost, `PumpTarget`, and `EffectSubject::Source`. The metadata
refresh added exactly three fingerprint rows and three `CARDS.md` checkboxes.

## Focused evidence

- Intended red: `build/verification-logs/20261124-181317-255-Q80-sliver-canary-intended-red-v2.log` (MissingCard, exit 101).
- Simplest canary green: `build/verification-logs/20261124-183000-074-Q80-sliver-simplest-canary-green-v4.log`.
- Hardest canary green: `build/verification-logs/20261124-183257-888-Q80-sliver-hardest-canary-green-v2.log`.
- Family green: `build/verification-logs/20261124-195234-383-Q80-all-focused-scenarios-v3.log`.
- Registry green: `build/verification-logs/20261124-195314-510-Q80-all-registry-tests-v4.log`.
- Formatting: `cargo fmt --all -- --check` exit 0.
- Map preparation: `build/verification-logs/20261124-195330-705-Q80-card-evidence-preparation-v3.log` exit 0.

## Review and gate

Independent frozen review: APPROVED (reviewers read `build/campaign/review/q80-batch-a/product-v2.patch`,
SHA-256 `7261e70808c75db607212d218be6b212b6853c89a110aba108b7064ce957d07d`; sliver and kicker approved
on v1 and carried forward, protection delta-approved on v2). Batch queue-v80 batch A full gate `verify-7230649c-8d9f-4a9c-8007-2f6539e126dd` exit 0 on the final product bytes; the batch
covers 01-sliver-pump-grant..03-kicker-spells (12 cards). No new primitive, shared-test change or
typed contract.
