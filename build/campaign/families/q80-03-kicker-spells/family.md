# 03-kicker-spells — One kicker option plus a deterministic printed rider

- Queue: `build/campaign/queue-v80`.
- Cards: 4.
- Band: multi-clause composition (3–5).
- Status: **verified**; independent review APPROVED; batch gate pass.
- Pinned bulk: SHA-256 `a9a2fd3b550cc5cff4cec9bbc82aaf5fb8e3f781b4b95a896a0d94c15dce7853`.
- Pinned rulings: SHA-256 `3ee07dbf91347f7940b4067ab1c01e0d5097da8f6c27ef620c202ed4262aaf1f`.

## Typed shape and analogue

Each card has one `cast_cost_groups` entry whose only option is `ManaCostChoiceKind::Kicker` or
`ObjectCastCostKind::Kicker`, then a printed effect whose amount or presence reads the stable
cast-cost receipt. No new primitive or shared test change is needed.

- Roil Eruption: `DamageTarget(amount: CastCost(... when_true: 5, otherwise: 3), kind: AnyTarget)`.
- Blink of an Eye: shared-target `ReturnToOwnersHand` plus `Draw(count: CastCost(... 1 / 0))`.
- Final Flourish / Eject the Warp Core: base `PumpTarget(-2/-2)` plus `ConditionalCastCost` with a
  further `-4/-4`, producing the printed `-6/-6` "instead" value. The same additive composition is
  shipped on Stomped by the Foot.

Closest shipped support: Burst Lightning for receipt-gated damage, Into the Roil and Whoosh! for
receipt-gated draw after a shared return, and Stomped by the Foot for the sacrifice kicker plus
additive pump. The sacrifice filter reuses the existing artifact-or-creature `any_of` shape.

## Cards

| Card | Oracle ID | Cost | Type | Oracle text | RON | Map |
|---|---|---|---|---|---|---|
| Blink of an Eye | `51acd176-e84d-42c2-acb2-cceeb28a9fec` | `{1}{U}` | Instant | Kicker `{1}{U}`. Return target nonland permanent to its owner's hand. If this spell was kicked, draw a card. | `tricerules/tricerules-cards/data/blink_of_an_eye.ron` | `authoring/review-maps/blink_of_an_eye.json` |
| Eject the Warp Core | `c06c17c2-203c-4b4a-8564-f29bc41c9eb5` | `{1}{B}` | Instant | Kicker—Sacrifice an artifact or creature. Target creature gets -2/-2 until end of turn. If this spell was kicked, that creature gets -6/-6 instead. | `.../eject_the_warp_core.ron` | `.../eject_the_warp_core.json` |
| Final Flourish | `63a85361-56bc-4209-aea8-b49319bc7b92` | `{1}{B}` | Instant | Kicker—Sacrifice an artifact or creature. Target creature gets -2/-2 until end of turn. If this spell was kicked, that creature gets -6/-6 instead. | `.../final_flourish.ron` | `.../final_flourish.json` |
| Roil Eruption | `6798163a-864f-4844-96b1-77585a7e7ab4` | `{1}{R}` | Sorcery | Kicker `{5}`. Roil Eruption deals 3 damage to any target. If this spell was kicked, it deals 5 damage instead. | `.../roil_eruption.ron` | `.../roil_eruption.json` |

## Rulings

CR 702.33a-e (kicker), 601.2b/f-h (additional costs), 608.2b (target revalidation), 400.3 (owner's
zone), and 121.1-121.2 (draw) govern. The 2018-04-27 ruling confirms a kicked Blink of an Eye with
an illegal sole target does not resolve and draws nothing. The 2024-11-08 rulings confirm a kicked
spell is kicked and a copy of a kicked spell is kicked; the tests assert the cast-time receipt.
Eject the Warp Core has no pinned rulings.

## Scenario evidence

Module: `tricerules-core/tests/scenario/q80_kicker_spells.rs` (registered in `tests/scenario.rs`).

- `q80_kicker_canary_simplest_roil_eruption_deals_three_or_five`
- `q80_kicker_canary_hardest_final_flourish_pays_sacrifice_for_six`
- `q80_kicker_cards_apply_base_and_kicked_modes`
- `q80_kicker_blink_fizzles_without_kicked_draw_when_its_only_target_is_gone`
- `q80_kicker_final_flourish_rejects_kicker_without_a_sacrifice`
- `mechanic_registry::q80_kicker_spells::q80_kicker_map_definitions`
- `mechanic_registry::q80_kicker_spells::q80_kicker_fingerprints_match_the_pinned_oracle_text`

The all-card test casts every card at base and kicked, asserts exact damage, return to the owner's
hand, the optional draw, and the exact pump values. The stale-target test removes Blink's only target
before resolution and asserts no draw. The rejected-kicker test proves the sacrifice option cannot be
announced without a legal object and spends nothing.

## Presentation

Each card maps line 1 to its `cast_cost_groups/0` and line 2 to the printed effect and (for Blink and
the two sacrifice cards) the single target group. Primitive references cover the cost group, the
mana or sacrifice option, the effect, the `Amount::CastCost` receipt and `ConditionalCastCost`.
The metadata refresh added exactly four fingerprint rows and four `CARDS.md` checkboxes.

## Focused evidence

- Intended red: `build/verification-logs/20261124-184118-451-Q80-kicker-canary-intended-red.log` (MissingCard, exit 101).
- Simplest canary green: `build/verification-logs/20261124-184456-299-Q80-kicker-simplest-canary-green.log`.
- Hardest canary green: `build/verification-logs/20261124-191920-076-Q80-kicker-hardest-canary-green-v2.log`.
- Family green: `build/verification-logs/20261124-192240-312-Q80-kicker-family-focused-green.log`.
- Combined family green: `build/verification-logs/20261124-193344-061-Q80-all-focused-scenarios-v2.log`.
- Registry green: `build/verification-logs/20261124-195314-510-Q80-all-registry-tests-v4.log`.
- Formatting: `cargo fmt --all -- --check` exit 0.
- Map preparation: `build/verification-logs/20261124-195330-705-Q80-card-evidence-preparation-v3.log` exit 0.

## Review and gate

Independent frozen review: APPROVED (reviewers read `build/campaign/review/q80-batch-a/product-v2.patch`,
SHA-256 `7261e70808c75db607212d218be6b212b6853c89a110aba108b7064ce957d07d`; sliver and kicker approved
on v1 and carried forward, protection delta-approved on v2). Batch queue-v80 batch A full gate `verify-7230649c-8d9f-4a9c-8007-2f6539e126dd` exit 0 on the final product bytes; the batch
covers 01-sliver-pump-grant..03-kicker-spells (12 cards). No new primitive, shared-test change or
typed contract.
