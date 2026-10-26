# Library search to battlefield

## Identity

- **Pattern ID:** `single-library-search-to-battlefield`
- **Category:** Library search
- **Search terms:** library search, one result, land filter, battlefield tapped, shuffle

## Behavior and limits

- **Behavior:** Existing resultless spell and triggered Program paths can offer one or two cards
  from the controller's library that match an authored filter; selected cards go onto the
  battlefield tapped, then the library is shuffled. A separate bound typed Program profile returns
  a generic `PermanentSelection` for at most one selected permanent, with authored tapped, shuffle,
  and reveal settings independent of the captured identity.
- **Composition and prerequisites:** One `SearchLibrary` effect with a static card filter, a
  battlefield destination whose `tapped` value is true, and `shuffle: true`. The spell scenarios
  observe a zero-to-one choice bound and select one result. Larval Scoutlander's ETB Program
  observes a zero-to-two bound and also covers choosing no result after paying its sacrifice cost.
  The typed resultful profile requires a mandatory Controller search from fixed `Library`, a
  permanent-proving type predicate in every query OR leaf, bounds whose feasible effective maximum
  is one, and a direct `Battlefield` destination. It can bind a narrower query without narrowing the
  generic result filter and can feed `Untap(PreviousEffectObject)` or the existing exact
  `MayBehold` success leaf.
- **Boundaries:** Existing spell scenarios cover a `BasicLand` filter and an OR of four land
  subtypes; the latter admits a nonbasic land carrying one of those subtypes. Larval Scoutlander
  covers a `BasicLand` filter, up to two tapped results, controller-only library selection, an
  eligible land-or-Lander sacrifice, and decline/no-eligible-cost paths that skip the search. A
  privacy fixture holds the nonempty controller Library's size constant across zero, one, and two
  eligible Basics: the 0..2 offer and public up-to-two prompt stay fixed, while ineligible candidates
  remain rejected. The maximum reflects authored capacity (capped by `count`), not the eligible
  candidate count; candidates and submitted selections remain filter-validated. This does not claim
  full library-identity secrecy. A combined resultless-search fixture selects two cards with
  serial entry choices and verifies that the caller resumes after both choices, the remaining
  instruction runs once, and one shuffle and one search-observer event complete the search.
- **Near misses:** These scenarios do not cover split destinations, a hand or library destination,
  untapped entry, or a conditional destination. The shipped-card scenarios do not cover reveal
  requirements. The Larval scenarios reject
  wrong-actor and ineligible-candidate answers, but do not prove full library-identity secrecy or
  shuffle randomness.

## Runtime support

- **Status:** `Supported` for the existing one-result spell searches and two-result
  triggered-Program form described above, plus the typed bound Program result exercised below.
- **Typed symbols:** `SpellEffectKind::SearchLibrary`, `SearchDestination::Battlefield`, and
  `ZoneCardFilter` in `tricerules/tricerules-card-model/src/primitives/effects.rs`.
- **Implementation references:** Loader: `tricerules/tricerules-cards/src/registry.rs#CardRegistry::from_chunks_and_tokens`;
  validator/profile: `tricerules/tricerules-card-model/src/primitives/effects.rs#SpellEffectKind::typed_resultful_battlefield_search_result_type`;
  engine choice producer: `tricerules/tricerules-core/src/engine/resolution/zones.rs#search_library`;
  selection and destination consumer:
  `tricerules/tricerules-core/src/engine/custom_resolution/library_search.rs#finish_library_search`.
- **Evidence and limits:** The Larval privacy scenario holds the controller Library's public size
  constant while varying eligible Basic Lands from zero to two. It checks the same 0..2 offer and
  public up-to-two prompt/log, filter-bound candidates, rejection of nonmatching and opponent-library
  selections without changing the pending offer, tapped results, one shuffle, and one observer. This
  checks bound/prompt capacity only; it does not prove full library-identity secrecy or shuffle
  randomness.
- Test-only typed fixture coverage includes `typed_creature_battlefield_search_binds_exact_revealed_result_with_count_two_and_max_one`,
  which offers two matching Creatures from a nonempty library for a count-two, bounds-zero-to-one search. The private
  offer reveals neither candidate; after one is selected, only that Creature is revealed and enters
  tapped. Its exact post-entry generation remains in the bound result through a parked Behold choice,
  and Untap consumes that identity. The authored search does not shuffle. The paired
  `typed_creature_battlefield_search_no_match_keeps_hidden_library_cards_private_and_runs_tail`
  scenario has no eligible candidates in a nonempty library, reveals nothing, and resumes the tail
  once with an empty result.
- The generic `resultless_two_card_battlefield_search_retains_cursor_across_both_entry_choices`
  fixture is engine regression evidence for serial entry choices and tail resumption, not a separate
  shipped-card semantic claim.

## Authoring path

The reviewed Rampant Growth, Farseek, and Larval Scoutlander definitions are direct RON. Runtime
coverage above is independent of any text-to-card generation route.
## Shipped card evidence

- `rampant_growth` — definition: [Rampant Growth](../../../data/rampant_growth.ron) — review map:
  [map](../../review-maps/rampant_growth.json).
- `farseek` — definition: [Farseek](../../../data/farseek.ron) — review map:
  [map](../../review-maps/farseek.json).
- `larval_scoutlander` — definition: [Larval Scoutlander](../../../data/generated/l/larval_scoutlander.ron) —
  review map: [map](../../review-maps/larval_scoutlander.json).
