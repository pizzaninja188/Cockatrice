# Voltaic Key untaps a targeted artifact

## Identity

- **Pattern ID:** `voltaic-key-artifact-untap`
- **Category:** Targeted activated ability
- **Search terms:** untap artifact, any controller, self-target, tap cost, artifact target

## Behavior and limits

- **Behavior:** While activating Voltaic Key, its controller chooses a required artifact target and
  pays {1} and the tap cost; that artifact is untapped only when the ability resolves.
- **Composition and prerequisites:** The target filter requires the artifact type but does not
  require that the target be tapped or controlled by the activator.
- **Boundaries:** Tests cover an opponent's artifact, Voltaic Key itself after paying its tap cost,
  and an already untapped artifact. A nonartifact target is rejected without paying costs.
- **Near misses:** Target removal or type change between activation and resolution is not covered.

## Runtime support

- **Status:** `Supported` for the unrestricted-controller artifact target and tested outcomes.
- **Typed symbols:** `AbilityCost::Mana` and `AbilityCost::Tap` in
  `tricerules/tricerules-cards/src/primitives/costs.rs`; `SpellEffectKind::Untap` and
  `EffectSubject::Chosen` in `tricerules/tricerules-cards/src/primitives/effects.rs`;
  `TargetFilter.permanent_types` in `tricerules/tricerules-cards/src/primitives/targeting.rs`.
- **Implementation references:** Loader: `tricerules/tricerules-cards/src/registry.rs#CardRegistry::from_embedded`
  and `tricerules/tricerules-cards/src/registry.rs#CardRegistry::from_chunks_and_tokens`; validator:
  `tricerules/tricerules-cards/src/primitives/abilities.rs#ActivatedAbilityDef::validate_shape`
  and `tricerules/tricerules-cards/src/primitives/targeting.rs#TargetingDef::validate_optional`; activation and target validation:
  `tricerules/tricerules-core/src/engine/casting.rs#GameEngine::activate_ability` and
  `tricerules/tricerules-core/src/engine/targeting.rs#validate_ability_targets`; consumer:
  `tricerules/tricerules-core/src/engine/resolution/misc.rs#untap`.
- **Evidence and limits:** The cited scenarios check cost payment before resolution, unchanged
  source state afterward, self-targeting, the no-op on an untapped artifact, and nonartifact
  rejection with unspent costs.

## Generator recognition

- **Status:** `Unassessed`.
- **Recipe and input shapes:** Not reviewed for this entry.
- **References and limits:** These engine tests do not establish generator recognition.

## Shipped card evidence

- `voltaic_key` — definition: [Voltaic Key](../../../data/voltaic_key.ron) — review map:
  [map](../../review-maps/voltaic_key.json).
- **Whole-card readiness:** `Unassessed` in this index pass. The linked map records prior complete-definition review; this entry assesses only the untap activation.

## Semantic test coverage

- `scenario deck_coverage_voltaic_key::voltaic_key_pays_and_taps_as_cost_then_untaps_an_opponents_artifact` —
  `tricerules/tricerules-core/tests/scenario/deck_coverage_voltaic_key.rs#voltaic_key_pays_and_taps_as_cost_then_untaps_an_opponents_artifact` — `Exercised`: one mana and the source tap are paid before the opponent's artifact untaps on resolution.
- `scenario deck_coverage_voltaic_key::voltaic_key_can_target_itself_and_untaps_after_paying_its_tap_cost` —
  `tricerules/tricerules-core/tests/scenario/deck_coverage_voltaic_key.rs#voltaic_key_can_target_itself_and_untaps_after_paying_its_tap_cost` — `Exercised`: the tapped source is a legal target and untaps on resolution.
- `scenario deck_coverage_voltaic_key::voltaic_key_may_target_an_already_untapped_artifact` —
  `tricerules/tricerules-core/tests/scenario/deck_coverage_voltaic_key.rs#voltaic_key_may_target_an_already_untapped_artifact` — `Exercised`: legal target remains untapped.
- `scenario deck_coverage_voltaic_key::voltaic_key_rejects_a_nonartifact_without_paying_any_cost` —
  `tricerules/tricerules-core/tests/scenario/deck_coverage_voltaic_key.rs#voltaic_key_rejects_a_nonartifact_without_paying_any_cost` — `Exercised`: a Forest is rejected while the source stays untapped and mana remains.
- **Uncovered behavior:** Target becoming illegal after activation.
- **Inapplicable cases:** `N/A` — optional choice; activation has a required target.

## Presentation prerequisites

- The Oracle line maps the activation; the UI must offer the selected artifact target. The Rust
  scenarios do not verify target display or manual client acceptance.

## Review provenance

- **Reviewed revision:** `34bb8f7ccaee20e69ae3d0c90dde65edaf23f76b`.
- **Reviewed paths:**
  - `tricerules/tricerules-cards/data/voltaic_key.ron`
  - `tricerules/tricerules-cards/authoring/review-maps/voltaic_key.json`
  - `tricerules/tricerules-core/tests/scenario/deck_coverage_voltaic_key.rs`
  - `tricerules/tricerules-cards/src/primitives/costs.rs`
  - `tricerules/tricerules-cards/src/primitives/effects.rs`
  - `tricerules/tricerules-cards/src/primitives/abilities.rs`
  - `tricerules/tricerules-cards/src/primitives/targeting.rs`
  - `tricerules/tricerules-cards/src/registry.rs`
  - `tricerules/tricerules-core/src/engine/casting.rs`
  - `tricerules/tricerules-core/src/engine/targeting.rs`
  - `tricerules/tricerules-core/src/engine/resolution/misc.rs`
- **Review note:** Existing assertions were inspected but not rerun in this documentation phase. No GUI acceptance is claimed.
