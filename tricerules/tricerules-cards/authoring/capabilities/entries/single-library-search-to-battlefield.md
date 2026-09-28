# Single library search result to battlefield

## Identity

- **Pattern ID:** `single-library-search-to-battlefield`
- **Category:** Library search
- **Search terms:** library search, one result, land filter, battlefield tapped, shuffle

## Behavior and limits

- **Behavior:** A resolving spell offers up to one card from its controller's library that matches
  an authored filter; a selected card goes onto the battlefield tapped, then the library is
  shuffled.
- **Composition and prerequisites:** One `SearchLibrary` effect with a static card filter, a
  battlefield destination whose `tapped` value is true, and `shuffle: true`. The cited scenarios
  observe a zero-to-one choice bound but exercise selecting one result.
- **Boundaries:** The two scenarios cover a `BasicLand` filter and an OR of four land subtypes; the
  latter admits a nonbasic land carrying one of those subtypes. Both select at most one card, put
  it onto the battlefield tapped, and assert the shuffle log.
- **Near misses:** These scenarios do not cover multiple results, split destinations, a hand or
  library destination, untapped entry, reveal requirements, or a conditional destination. They do
  not submit an ineligible choice or test a zero-card choice.

## Runtime support

- **Status:** `Supported` for the one-result, tapped-battlefield behavior and the two tested filter
  shapes described above.
- **Typed symbols:** `SpellEffectKind::SearchLibrary`, `SearchDestination::Battlefield`, and
  `ZoneCardFilter` in `tricerules/tricerules-cards/src/primitives/effects.rs`.
- **Implementation references:** Loader: `tricerules/tricerules-cards/src/registry.rs#CardRegistry::from_chunks_and_tokens`;
  validator: `tricerules/tricerules-cards/src/primitives/effects.rs#SpellEffectKind::validate`;
  engine choice producer: `tricerules/tricerules-core/src/engine/resolution/zones.rs#search_library`;
  selection and destination consumer:
  `tricerules/tricerules-core/src/engine/custom_resolution/library_search.rs#finish_library_search`.
- **Evidence and limits:** The scenarios inspect candidate IDs, choose one legal card, and assert
  the chosen card's battlefield zone and tapped state, an excluded card remaining in the library,
  and a shuffle log. They do not prove shuffle randomness or invalid-choice rejection.

## Generator recognition

- **Status:** `Partial`.
- **Recipe and input shapes:** Recipe `spell.search_library.basic_land.battlefield_tapped` recognizes
  the exact single-basic-land, tapped-battlefield, then-shuffle clause.
- **References and limits:** `tricerules/tricerules-cards/src/bin/gen_cards/recipes.rs#match_spell_search_basic_land_battlefield_tapped`
  implements that exact matcher. This entry's subtype-OR example is outside that recipe's exact
  input shape; other generator routes for that wording were not assessed.

## Shipped card evidence

- `rampant_growth` — definition: [Rampant Growth](../../../data/rampant_growth.ron) — review map:
  [map](../../review-maps/rampant_growth.json).
- `farseek` — definition: [Farseek](../../../data/farseek.ron) — review map:
  [map](../../review-maps/farseek.json).
- **Whole-card readiness:** `Unassessed` for both named cards in this index entry; their linked
  maps record earlier complete-definition reviews.

## Semantic test coverage

- `scenario deck_coverage_search_lands::rampant_growth_rejects_a_nonbasic_land_and_puts_a_basic_onto_the_battlefield_tapped` —
  `tricerules/tricerules-core/tests/scenario/deck_coverage_search_lands.rs#rampant_growth_rejects_a_nonbasic_land_and_puts_a_basic_onto_the_battlefield_tapped` — `Exercised`: the offered candidates include a basic land but exclude a nonbasic land; after choosing the basic land, it is on the battlefield tapped, the excluded card remains in the library, and a shuffle log is emitted.
- `scenario deck_coverage_search_lands::farseek_accepts_each_listed_subtype_including_a_nonbasic_land_and_enters_tapped` —
  `tricerules/tricerules-core/tests/scenario/deck_coverage_search_lands.rs#farseek_accepts_each_listed_subtype_including_a_nonbasic_land_and_enters_tapped` — `Exercised`: the offered candidates include each listed subtype and a matching nonbasic land, exclude a land with none of those subtypes, then put the selected card onto the battlefield tapped and emit a shuffle log.
- **Uncovered behavior:** Multiple selected results, split destinations, destination alternatives, reveal/privacy presentation, ineligible-choice rejection, and shuffle randomness.
- **Inapplicable cases:** `N/A` — trigger behavior; both cited definitions are sorceries with spell effects.

## Presentation prerequisites

- Both review maps link the spell's Oracle line to its typed search effect. The scenarios check the
  `LibrarySearch` choice kind, but do not establish client rendering, privacy presentation, or
  manual play acceptance.

## Review provenance

- **Reviewed revision:** `0ce073fdee9238fb516c5f7d36287d3b8659d076`.
- **Reviewed paths:**
  - `tricerules/tricerules-cards/data/rampant_growth.ron`
  - `tricerules/tricerules-cards/data/farseek.ron`
  - `tricerules/tricerules-cards/authoring/review-maps/rampant_growth.json`
  - `tricerules/tricerules-cards/authoring/review-maps/farseek.json`
  - `tricerules/tricerules-core/tests/scenario/deck_coverage_search_lands.rs`
  - `tricerules/tricerules-cards/src/primitives/effects.rs`
  - `tricerules/tricerules-cards/src/registry.rs`
  - `tricerules/tricerules-core/src/engine/resolution/zones.rs`
  - `tricerules/tricerules-core/src/engine/custom_resolution/library_search.rs`
  - `tricerules/tricerules-cards/src/bin/gen_cards/recipes.rs`
- **Review note:** Source and assertions were inspected, not run. No client acceptance is claimed.
