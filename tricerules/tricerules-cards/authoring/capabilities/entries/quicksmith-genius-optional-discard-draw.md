# Quicksmith Genius optional discard then draw

## Identity

- **Pattern ID:** `quicksmith-genius-optional-discard-draw`
- **Category:** Triggered choice
- **Search terms:** controlled artifact enters, optional discard, discard then draw, rummage

## Behavior and limits

- **Behavior:** When an artifact enters under Quicksmith Genius's controller, that player may
  discard one card; if they do, they draw one card.
- **Composition and prerequisites:** The checked RON uses an artifact-entry trigger followed by an
  optional one-card `DiscardThenDraw` effect. It is discard-before-draw, not draw-before-discard.
- **Boundaries:** The cited scenario resolves one Sol Ring entry event, exercises choosing a card to
  discard, and separately exercises declining the optional discard.
- **Near misses:** Repeated artifact entries, opponent-controlled artifacts, and a draw-first loot
  sequence are outside the cited semantic coverage.

## Runtime support

- **Status:** `Supported` for one controlled artifact-entry event and the optional discard/draw
  choice shown in the cited fixture.
- **Typed symbols:** `TriggerCondition::WheneverPermanentEntersBattlefield` in
  `tricerules/tricerules-cards/src/primitives/abilities.rs`; `SpellEffectKind::DrawDiscard` and
  `DrawDiscardOrder::DiscardThenDraw` in
  `tricerules/tricerules-cards/src/primitives/effects.rs`.
- **Implementation references:** Loader: `tricerules/tricerules-cards/src/registry.rs#CardRegistry::from_embedded`
  and `tricerules/tricerules-cards/src/registry.rs#CardRegistry::from_chunks_and_tokens`; validator:
  `tricerules/tricerules-cards/src/primitives/abilities.rs#TriggeredAbilityDef::validate_shape`
  and `tricerules/tricerules-cards/src/primitives/effects.rs#SpellEffectKind::validate_list`;
  trigger collector: `tricerules/tricerules-core/src/engine/triggers.rs#collect_event_triggers`;
  choice/effect consumer: `tricerules/tricerules-core/src/engine/resolution/zones.rs#draw_discard`.
- **Evidence and limits:** The selected-card case checks the controller's hand-card choice, the selected
  card entering the graveyard, the next card entering hand, and completed resolution. The decline
  case checks unchanged hand and library. Neither scenario tests recipient redaction or proves that
  another player cannot see the choice data.

## Generator recognition

- **Status:** `Unassessed`.
- **Recipe and input shapes:** Not reviewed for this entry.
- **References and limits:** These engine scenarios do not establish generator recognition.

## Shipped card evidence

- `quicksmith_genius` — definition: [Quicksmith Genius](../../../data/quicksmith_genius.ron) —
  review map: [map](../../review-maps/quicksmith_genius.json).
- **Whole-card readiness:** `Unassessed` in this index pass. The linked map records prior complete-definition review; this entry assesses the cited trigger/choice only.

## Semantic test coverage

- `scenario deck_coverage_quicksmith_genius::quicksmith_genius_loots_once_for_each_controlled_artifact_entry` —
  `tricerules/tricerules-core/tests/scenario/deck_coverage_quicksmith_genius.rs#quicksmith_genius_loots_once_for_each_controlled_artifact_entry` — `Exercised`: one Sol Ring entry, a controller hand-card choice with bounds 0-to-1, then one draw after selecting a discard. The test name's “loots” wording does not change the discard-first order in the definition.
- `scenario deck_coverage_quicksmith_genius::quicksmith_genius_may_decline_the_optional_discard` —
  `tricerules/tricerules-core/tests/scenario/deck_coverage_quicksmith_genius.rs#quicksmith_genius_may_decline_the_optional_discard` — `Exercised`: declining leaves the held card and library unchanged.
- **Uncovered behavior:** Multiple artifacts entering before resolution, opponent artifacts entering,
  and redaction/no-leak behavior for other players observing the hand-card choice.
- **Inapplicable cases:** `N/A` — target selection; the trigger has no target.

## Presentation prerequisites

- The Oracle line maps the trigger; resolving it requires an optional hand-card choice presented to
  its deciding player. The fixture does not prove recipient redaction/no-leak behavior; manual
  client acceptance was not performed.

## Review provenance

- **Reviewed revision:** `34bb8f7ccaee20e69ae3d0c90dde65edaf23f76b`.
- **Reviewed paths:**
  - `tricerules/tricerules-cards/data/quicksmith_genius.ron`
  - `tricerules/tricerules-cards/authoring/review-maps/quicksmith_genius.json`
  - `tricerules/tricerules-core/tests/scenario/deck_coverage_quicksmith_genius.rs`
  - `tricerules/tricerules-cards/src/primitives/abilities.rs`
  - `tricerules/tricerules-cards/src/primitives/effects.rs`
  - `tricerules/tricerules-cards/src/registry.rs`
  - `tricerules/tricerules-core/src/engine/triggers.rs`
  - `tricerules/tricerules-core/src/engine/resolution/zones.rs`
- **Review note:** Existing assertions were inspected but not rerun in this documentation phase. No GUI acceptance is claimed.
