# Permanent taps for {C}

## Identity

- **Pattern ID:** `permanent-taps-for-colorless`
- **Category:** Mana ability
- **Search terms:** tap for colorless, `{T}: add {C}`, land mana, artifact mana

## Behavior and limits

- **Behavior:** A battlefield permanent taps to add one colorless mana to its controller's pool; the
  two cited scenarios show this ability resolving immediately without using the stack.
- **Composition and prerequisites:** The authored activation has a tap cost and one `ProduceMana`
  option containing one colorless mana.
- **Boundaries:** Reviewed on the artifact Mind Stone and the land Scavenger Grounds. Each remains
  on the battlefield tapped after activation.
- **Near misses:** A source sacrificed for mana, colored or multiple mana, and additional mana
  restrictions are not covered here.

## Runtime support

- **Status:** `Supported` for the exact one-colorless, tap-only ability in the two cited cards.
- **Typed symbols:** `AbilityCost::Tap` in `tricerules/tricerules-cards/src/primitives/costs.rs`;
  `SpellEffectKind::ProduceMana` and `ManaAmount` in
  `tricerules/tricerules-cards/src/primitives/effects.rs`.
- **Implementation references:** Loader: `tricerules/tricerules-cards/src/registry.rs#CardRegistry::from_embedded`
  and `tricerules/tricerules-cards/src/registry.rs#CardRegistry::from_chunks_and_tokens`; validator:
  `tricerules/tricerules-cards/src/primitives/abilities.rs#ActivatedAbilityDef::validate_shape`;
  engine consumer: `tricerules/tricerules-core/src/engine/casting.rs#GameEngine::activate_ability`
  and `tricerules/tricerules-core/src/engine/resolution/misc.rs#produce_mana`.
- **Evidence and limits:** Both checked-in semantic scenarios assert one colorless mana, a tapped
  source still on the battlefield, and an empty stack. These assertions do not cover every
  replacement or mana-spending restriction.

## Generator recognition

- **Status:** `Unassessed`.
- **Recipe and input shapes:** Not reviewed for this entry.
- **References and limits:** Runtime scenarios do not establish generator recognition.

## Shipped card evidence

- `mind_stone` — definition: [Mind Stone](../../../data/mind_stone.ron) — review map:
  [map](../../review-maps/mind_stone.json).
- `scavenger_grounds` — definition: [Scavenger Grounds](../../../data/scavenger_grounds.ron) —
  review map: [map](../../review-maps/scavenger_grounds.json).
- **Whole-card readiness:** `Unassessed` in this index pass. Both linked maps record prior complete-definition review; this entry assesses only the mana ability.

## Semantic test coverage

- `scenario deck_coverage_mind_stone::mind_stone_taps_for_colorless_without_using_the_stack` —
  `tricerules/tricerules-core/tests/scenario/deck_coverage_mind_stone.rs#mind_stone_taps_for_colorless_without_using_the_stack` — `Exercised`: adds exactly one colorless mana; Mind Stone taps, stays on the battlefield, and never enters the stack.
- `scenario deck_coverage_scavenger_grounds::scavenger_grounds_produces_one_colorless_mana_without_the_stack` —
  `tricerules/tricerules-core/tests/scenario/deck_coverage_scavenger_grounds.rs#scavenger_grounds_produces_one_colorless_mana_without_the_stack` — `Exercised`: asserts one colorless and zero colored mana, and that the land stays tapped on the battlefield with no stack item.
- **Uncovered behavior:** Mana-spending restrictions and replacement effects.
- **Inapplicable cases:** `N/A` — target selection; neither ability has a target.

## Presentation prerequisites

- Both definitions map their activated ability to an Oracle line. No card-specific choice or target
  prompt is defined. Rust tests do not verify client display or manual play acceptance.

## Review provenance

- **Reviewed revision:** `34bb8f7ccaee20e69ae3d0c90dde65edaf23f76b`.
- **Reviewed paths:**
  - `tricerules/tricerules-cards/data/mind_stone.ron`
  - `tricerules/tricerules-cards/data/scavenger_grounds.ron`
  - `tricerules/tricerules-cards/authoring/review-maps/mind_stone.json`
  - `tricerules/tricerules-cards/authoring/review-maps/scavenger_grounds.json`
  - `tricerules/tricerules-core/tests/scenario/deck_coverage_mind_stone.rs`
  - `tricerules/tricerules-core/tests/scenario/deck_coverage_scavenger_grounds.rs`
  - `tricerules/tricerules-cards/src/primitives/costs.rs`
  - `tricerules/tricerules-cards/src/primitives/effects.rs`
  - `tricerules/tricerules-cards/src/primitives/abilities.rs`
  - `tricerules/tricerules-cards/src/registry.rs`
  - `tricerules/tricerules-core/src/engine/casting.rs`
  - `tricerules/tricerules-core/src/engine/resolution/misc.rs`
- **Review note:** Existing assertions were inspected but not rerun in this documentation phase. No GUI acceptance is claimed.
