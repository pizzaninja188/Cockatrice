# Scavenger Grounds sacrifices a Desert to exile all graveyards

## Identity

- **Pattern ID:** `scavenger-grounds-all-graveyards-exile`
- **Category:** Activated graveyard effect
- **Search terms:** exile all graveyards, sacrifice Desert, self-sacrifice, resolution-time snapshot

## Behavior and limits

- **Behavior:** After paying {2}, tapping Scavenger Grounds, and sacrificing a controlled Desert,
  its ability exiles cards from every player's graveyard when it resolves.
- **Composition and prerequisites:** The selected Desert is a sacrifice cost, not an effect target.
  The sacrificed permanent may be Scavenger Grounds itself.
- **Boundaries:** Tests check rejection of an uncontrolled or non-Desert choice without partial
  payment, and exile of cards present at resolution, including the sacrificed Desert and cards
  added after activation.
- **Near misses:** Targeted graveyard exile, one-player-only exile, or effects that preserve matching
  cards are not covered.

## Runtime support

- **Status:** `Supported` for the tested all-player exile and controlled-Desert cost.
- **Typed symbols:** `AbilityCost::Mana`, `AbilityCost::Tap`, and
  `AbilityCost::SacrificePermanent` in `tricerules/tricerules-cards/src/primitives/costs.rs`;
  `TargetFilter.required_subtypes` and `TargetController::You` in
  `tricerules/tricerules-cards/src/primitives/targeting.rs`; `SpellEffectKind::ExileGraveyards` and
  `RelativePlayerSet::All` in `tricerules/tricerules-cards/src/primitives/effects.rs`.
- **Implementation references:** Loader: `tricerules/tricerules-cards/src/registry.rs#CardRegistry::from_embedded`
  and `tricerules/tricerules-cards/src/registry.rs#CardRegistry::from_chunks_and_tokens`; validator:
  `tricerules/tricerules-cards/src/primitives/abilities.rs#ActivatedAbilityDef::validate_shape`;
  activation cost consumer: `tricerules/tricerules-core/src/engine/casting.rs#GameEngine::activate_ability`;
  effect consumer: `tricerules/tricerules-core/src/engine/resolution/zones.rs#exile_graveyards`.
- **Evidence and limits:** Both a different controlled Desert and the source itself are tested as
  costs. The resolving ability remains after source sacrifice and exiles all listed graveyard
  objects present when the effect resolves.

## Generator recognition

- **Status:** `Unassessed`.
- **Recipe and input shapes:** Not reviewed for this entry.
- **References and limits:** No generator conclusion is drawn from the runtime tests.

## Shipped card evidence

- `scavenger_grounds` — definition: [Scavenger Grounds](../../../data/scavenger_grounds.ron) —
  review map: [map](../../review-maps/scavenger_grounds.json).
- **Whole-card readiness:** `Unassessed` in this index pass. The linked map records prior complete-definition review; this entry assesses only the graveyard-exile activation.

## Semantic test coverage

- `scenario deck_coverage_scavenger_grounds::scavenger_grounds_rejects_non_deserts_and_opponent_deserts_without_partial_payment` —
  `tricerules/tricerules-core/tests/scenario/deck_coverage_scavenger_grounds.rs#scavenger_grounds_rejects_non_deserts_and_opponent_deserts_without_partial_payment` — `Exercised`: neither an Island nor an opponent's Desert spends mana, taps the source, sacrifices a permanent, or creates a stack item.
- `scenario deck_coverage_scavenger_grounds::scavenger_grounds_sacrifices_another_desert_then_exiles_all_graveyards_at_resolution` —
  `tricerules/tricerules-core/tests/scenario/deck_coverage_scavenger_grounds.rs#scavenger_grounds_sacrifices_another_desert_then_exiles_all_graveyards_at_resolution` — `Exercised`: the paid ability exiles both players' earlier and later graveyard cards plus the sacrificed Desert.
- `scenario deck_coverage_scavenger_grounds::scavenger_grounds_can_sacrifice_itself_and_exile_its_graveyard_card_on_resolution` —
  `tricerules/tricerules-core/tests/scenario/deck_coverage_scavenger_grounds.rs#scavenger_grounds_can_sacrifice_itself_and_exile_its_graveyard_card_on_resolution` — `Exercised`: the source may be sacrificed and then is exiled with the graveyard cards when the ability resolves.
- **Uncovered behavior:** Other graveyard filters and target revalidation; this ability has no target.
- **Inapplicable cases:** `N/A` — target selection; the Desert is selected to pay a cost.

## Presentation prerequisites

- The activation maps to its Oracle line and asks the activating player to choose a controlled
  Desert as a cost. The engine fixture does not establish client selection display or manual
  acceptance.

## Review provenance

- **Reviewed revision:** `34bb8f7ccaee20e69ae3d0c90dde65edaf23f76b`.
- **Reviewed paths:**
  - `tricerules/tricerules-cards/data/scavenger_grounds.ron`
  - `tricerules/tricerules-cards/authoring/review-maps/scavenger_grounds.json`
  - `tricerules/tricerules-core/tests/scenario/deck_coverage_scavenger_grounds.rs`
  - `tricerules/tricerules-cards/src/primitives/abilities.rs`
  - `tricerules/tricerules-cards/src/primitives/effects.rs`
  - `tricerules/tricerules-cards/src/primitives/targeting.rs`
  - `tricerules/tricerules-cards/src/registry.rs`
  - `tricerules/tricerules-core/src/engine/casting.rs`
  - `tricerules/tricerules-core/src/engine/resolution/zones.rs`
- **Review note:** Existing assertions were inspected but not rerun in this documentation phase. No GUI acceptance is claimed.
