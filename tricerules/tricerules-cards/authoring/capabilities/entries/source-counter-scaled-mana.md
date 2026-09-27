# Mana scaled by a source counter

## Identity

- **Pattern ID:** `source-counter-scaled-mana`
- **Category:** Mana ability
- **Search terms:** mana per counter, source counter, charge counters, variable mana output, zero-output mana ability

## Behavior and limits

- **Behavior:** A battlefield activated mana ability offers one-mana options. The chosen option is
  multiplied by the number of the named counter on its source when the mana ability is activated.
  At zero counters, the output is still empty and the activation remains a mana ability.
- **Composition and prerequisites:** The effect must be the sole direct effect of an activated
  ability whose source zone is the battlefield and whose only cost is tapping the source. Each
  option must contain exactly one mana unit. The source remains on the battlefield for this
  tap-only activation; CR 605 mana-ability timing applies.
- **Boundaries:** The shipped scenario exercises Astral Cornucopia's five single-colored options,
  X-charge entry replacement, live Charge-counter scaling, and its zero-counter case. At zero
  counters the engine publishes no mana output, while a separate ordered option-label field keeps
  the five color choices available to the client.
- **Near misses:** Mixed or nested effects, non-battlefield sources, costs beyond tapping, and
  triggered or reflexive uses are rejected. This evidence does not establish a complete
  Everflowing Chalice definition: its repeated Multikicker cost and counter receipt remain
  unimplemented. Colorless scaled output is not exercised by the cited card scenario.

## Runtime support

- **Status:** `Supported` for the exact one-mana-per-option, tap-only battlefield activation
  demonstrated by Astral Cornucopia.
- **Typed symbols:** `SpellEffectKind::ProduceManaPerSourceCounter` and `ManaAmount` in
  `tricerules/tricerules-cards/src/primitives/effects.rs`; `ActivatedAbilityDef::mana_options` and
  `ActivatedAbilityDef::mana_source_counter` in
  `tricerules/tricerules-cards/src/primitives/abilities.rs`.
- **Implementation references:** loader:
  `tricerules/tricerules-cards/src/registry.rs#CardRegistry::from_embedded`; validator:
  `tricerules/tricerules-cards/src/primitives/effects.rs#SpellEffectKind::validate` and
  `tricerules/tricerules-cards/src/primitives/abilities.rs#ActivatedAbilityDef::validate_shape`;
  engine option calculation:
  `tricerules/tricerules-core/src/engine/casting.rs#GameEngine::active_mana_options`; immediate
  ability handling: `tricerules/tricerules-core/src/engine/casting.rs#GameEngine::activate_ability`
  and `tricerules/tricerules-core/src/engine/resolution/misc.rs#produce_mana`; public output:
  `tricerules/tricerules-core/src/engine/legal_actions.rs#activated_ability_info`.
- **Evidence and limits:** The card scenarios assert exact mana pools for all five choices at two
  counters, three mana after the live counter count changes to three, and no mana at zero counters.
  The zero-counter case also asserts the source taps and no stack item appears. Primitive tests
  accept the supported shape and reject mixed, nested, triggered, and reflexive occurrences. They
  do not substitute for the per-card scenario or establish colorless scaled output.

## Generator recognition

- **Status:** `Unassessed`.
- **Recipe and input shapes:** Astral Cornucopia is hand-authored as direct RON; no generator
  recipe was assessed for this entry.
- **References and limits:** The shipped definition and semantic scenarios establish runtime
  behavior, not generator recognition.

## Shipped card evidence

- `astral_cornucopia` — definition: [Astral Cornucopia](../../../data/astral_cornucopia.ron) — review map: [map](../../review-maps/astral_cornucopia.json).
  Oracle ID `1bc42024-52da-4d93-8b47-544f0a4a72a1`; one normal Artifact face, cost `{X}{X}{X}`,
  colorless, no power/toughness. It appears in Atraxa's pinned mainboard-plus-commander map;
  Moxfield section data was unavailable, so mainboard placement is inferred from the nonlegendary
  artifact identity.
