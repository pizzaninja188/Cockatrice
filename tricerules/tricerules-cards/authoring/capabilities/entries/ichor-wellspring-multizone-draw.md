# One draw trigger for entry and battlefield-to-graveyard

## Identity

- **Pattern ID:** `ichor-wellspring-multizone-draw`
- **Category:** Triggered ability
- **Search terms:** enters, dies, graveyard event, draw one, same ability ID

## Behavior and limits

- **Behavior:** Ichor Wellspring's one triggered ability draws one card when it enters and when it
  is put into a graveyard from the battlefield.
- **Composition and prerequisites:** Both event forms share one ability definition and one
  `Draw(Controller, 1)` effect.
- **Boundaries:** The semantic scenario destroys Wellspring while its entry trigger is waiting on
  the stack; both resulting triggers then resolve to one draw each.
- **Near misses:** This does not cover movement to a graveyard from another zone, repeated copies of
  the permanent, or different controller behavior.

## Runtime support

- **Status:** `Supported` for these two events on the same Ichor Wellspring ability.
- **Typed symbols:** `TriggerCondition::WhenSelfEntersOrIsPutIntoGraveyardFromBattlefield` in
  `tricerules/tricerules-cards/src/primitives/abilities.rs`; `SpellEffectKind::Draw` in
  `tricerules/tricerules-cards/src/primitives/effects.rs`.
- **Implementation references:** Loader: `tricerules/tricerules-cards/src/registry.rs#CardRegistry::from_embedded`
  and `tricerules/tricerules-cards/src/registry.rs#CardRegistry::from_chunks_and_tokens`; validator:
  `tricerules/tricerules-cards/src/primitives/abilities.rs#TriggeredAbilityDef::validate_shape`
  and `tricerules/tricerules-cards/src/primitives/effects.rs#SpellEffectKind::validate_list`;
  event collector: `tricerules/tricerules-core/src/engine/triggers.rs#collect_event_triggers`;
  draw consumer: `tricerules/tricerules-core/src/engine/resolution/zones.rs#draw`.
- **Evidence and limits:** The fixture confirms the entry trigger exists, destroys the source before
  that trigger resolves, observes a second trigger, and checks one draw from each. It does not
  establish every zone-change or replacement interaction.

## Generator recognition

- **Status:** `Unassessed`.
- **Recipe and input shapes:** Not reviewed for this entry.
- **References and limits:** The semantic fixture does not establish generator recognition.

## Shipped card evidence

- `ichor_wellspring` — definition: [Ichor Wellspring](../../../data/ichor_wellspring.ron) — review
  map: [map](../../review-maps/ichor_wellspring.json).
- **Whole-card readiness:** `Unassessed` in this index pass. The linked map records prior complete-definition review; this entry assesses this trigger only.

## Semantic test coverage

- `scenario deck_coverage_ichor_wellspring::ichor_wellspring_draws_for_entry_and_graveyard_events_on_one_ability` —
  `tricerules/tricerules-core/tests/scenario/deck_coverage_ichor_wellspring.rs#ichor_wellspring_draws_for_entry_and_graveyard_events_on_one_ability` — `Exercised`: entry trigger waits on the stack, Disenchant moves the source to the graveyard before it resolves, the same ability identity creates another trigger, and both draw exactly one.
- **Uncovered behavior:** Other graveyard-entry causes and interactions that replace either move.
- **Inapplicable cases:** `N/A` — target selection and choice selection; this trigger has neither.

## Presentation prerequisites

- The single Oracle line maps to the trigger. No special token, target, or choice presentation is
  defined. The scenario does not test client rendering or manual acceptance.

## Review provenance

- **Reviewed revision:** `34bb8f7ccaee20e69ae3d0c90dde65edaf23f76b`.
- **Reviewed paths:**
  - `tricerules/tricerules-cards/data/ichor_wellspring.ron`
  - `tricerules/tricerules-cards/authoring/review-maps/ichor_wellspring.json`
  - `tricerules/tricerules-core/tests/scenario/deck_coverage_ichor_wellspring.rs`
  - `tricerules/tricerules-cards/src/primitives/abilities.rs`
  - `tricerules/tricerules-cards/src/primitives/effects.rs`
  - `tricerules/tricerules-cards/src/registry.rs`
  - `tricerules/tricerules-core/src/engine/triggers.rs`
  - `tricerules/tricerules-core/src/engine/resolution/zones.rs`
- **Review note:** Existing assertions were inspected but not rerun in this documentation phase. No GUI acceptance is claimed.
