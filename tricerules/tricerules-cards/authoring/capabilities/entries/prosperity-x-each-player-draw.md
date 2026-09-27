# Prosperity makes each player draw X

## Identity

- **Pattern ID:** `prosperity-x-each-player-draw`
- **Category:** Group draw
- **Search terms:** X draw, each player, multiplayer, APNAP, Prosperity

## Behavior and limits

- **Behavior:** Prosperity's controller announces and pays X as part of casting; on resolution,
  each player draws that many cards in the tested APNAP order.
- **Composition and prerequisites:** The spell uses `{X}{U}` and `Draw(EachPlayer, X)`. The variable
  is chosen before resolution and is fixed on the stack item.
- **Boundaries:** Tests cover X=2 with three players, and X=0 with two players. The zero case still
  pays the blue mana and draws no cards.
- **Near misses:** This does not cover an untargeted fixed-count draw for opponents, optional draws,
  or a triggered group draw.

## Runtime support

- **Status:** `Supported` for the tested X values and player sets in the cited scenarios.
- **Typed symbols:** `SpellEffectKind::Draw`, `PlayerRecipient::EachPlayer`, and `Amount::X` in
  `tricerules/tricerules-cards/src/primitives/effects.rs`.
- **Implementation references:** Loader: `tricerules/tricerules-cards/src/registry.rs#CardRegistry::from_embedded`
  and `tricerules/tricerules-cards/src/registry.rs#CardRegistry::from_chunks_and_tokens`; validator:
  `tricerules/tricerules-cards/src/primitives/effects.rs#SpellEffectKind::validate_list`;
  cast/choice consumer: `tricerules/tricerules-core/src/engine/casting.rs#GameEngine::cast_spell`;
  draw consumer: `tricerules/tricerules-core/src/engine/resolution/zones.rs#draw` and
  `tricerules/tricerules-core/src/engine/resolution/mod.rs#SpellEffectKind::Draw`.
- **Evidence and limits:** The X=2 test checks exact per-player counts and APNAP draw logs for
  players 0, 1, and 2. The X=0 test checks the blue payment and unchanged library sizes.

## Generator recognition

- **Status:** `Unassessed`.
- **Recipe and input shapes:** Not reviewed for this entry.
- **References and limits:** Engine support does not prove generator recognition for this spelling.

## Shipped card evidence

- `prosperity` — definition: [Prosperity](../../../data/prosperity.ron) — review map:
  [map](../../review-maps/prosperity.json).
- **Whole-card readiness:** `Unassessed` in this index pass. The linked map records prior complete-definition review; this entry assesses only X group draw.

## Semantic test coverage

- `scenario deck_coverage_prosperity::prosperity_pays_x_plus_blue_and_each_player_draws_x_in_apnap_order` —
  `tricerules/tricerules-core/tests/scenario/deck_coverage_prosperity.rs#prosperity_pays_x_plus_blue_and_each_player_draws_x_in_apnap_order` — `Exercised`: with three players and X=2, each draws two, and the log orders the batches as P0, P1, P2.
- `scenario deck_coverage_prosperity::prosperity_with_x_zero_draws_no_cards_but_still_pays_blue` —
  `tricerules/tricerules-core/tests/scenario/deck_coverage_prosperity.rs#prosperity_with_x_zero_draws_no_cards_but_still_pays_blue` — `Exercised`: X=0 is accepted, blue is spent, and neither library changes.
- **Uncovered behavior:** Other announced X values and draw-replacement interactions.
- **Inapplicable cases:** `N/A` — target selection; the spell has no target.

## Presentation prerequisites

- The spell uses its Oracle-line mapping and requires a client cast flow that submits X and mana.
  These Rust tests do not verify the client input flow or manual acceptance.

## Review provenance

- **Reviewed revision:** `34bb8f7ccaee20e69ae3d0c90dde65edaf23f76b`.
- **Reviewed paths:**
  - `tricerules/tricerules-cards/data/prosperity.ron`
  - `tricerules/tricerules-cards/authoring/review-maps/prosperity.json`
  - `tricerules/tricerules-core/tests/scenario/deck_coverage_prosperity.rs`
  - `tricerules/tricerules-cards/src/primitives/effects.rs`
  - `tricerules/tricerules-cards/src/registry.rs`
  - `tricerules/tricerules-core/src/engine/casting.rs`
  - `tricerules/tricerules-core/src/engine/resolution/zones.rs`
  - `tricerules/tricerules-core/src/engine/resolution/mod.rs`
- **Review note:** Existing assertions were inspected but not rerun in this documentation phase. No GUI acceptance is claimed.