- Oracle source spans in the review map: face line 1 is the X Charge-counter entry clause; face
  line 2 is the tap, choose-a-color, one mana per Charge counter clause. The [Scryfall printing](https://scryfall.com/card/soc/342/astral-cornucopia)
  and its [rulings](https://api.scryfall.com/cards/15175742-11ae-4819-a5bd-412084b0b686/rulings) were checked; the
  2014-02-01 ruling is relevant to X payment/entry and mana-ability timing.
- **Whole-card readiness:** `Ready` for the authored rules definition and required card-data
  gates. Automated engine, client-contract, and full affected-side checks passed. Manual GUI
  acceptance was not performed and is tracked separately from this readiness result.

## Semantic test coverage

- `scenario deck_coverage_astral_cornucopia::astral_cornucopia_registers_exact_identity_and_typed_abilities` —
  `tricerules/tricerules-core/tests/scenario/deck_coverage_astral_cornucopia.rs#astral_cornucopia_registers_exact_identity_and_typed_abilities` — `Exercised`: exact Oracle card shape, X cost, Charge-counter entry replacement, tap cost, and five one-mana options.
- `scenario deck_coverage_astral_cornucopia::astral_cornucopia_casts_x_zero_one_two_and_enters_with_chosen_charge` —
  `tricerules/tricerules-core/tests/scenario/deck_coverage_astral_cornucopia.rs#astral_cornucopia_casts_x_zero_one_two_and_enters_with_chosen_charge` — `Exercised`: X=0/1/2 pays 0/3/6 generic mana and enters with the corresponding Charge count.
- `scenario deck_coverage_astral_cornucopia::astral_cornucopia_outputs_two_mana_in_each_chosen_color_without_stack` —
  `tricerules/tricerules-core/tests/scenario/deck_coverage_astral_cornucopia.rs#astral_cornucopia_outputs_two_mana_in_each_chosen_color_without_stack` — `Exercised`: each W/U/B/R/G option adds exactly two mana of only that color; the source taps and the stack stays empty.
- `scenario deck_coverage_astral_cornucopia::astral_cornucopia_output_uses_live_charge_counter_count` —
  `tricerules/tricerules-core/tests/scenario/deck_coverage_astral_cornucopia.rs#astral_cornucopia_output_uses_live_charge_counter_count` — `Exercised`: three current Charge counters produce exactly three chosen-color mana.
- `scenario deck_coverage_astral_cornucopia::astral_cornucopia_with_zero_counters_taps_for_no_mana_without_stack` —
  `tricerules/tricerules-core/tests/scenario/deck_coverage_astral_cornucopia.rs#astral_cornucopia_with_zero_counters_taps_for_no_mana_without_stack` — `Exercised`: zero counters leave output empty while the ability accepts the chosen option, taps the source, and adds no stack item.
- `scenario deck_coverage_astral_cornucopia::astral_cornucopia_rejects_invalid_mana_option_without_mutation` —
  `tricerules/tricerules-core/tests/scenario/deck_coverage_astral_cornucopia.rs#astral_cornucopia_rejects_invalid_mana_option_without_mutation` — `Exercised`: an invalid option changes neither the source nor mana, command index, pending resolution, or stack.
- `RuledClientTest.ZeroOutputManaAbilityRetainsItsSelectableOptionsDuringPayment` —
  `tests/ruled_client_tests/ruled_client_test.cpp#ZeroOutputManaAbilityRetainsItsSelectableOptionsDuringPayment` — `Exercised`: five option indices and “Choose” labels survive an empty current output in payment and ordinary action menus.
- **Uncovered behavior:** Colorless source-counter-scaled output, counter types other than Charge, and interaction with additional costs are outside the cited card scenario; additional costs are rejected by the current validator.
- **Inapplicable cases:** `N/A` — targets and target-departure handling; the activated ability is untargeted.

## Presentation prerequisites

- The card definition maps its static and activated abilities to Oracle lines 1 and 2. The engine
  reports actual output through `mana_produced`. At zero output it separately sends ordered
  `mana_option_labels` in `libcockatrice_protocol/libcockatrice/protocol/pb/ruled_v1.proto`; the
  client reads them in `cockatrice/src/game/ruled/ruled_event_dispatcher.cpp` and uses them in
  `cockatrice/src/game/ruled/ruled_pending_cast.cpp` and
  `cockatrice/src/game/ruled/ruled_payment_progression.cpp`. The Qt contract test checks selectable
  indices and labels. Manual GUI acceptance remains unperformed.

## Review provenance

- **Reviewed revision:** `944c1aab61457df53c4282128036abcd4d21db1b`.
- **Reviewed paths:**
  - `tricerules/tricerules-cards/data/astral_cornucopia.ron`
  - `tricerules/tricerules-cards/authoring/review-maps/astral_cornucopia.json`
  - `tricerules/tricerules-cards/src/primitives/abilities.rs`
  - `tricerules/tricerules-cards/src/primitives/effects.rs`
  - `tricerules/tricerules-cards/src/primitives/presentation.rs`
  - `tricerules/tricerules-cards/src/primitives/tests.rs`
  - `tricerules/tricerules-cards/src/registry.rs`
  - `tricerules/tricerules-core/src/engine/casting.rs`
  - `tricerules/tricerules-core/src/engine/legal_actions.rs`
  - `tricerules/tricerules-core/src/engine/resolution/misc.rs`
  - `tricerules/tricerules-core/src/engine/resolution/mod.rs`
  - `tricerules/tricerules-core/tests/scenario/deck_coverage_astral_cornucopia.rs`
  - `tricerules/tricerules-core/tests/scenario.rs`
  - `tricerules/tricerules-core/tests/conformance/baseline.tsv`
  - `libcockatrice_protocol/libcockatrice/protocol/pb/ruled_v1.proto`
  - `cockatrice/src/game/ruled/ruled_client_state.h`
  - `cockatrice/src/game/ruled/ruled_diagnostic_state.cpp`
  - `cockatrice/src/game/ruled/ruled_event_dispatcher.cpp`
  - `cockatrice/src/game/ruled/ruled_payment_progression.cpp`
  - `cockatrice/src/game/ruled/ruled_payment_ui.cpp`
  - `cockatrice/src/game/ruled/ruled_pending_cast.cpp`
  - `tests/ruled_client_tests/ruled_client_test.cpp`
- **Review note:** Sol independently approved the frozen patch. The current official rules version
  is September 25, 2026: CR 107.3m, 122.6, 605.1a, 605.2, and 605.3b govern X entry counters,
  mana-ability classification at zero output, and immediate resolution. See the [official rules
  PDF](https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.pdf). Everflowing Chalice
  shares the counter-scaled output idea but is not a delivered card or demonstrated colorless case.
