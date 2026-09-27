# Artifact sacrificed to draw one card

## Identity

- **Pattern ID:** `artifact-sacrifice-draw`
- **Category:** Activated ability
- **Search terms:** sacrifice artifact, pay mana and tap, draw one, Mind Stone

## Behavior and limits

- **Behavior:** Mind Stone's controller pays {1}, taps it, and sacrifices it as activation costs;
  one card is drawn when the resulting ability resolves.
- **Composition and prerequisites:** `Mana("{1}")`, `Tap`, and `SacrificeSelf` are all costs of the
  same activated ability. Drawing is a later effect.
- **Boundaries:** The cited test checks that missing mana rejects activation without partial payment,
  and that the card reaches the graveyard before the draw resolves.
- **Near misses:** Sacrificing another artifact, drawing multiple cards, or drawing before paying
  costs is not covered.

## Runtime support

- **Status:** `Supported` for Mind Stone's one-card draw after its three activation costs.
- **Typed symbols:** `AbilityCost::Mana`, `AbilityCost::Tap`, and `AbilityCost::SacrificeSelf` in
  `tricerules/tricerules-cards/src/primitives/costs.rs`; `SpellEffectKind::Draw` in
  `tricerules/tricerules-cards/src/primitives/effects.rs`.
- **Implementation references:** Loader: `tricerules/tricerules-cards/src/registry.rs#CardRegistry::from_embedded`
  and `tricerules/tricerules-cards/src/registry.rs#CardRegistry::from_chunks_and_tokens`; validator:
  `tricerules/tricerules-cards/src/primitives/abilities.rs#ActivatedAbilityDef::validate_shape`;
  activation costs: `tricerules/tricerules-core/src/engine/casting.rs#GameEngine::activate_ability`;
  draw consumer: `tricerules/tricerules-core/src/engine/resolution/zones.rs#draw`.
- **Evidence and limits:** The test captures the top card and both zone sizes, confirms a failed
  unpaid activation changes nothing, then checks the paid cost, graveyard source, exact drawn card,
  and completed resolution.

## Generator recognition

- **Status:** `Unassessed`.
- **Recipe and input shapes:** Not reviewed for this entry.
- **References and limits:** The runtime fixture does not establish generator recognition.

## Shipped card evidence

- `mind_stone` — definition: [Mind Stone](../../../data/mind_stone.ron) — review map:
  [map](../../review-maps/mind_stone.json).
- **Whole-card readiness:** `Unassessed` in this index pass. The linked map records prior complete-definition review; this entry assesses only this activation.

## Semantic test coverage

- `scenario deck_coverage_mind_stone::mind_stone_pays_its_cost_before_drawing_one_card` —
  `tricerules/tricerules-core/tests/scenario/deck_coverage_mind_stone.rs#mind_stone_pays_its_cost_before_drawing_one_card` — `Exercised`: missing mana rejects without state change; after payment, Mind Stone is in the graveyard before the ability resolves and exactly the captured top card enters hand.
- **Uncovered behavior:** Other sacrifice-cost sources and replacement effects that alter movement.
- **Inapplicable cases:** `N/A` — target selection; the ability has no target.

## Presentation prerequisites

- The activated ability uses an Oracle-line mapping and has no target or choice prompt. This Rust
  test does not establish client display or manual play acceptance.

## Review provenance

- **Reviewed revision:** `34bb8f7ccaee20e69ae3d0c90dde65edaf23f76b`.
- **Reviewed paths:**
  - `tricerules/tricerules-cards/data/mind_stone.ron`
  - `tricerules/tricerules-cards/authoring/review-maps/mind_stone.json`
  - `tricerules/tricerules-core/tests/scenario/deck_coverage_mind_stone.rs`
  - `tricerules/tricerules-cards/src/primitives/costs.rs`
  - `tricerules/tricerules-cards/src/primitives/effects.rs`
  - `tricerules/tricerules-cards/src/primitives/abilities.rs`
  - `tricerules/tricerules-cards/src/registry.rs`
  - `tricerules/tricerules-core/src/engine/casting.rs`
  - `tricerules/tricerules-core/src/engine/resolution/zones.rs`
- **Review note:** Existing assertions were inspected but not rerun in this documentation phase. No GUI acceptance is claimed.
