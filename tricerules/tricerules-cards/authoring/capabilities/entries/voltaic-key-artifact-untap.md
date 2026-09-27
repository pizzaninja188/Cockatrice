# An activated ability untaps a targeted artifact

## Identity

- **Pattern ID:** `voltaic-key-artifact-untap`
- **Category:** Targeted activated ability
- **Search terms:** untap artifact, any controller, self-target, tap cost, selected artifact cost, artifact target

## Behavior and limits

- **Behavior:** The ability chooses a required artifact target and untaps it only when the ability
  resolves. The two reviewed card shapes pay either {1} and the source's tap cost (Voltaic Key), or
  tap exactly two untapped artifacts the activator controls (Clock of Omens).
- **Composition and prerequisites:** The target filter requires the artifact type but does not
  require that the target be tapped or controlled by the activator. Clock of Omens may be one of
  its two cost artifacts and the chosen target; its selected tap cost is not a separate `{T}` cost.
- **Boundaries:** Tests cover an opponent's artifact, each source as target after paying its own
  cost, Clock as one of two selected tap payments, and an already untapped artifact. Nonartifact
  targets are rejected without paying costs; duplicate, tapped, foreign-controlled, nonartifact,
  too few, and too many tap-cost selections are rejected atomically for Clock.
- **Near misses:** Target removal or type change between activation and resolution is not covered
  by these card-specific scenarios.

## Runtime support

- **Status:** `Supported` for the unrestricted-controller artifact target and tested outcomes.
- **Typed symbols:** `AbilityCost::Mana`, `AbilityCost::Tap`, `AbilityCost::TapPermanents`, and
  `ObjectPaymentConstraint::ExactCount` in `tricerules/tricerules-cards/src/primitives/costs.rs`;
  `SpellEffectKind::Untap` and
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
- `clock_of_omens` — definition: [Clock of Omens](../../../data/clock_of_omens.ron) — review
  map: [map](../../review-maps/clock_of_omens.json).
- **Whole-card readiness:** Voltaic Key remains `Unassessed` in this index pass. Clock of Omens has
  a complete-definition review recorded in its linked map; this entry assesses the shared untap
  activation and does not claim generator recognition.

## Semantic test coverage

- `scenario deck_coverage_voltaic_key::voltaic_key_pays_and_taps_as_cost_then_untaps_an_opponents_artifact` —
  `tricerules/tricerules-core/tests/scenario/deck_coverage_voltaic_key.rs#voltaic_key_pays_and_taps_as_cost_then_untaps_an_opponents_artifact` — `Exercised`: one mana and the source tap are paid before the opponent's artifact untaps on resolution.
- `scenario deck_coverage_voltaic_key::voltaic_key_can_target_itself_and_untaps_after_paying_its_tap_cost` —
  `tricerules/tricerules-core/tests/scenario/deck_coverage_voltaic_key.rs#voltaic_key_can_target_itself_and_untaps_after_paying_its_tap_cost` — `Exercised`: the tapped source is a legal target and untaps on resolution.
- `scenario deck_coverage_voltaic_key::voltaic_key_may_target_an_already_untapped_artifact` —
  `tricerules/tricerules-core/tests/scenario/deck_coverage_voltaic_key.rs#voltaic_key_may_target_an_already_untapped_artifact` — `Exercised`: legal target remains untapped.
- `scenario deck_coverage_voltaic_key::voltaic_key_rejects_a_nonartifact_without_paying_any_cost` —
  `tricerules/tricerules-core/tests/scenario/deck_coverage_voltaic_key.rs#voltaic_key_rejects_a_nonartifact_without_paying_any_cost` — `Exercised`: a Forest is rejected while the source stays untapped and mana remains.
- `scenario deck_coverage_clock_of_omens::clock_taps_two_artifacts_as_cost_then_untaps_a_tapped_opponents_artifact` —
  `tricerules/tricerules-core/tests/scenario/deck_coverage_clock_of_omens.rs#clock_taps_two_artifacts_as_cost_then_untaps_a_tapped_opponents_artifact` — `Exercised`: two controlled artifacts are tapped before an opponent's tapped artifact is untapped on resolution.
- `scenario deck_coverage_clock_of_omens::clock_can_be_one_of_its_two_cost_artifacts_and_its_own_target` —
  `tricerules/tricerules-core/tests/scenario/deck_coverage_clock_of_omens.rs#clock_can_be_one_of_its_two_cost_artifacts_and_its_own_target` — `Exercised`: Clock can be tapped as one of the two cost artifacts and still be the target that untaps on resolution.
- `scenario deck_coverage_clock_of_omens::clock_rejects_incomplete_duplicate_foreign_and_nonartifact_cost_selections_atomically` —
  `tricerules/tricerules-core/tests/scenario/deck_coverage_clock_of_omens.rs#clock_rejects_incomplete_duplicate_foreign_and_nonartifact_cost_selections_atomically` — `Exercised`: incomplete, duplicate, foreign-controlled, nonartifact, excessive, and already-tapped selections cannot partially pay the cost.
- `scenario deck_coverage_clock_of_omens::clock_rejects_a_nonartifact_target_without_tapping_its_cost_artifacts` —
  `tricerules/tricerules-core/tests/scenario/deck_coverage_clock_of_omens.rs#clock_rejects_a_nonartifact_target_without_tapping_its_cost_artifacts` — `Exercised`: a nonartifact target is rejected before the selected tap cost is paid.
- `scenario deck_coverage_clock_of_omens::clock_can_target_an_already_untapped_artifact` —
  `tricerules/tricerules-core/tests/scenario/deck_coverage_clock_of_omens.rs#clock_can_target_an_already_untapped_artifact` — `Exercised`: an untapped artifact is legal and remains untapped after resolution.
- **Uncovered behavior:** Target becoming illegal after activation.
- **Inapplicable cases:** `N/A` — optional choice; activation has a required target.

## Presentation prerequisites

- The Oracle line maps the activation; the UI must offer the selected artifact target. The Rust
  scenarios do not verify target display or manual client acceptance.

## Review provenance

- **Reviewed revision:** `8cdfb09388b74cd98ff41d7940e54487c3899779`.
- **Reviewed paths:**
  - `tricerules/tricerules-cards/data/voltaic_key.ron`
  - `tricerules/tricerules-cards/authoring/review-maps/voltaic_key.json`
  - `tricerules/tricerules-core/tests/scenario/deck_coverage_voltaic_key.rs`
  - `tricerules/tricerules-cards/data/clock_of_omens.ron`
  - `tricerules/tricerules-cards/authoring/review-maps/clock_of_omens.json`
  - `tricerules/tricerules-cards/tests/deck_coverage_clock_of_omens.rs`
  - `tricerules/tricerules-core/tests/scenario/deck_coverage_clock_of_omens.rs`
  - `tricerules/tricerules-cards/src/primitives/costs.rs`
  - `tricerules/tricerules-cards/src/primitives/effects.rs`
  - `tricerules/tricerules-cards/src/primitives/abilities.rs`
  - `tricerules/tricerules-cards/src/primitives/targeting.rs`
  - `tricerules/tricerules-cards/src/registry.rs`
  - `tricerules/tricerules-core/src/engine/casting.rs`
  - `tricerules/tricerules-core/src/engine/targeting.rs`
  - `tricerules/tricerules-core/src/engine/resolution/misc.rs`
- **Review note:** Clock's definition, review map, registry test, and scenarios were reviewed against this delivered revision; the root ran the focused and full Rust/CardData gates, and Sol independently approved the frozen patch. No GUI acceptance is claimed.
