# Prized Statue creates Treasure on entry and battlefield-to-graveyard

## Identity

- **Pattern ID:** `prized-statue-event-treasure`
- **Category:** Triggered token creation
- **Search terms:** Treasure token, enters battlefield, battlefield-to-graveyard, dies trigger

## Behavior and limits

- **Behavior:** Prized Statue creates one Treasure when it enters the battlefield and another when
  it is put into a graveyard from the battlefield.
- **Composition and prerequisites:** These are two distinct triggered abilities, each creating one
  Treasure for the Statue's controller.
- **Boundaries:** The scenario casts the artifact, resolves the entry trigger, destroys it, and
  resolves the death trigger. It checks the resulting token count and controller.
- **Near misses:** The fixture does not activate a Treasure's mana ability or cover other ways the
  Statue could change zones.

## Runtime support

- **Status:** `Supported` for the one-Treasure entry and dies triggers.
- **Typed symbols:** `TriggerCondition::WhenSelfEntersBattlefield` and the implementation variant
  `TriggerCondition::WhenSelfDies` (the battlefield-to-graveyard event) in
  `tricerules/tricerules-cards/src/primitives/abilities.rs`;
  `SpellEffectKind::CreateTokens` in `tricerules/tricerules-cards/src/primitives/effects.rs`.
- **Implementation references:** Loader and token validation:
  `tricerules/tricerules-cards/src/registry.rs#CardRegistry::from_chunks_and_tokens`; trigger
  collector: `tricerules/tricerules-core/src/engine/triggers.rs#collect_event_triggers`; token
  consumer: `tricerules/tricerules-core/src/engine/resolution/tokens.rs#create_tokens`.
- **Evidence and limits:** The scenario asserts the Statue is on the battlefield before removal,
  then in the graveyard with two Treasures controlled by player 0 and none by player 1.

## Generator recognition

- **Status:** `Unassessed`.
- **Recipe and input shapes:** Not reviewed for this entry.
- **References and limits:** Runtime coverage does not establish a generator route.

## Shipped card evidence

- `prized_statue` — definition: [Prized Statue](../../../data/prized_statue.ron) — review map:
  [map](../../review-maps/prized_statue.json).
- Token definition: [Treasure](../../../data/tokens/treasure.ron); the review map records the token
  identity used by both effects.
- **Whole-card readiness:** `Unassessed` in this index pass. The linked map records prior complete-definition review; this entry assesses only the two token triggers.

## Semantic test coverage

- `scenario deck_coverage_prized_statue::prized_statue_creates_a_treasure_on_entry_and_when_put_into_a_graveyard` —
  `tricerules/tricerules-core/tests/scenario/deck_coverage_prized_statue.rs#prized_statue_creates_a_treasure_on_entry_and_when_put_into_a_graveyard` — `Exercised`: one controller Treasure after entry, two after the Statue reaches its graveyard, none for the opponent.
- **Uncovered behavior:** Treasure activation, including its mana choices and sacrifice.
- **Inapplicable cases:** `N/A` — target and choice selection; neither trigger has either.

## Presentation prerequisites

- The ability maps to one Oracle line and depends on the Treasure token definition. No client token
  display or manual acceptance is proved by the Rust scenario.

## Review provenance

- **Reviewed revision:** `34bb8f7ccaee20e69ae3d0c90dde65edaf23f76b`.
- **Reviewed paths:**
  - `tricerules/tricerules-cards/data/prized_statue.ron`
  - `tricerules/tricerules-cards/data/tokens/treasure.ron`
  - `tricerules/tricerules-cards/authoring/review-maps/prized_statue.json`
  - `tricerules/tricerules-core/tests/scenario/deck_coverage_prized_statue.rs`
  - `tricerules/tricerules-cards/src/primitives/abilities.rs`
  - `tricerules/tricerules-cards/src/primitives/effects.rs`
  - `tricerules/tricerules-cards/src/registry.rs`
  - `tricerules/tricerules-core/src/engine/triggers.rs`
  - `tricerules/tricerules-core/src/engine/resolution/tokens.rs`
- **Review note:** Existing assertions were inspected but not rerun in this documentation phase. No GUI acceptance is claimed.
