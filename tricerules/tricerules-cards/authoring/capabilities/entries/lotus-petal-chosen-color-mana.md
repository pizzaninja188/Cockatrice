# Lotus Petal sacrifices for one chosen color

## Identity

- **Pattern ID:** `lotus-petal-chosen-color-mana`
- **Category:** Mana ability
- **Search terms:** Lotus Petal, sacrifice for mana, choose color, zero-cost artifact

## Behavior and limits

- **Behavior:** Tapping and sacrificing Lotus Petal adds exactly one mana of one chosen color from
  white, blue, black, red, or green. The tested ability resolves without using the stack.
- **Composition and prerequisites:** The ability has both tap and self-sacrifice costs; the color
  selection is the `ProduceMana` option index.
- **Boundaries:** The valid set is those five colors. An out-of-range choice is rejected before any
  cost is paid.
- **Near misses:** Colorless mana, more than one mana, retaining the source, or a restricted mana
  ability is not covered.

## Runtime support

- **Status:** `Supported` for Lotus Petal's five one-mana color choices.
- **Typed symbols:** `AbilityCost::Tap` and `AbilityCost::SacrificeSelf` in
  `tricerules/tricerules-cards/src/primitives/costs.rs`; `SpellEffectKind::ProduceMana` and
  `ManaAmount` in `tricerules/tricerules-cards/src/primitives/effects.rs`.
- **Implementation references:** Loader: `tricerules/tricerules-cards/src/registry.rs#CardRegistry::from_embedded`
  and `tricerules/tricerules-cards/src/registry.rs#CardRegistry::from_chunks_and_tokens`; validator:
  `tricerules/tricerules-cards/src/primitives/abilities.rs#ActivatedAbilityDef::validate_shape`;
  engine consumer: `tricerules/tricerules-core/src/engine/casting.rs#GameEngine::activate_ability`
  and `tricerules/tricerules-core/src/engine/resolution/misc.rs#produce_mana`.
- **Evidence and limits:** The semantic fixture iterates over all five options and asserts the
  resulting one-color pool, source in its owner's graveyard, and no stack item. A sixth option
  fails with the source untapped, on the battlefield, and no mana added.

## Generator recognition

- **Status:** `Unassessed`.
- **Recipe and input shapes:** Not reviewed for this entry.
- **References and limits:** The runtime fixture does not establish generator recognition.

## Shipped card evidence

- `lotus_petal` — definition: [Lotus Petal](../../../data/lotus_petal.ron) — review map:
  [map](../../review-maps/lotus_petal.json).
- **Whole-card readiness:** `Unassessed` in this index pass. The linked map records prior complete-definition review; this entry assesses only the mana ability.

## Semantic test coverage

- `scenario deck_coverage_lotus_petal::lotus_petal_sacrifices_and_adds_the_chosen_color_without_the_stack` —
  `tricerules/tricerules-core/tests/scenario/deck_coverage_lotus_petal.rs#lotus_petal_sacrifices_and_adds_the_chosen_color_without_the_stack` — `Exercised`: all five mana choices, source sacrifice, and immediate resolution.
- `scenario deck_coverage_lotus_petal::lotus_petal_rejects_an_out_of_range_color_without_paying_its_costs` —
  `tricerules/tricerules-core/tests/scenario/deck_coverage_lotus_petal.rs#lotus_petal_rejects_an_out_of_range_color_without_paying_its_costs` — `Exercised`: choice 5 is rejected without changing the command index, mana, tap state, or zone.
- **Uncovered behavior:** Interactions with mana replacement or spending restrictions.
- **Inapplicable cases:** `N/A` — target selection; the ability has no target.

## Presentation prerequisites

- The Oracle line maps to this activation. The engine test proves the five legal options, not how the
  client displays or collects the color choice; manual client acceptance was not performed.

## Review provenance

- **Reviewed revision:** `34bb8f7ccaee20e69ae3d0c90dde65edaf23f76b`.
- **Reviewed paths:**
  - `tricerules/tricerules-cards/data/lotus_petal.ron`
  - `tricerules/tricerules-cards/authoring/review-maps/lotus_petal.json`
  - `tricerules/tricerules-core/tests/scenario/deck_coverage_lotus_petal.rs`
  - `tricerules/tricerules-cards/src/primitives/costs.rs`
  - `tricerules/tricerules-cards/src/primitives/effects.rs`
  - `tricerules/tricerules-cards/src/primitives/abilities.rs`
  - `tricerules/tricerules-cards/src/registry.rs`
  - `tricerules/tricerules-core/src/engine/casting.rs`
  - `tricerules/tricerules-core/src/engine/resolution/misc.rs`
- **Review note:** Existing assertions were inspected but not rerun in this documentation phase. No GUI acceptance is claimed.
