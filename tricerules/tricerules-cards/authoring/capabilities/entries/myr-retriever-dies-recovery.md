# Myr Retriever returns another artifact from its controller's graveyard

## Identity

- **Pattern ID:** `myr-retriever-dies-recovery`
- **Category:** Targeted triggered ability
- **Search terms:** dies trigger, artifact card, own graveyard, return to hand, simultaneous death

## Behavior and limits

- **Behavior:** When Myr Retriever dies, its controller chooses one other artifact card in that
  controller's graveyard; the chosen card returns to hand on resolution.
- **Composition and prerequisites:** The graveyard-card effect is a mandatory single target. Its
  filter excludes the source and restricts the card type and graveyard owner.
- **Boundaries:** The scenario shows an artifact that dies simultaneously with Myr Retriever can
  be selected, then returned to its controller's hand.
- **Near misses:** The test does not cover a chosen card leaving the graveyard after selection and
  before resolution, or any graveyard other than the trigger controller's.

## Runtime support

- **Status:** `Supported` for this dies trigger and its tested graveyard filter/return path.
- **Typed symbols:** `TriggerCondition::WhenSelfDies` in
  `tricerules/tricerules-cards/src/primitives/abilities.rs`; `SpellEffectKind::MoveGraveyardCards`,
  `TargetObjectExclusion::Source`, and `GraveyardDestination::Hand` in
  `tricerules/tricerules-cards/src/primitives/effects.rs`; `TargetGroupDef` in
  `tricerules/tricerules-cards/src/primitives/targeting.rs`.
- **Implementation references:** Loader: `tricerules/tricerules-cards/src/registry.rs#CardRegistry::from_embedded`
  and `tricerules/tricerules-cards/src/registry.rs#CardRegistry::from_chunks_and_tokens`; validator:
  `tricerules/tricerules-cards/src/primitives/abilities.rs#TriggeredAbilityDef::validate_shape`
  and `tricerules/tricerules-cards/src/primitives/targeting.rs#TargetingDef::validate_optional`; target validator:
  `tricerules/tricerules-core/src/engine/targeting.rs#validate_ability_targets`; consumer:
  `tricerules/tricerules-core/src/engine/resolution/zones.rs#move_graveyard_cards`.
- **Evidence and limits:** After a board wipe, the test checks the simultaneous-death artifact is
  offered, the source, a nonartifact, and an opponent's artifact are excluded, self-selection is
  rejected, and the chosen artifact moves to hand.

## Generator recognition

- **Status:** `Unassessed`.
- **Recipe and input shapes:** Not reviewed for this entry.
- **References and limits:** This runtime coverage says nothing about generator recognition.

## Shipped card evidence

- `myr_retriever` — definition: [Myr Retriever](../../../data/myr_retriever.ron) — review map:
  [map](../../review-maps/myr_retriever.json).
- **Whole-card readiness:** `Unassessed` in this index pass. The linked map records prior complete-definition review; this entry assesses the dies trigger only.

## Semantic test coverage

- `scenario deck_coverage_myr_retriever::myr_retriever_targets_another_artifact_that_dies_at_the_same_time` —
  `tricerules/tricerules-core/tests/scenario/deck_coverage_myr_retriever.rs#myr_retriever_targets_another_artifact_that_dies_at_the_same_time` — `Exercised`: simultaneous death, candidate filtering, self-target rejection, and the selected card's return to hand.
- **Uncovered behavior:** Target becoming illegal after selection but before resolution.
- **Inapplicable cases:** `N/A` — optional choice; the target is mandatory in this definition.

## Presentation prerequisites

- The Oracle line maps the ability, and its target group supplies the graveyard-card prompt. A client
  must present that target selection; the Rust scenario does not establish GUI acceptance.

## Review provenance

- **Reviewed revision:** `34bb8f7ccaee20e69ae3d0c90dde65edaf23f76b`.
- **Reviewed paths:**
  - `tricerules/tricerules-cards/data/myr_retriever.ron`
  - `tricerules/tricerules-cards/authoring/review-maps/myr_retriever.json`
  - `tricerules/tricerules-core/tests/scenario/deck_coverage_myr_retriever.rs`
  - `tricerules/tricerules-cards/src/primitives/abilities.rs`
  - `tricerules/tricerules-cards/src/primitives/effects.rs`
  - `tricerules/tricerules-cards/src/primitives/targeting.rs`
  - `tricerules/tricerules-cards/src/registry.rs`
  - `tricerules/tricerules-core/src/engine/targeting.rs`
  - `tricerules/tricerules-core/src/engine/resolution/zones.rs`
- **Review note:** Existing assertions were inspected but not rerun in this documentation phase. No GUI acceptance is claimed.