- **Whole-card readiness:** `Unassessed` in this navigation entry. The linked maps record independent
  complete-definition reviews; capability evidence does not replace applicable delivery gates.

## Semantic test coverage

- `scenario deck_coverage_search_lands::rampant_growth_rejects_a_nonbasic_land_and_puts_a_basic_onto_the_battlefield_tapped` —
  `tricerules/tricerules-core/tests/scenario/deck_coverage_search_lands.rs#rampant_growth_rejects_a_nonbasic_land_and_puts_a_basic_onto_the_battlefield_tapped` — `Exercised`: the offered candidates include a basic land but exclude a nonbasic land; after choosing the basic land, it is on the battlefield tapped, the excluded card remains in the library, and a shuffle log is emitted.
- `scenario deck_coverage_search_lands::farseek_accepts_each_listed_subtype_including_a_nonbasic_land_and_enters_tapped` —
  `tricerules/tricerules-core/tests/scenario/deck_coverage_search_lands.rs#farseek_accepts_each_listed_subtype_including_a_nonbasic_land_and_enters_tapped` — `Exercised`: the offered candidates include each listed subtype and a matching nonbasic land, exclude a land with none of those subtypes, then put the selected card onto the battlefield tapped and emit a shuffle log.
- `scenario issue_423_station_thresholds::issue_423_larval_scoutlander_may_sacrifice_a_land_to_search_two_basics_tapped` —
  `tricerules/tricerules-core/tests/scenario/issue_423_station_thresholds.rs#issue_423_larval_scoutlander_may_sacrifice_a_land_to_search_two_basics_tapped` — `Exercised`: the ETB trigger permits a land sacrifice and puts two selected Basic Lands onto the battlefield tapped.
- `scenario issue_423_station_thresholds::issue_423_larval_scoutlander_may_pay_with_lander_and_searches_only_its_controllers_library` —
  `tricerules/tricerules-core/tests/scenario/issue_423_station_thresholds.rs#issue_423_larval_scoutlander_may_pay_with_lander_and_searches_only_its_controllers_library` — `Exercised`: a Lander may be sacrificed; the controller's search offer excludes an opponent's library card, rejects a wrong-actor answer, selects one result, and emits no explicit reveal event.
- `scenario issue_423_station_thresholds::issue_423_larval_scoutlander_can_choose_no_basic_land_after_sacrificing` —
  `tricerules/tricerules-core/tests/scenario/issue_423_station_thresholds.rs#issue_423_larval_scoutlander_can_choose_no_basic_land_after_sacrificing` — `Exercised`: the cost is paid, zero cards are selected, and the resultless search still shuffles and resumes.
- `scenario issue_423_station_thresholds::issue_423_larval_scoutlander_search_prompt_hides_eligible_basic_count` —
  `tricerules/tricerules-core/tests/scenario/issue_423_station_thresholds.rs#issue_423_larval_scoutlander_search_prompt_hides_eligible_basic_count` — `Exercised`: same-sized nonempty controller Libraries with zero, one, or two eligible Basics retain the 0..2 offer and public up-to-two prompt/log; nonmatching and opponent-library answers are rejected without replacing the pending offer, then selected cards enter tapped and the search emits one shuffle and one observer.
- `scenario issue_423_station_thresholds::issue_423_larval_scoutlander_decline_and_no_eligible_cost_skip_the_search` —
  `tricerules/tricerules-core/tests/scenario/issue_423_station_thresholds.rs#issue_423_larval_scoutlander_decline_and_no_eligible_cost_skip_the_search` — `Exercised`: declining or lacking an eligible sacrifice skips the search branch.
- **Uncovered behavior:** Split destinations, destination alternatives, and shuffle randomness.
- **Inapplicable cases:** `N/A` — the two existing spell definitions do not have trigger behavior; Larval Scoutlander is a triggered Program search.

## Presentation prerequisites

- The spell review maps link the Oracle lines to their typed search effects; Larval Scoutlander's
  map links its ETB, Station, and threshold lines to typed paths. Scenarios inspect the
  `LibrarySearch` choice kind and relevant private-search outcomes, but do not establish client
  rendering or manual play acceptance.

## Review provenance

- **Reviewed revision:** `d4fa33fdba1926f5b0f699bdcdacec6aaf7aec14`.
- **Reviewed paths:**
  - `tricerules/tricerules-cards/data/rampant_growth.ron`
  - `tricerules/tricerules-cards/data/farseek.ron`
  - `tricerules/tricerules-cards/authoring/review-maps/rampant_growth.json`
  - `tricerules/tricerules-cards/authoring/review-maps/farseek.json`
  - `tricerules/tricerules-cards/authoring/review-maps/larval_scoutlander.json`
  - `tricerules/tricerules-cards/data/generated/l/larval_scoutlander.ron`
  - `tricerules/tricerules-core/tests/scenario/deck_coverage_search_lands.rs`
  - `tricerules/tricerules-core/tests/scenario/issue_423_station_thresholds.rs`
  - `tricerules/tricerules-card-model/src/primitives/effects.rs`
  - `tricerules/tricerules-cards/src/registry.rs`
  - `tricerules/tricerules-core/src/engine/resolution/zones.rs`
  - `tricerules/tricerules-core/src/engine/custom_resolution/library_search.rs`
  - `tricerules/tricerules-cards/src/bin/gen_cards/recipes.rs`
- **Review note:** The linked card definitions, maps, and card-specific semantic scenarios were
  inspected at this revision. Generic typed and resultless Program fixtures are engine regression
  evidence, not additional shipped-card semantic claims. No client acceptance is claimed.
