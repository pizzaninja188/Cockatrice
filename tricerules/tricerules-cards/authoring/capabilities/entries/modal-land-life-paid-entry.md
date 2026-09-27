# Modal land face with life-paid untapped entry

## Identity

- **Pattern ID:** `modal-land-life-paid-entry`
- **Category:** Battlefield-entry replacement
- **Search terms:** `land enters tapped`, `pay life`, `modal double-faced land`, `entry cost`

## Behavior and limits

- **Behavior:** When one of these land faces is played, its controller may pay 3 life as it enters. Paying leaves it untapped; declining or being unable to pay makes it enter tapped. Its separate tap ability produces exactly the color printed on that land face.
- **Composition and prerequisites:** The modal double-faced card's back face is selected as a land, then its self-affecting `EntersTapped` replacement offers an `EntryCost::PayLife`. The cost is handled during the proposed entry event, before the permanent is on the battlefield.
- **Boundaries:** This claim covers the exact three-life, unconditional, self-entry form on the two named MH3 back faces. The scenarios check payment from 20 life to 17, decline, inability to pay from 2 life, and the distinct red or green mana output.
- **Near misses:** The generator's exact two-life shockland clause is a nearby but different input shape. Unconditional tapped entry has no payment branch. Conditional land entry and a battlefield source that changes other permanents' entries are outside this entry.

## Runtime support

- **Status:** `Supported` for the exact self-entry, three-life form above.
- **Typed symbols:** `StaticAbilityDef::EntersTapped`, `EntersTappedAffected::Self_`, and `EntryCost::PayLife { amount: 3 }` in `tricerules/tricerules-cards/src/primitives/abilities.rs`; the authored face also uses `SpellEffectKind::ProduceMana` and `AbilityCost::Tap`.
- **Implementation references:** embedded loader: `tricerules/tricerules-cards/src/registry.rs#CardRegistry::from_embedded`; typed entry-cost lookup: `tricerules/tricerules-core/src/engine/replacement.rs#GameEngine::entry_unless_cost`; choice creation and payment eligibility: `tricerules/tricerules-core/src/engine/replacement.rs#GameEngine::park_entry_cost_choice`; validated payment, life loss, and replacement completion: `tricerules/tricerules-core/src/engine/replacement.rs#GameEngine::finish_entry_cost_choice`.
- **Evidence and limits:** The engine recognizes the self-entry replacement on the proposed event, asks the destination controller, marks the payment branch unavailable when life is insufficient, records an accepted life payment, and applies the tapped replacement on decline. The shared land scenario asserts both faces' exact life totals, battlefield/tapped state, and mana colors. Other life amounts, additional entry replacements, and interactions with global or conditional tapped-entry effects are not established by these card scenarios.

## Generator recognition

- **Status:** `Not recognized`.
- **Recipe and input shapes:** `static.enters_tapped.unless_pay_life_2` recognizes only the exact two-life shockland wording.
- **References and limits:** `tricerules/tricerules-cards/src/bin/gen_cards/recipes.rs#match_shockland_entry_payment` emits `EntryCost::PayLife { amount: 2 }` for that exact string. Its negative calibration rejects the otherwise matching three-life wording used here. Both delivered definitions therefore use reviewed direct RON; this generator limitation is not a runtime-support gap.

## Shipped card evidence

- `bridgeworks_battle_tanglespan_bridgeworks` — definition: [Bridgeworks Battle // Tanglespan Bridgeworks](../../../data/bridgeworks_battle_tanglespan_bridgeworks.ron) — review map: [map](../../review-maps/bridgeworks_battle_tanglespan_bridgeworks.json). Oracle ID `9d581188-ce80-494e-bd38-f411e1f4efb5`; Scryfall printing [MH3 #249](https://scryfall.com/card/mh3/249/bridgeworks-battle-tanglespan-bridgeworks), [record](https://api.scryfall.com/cards/ebef3db0-2b58-4581-a79c-fbca9a059e63), and [rulings](https://api.scryfall.com/cards/ebef3db0-2b58-4581-a79c-fbca9a059e63/rulings) were checked on 2026-09-27. Front: `{2}{G}`, Sorcery, green, no power/toughness; its source text is mapped at face line 1. Back: no mana cost, Land, colorless, no power/toughness; line 1 is the life-payment entry clause and line 2 is `{T}: Add {G}.` The relevant ruling confirms the optional fight's target legality is separate from the still-legal pump.
- `sundering_eruption_volcanic_fissure` — definition: [Sundering Eruption // Volcanic Fissure](../../../data/sundering_eruption_volcanic_fissure.ron) — review map: [map](../../review-maps/sundering_eruption_volcanic_fissure.json). Oracle ID `c95309e9-5c2f-4518-b2fd-825d3d0a4ae0`; Scryfall printing [MH3 #248](https://scryfall.com/card/mh3/248/sundering-eruption-volcanic-fissure), [record](https://api.scryfall.com/cards/50686ac7-346c-43d1-bdaa-28d46a12ad93), and [rulings](https://api.scryfall.com/cards/50686ac7-346c-43d1-bdaa-28d46a12ad93/rulings) were checked on 2026-09-27. Front: `{2}{R}`, Sorcery, red, no power/toughness; the three ordered effects are mapped to its complete source text at face line 1. Back: no mana cost, Land, colorless, no power/toughness; line 1 is the life-payment entry clause and line 2 is `{T}: Add {R}.` Relevant rulings confirm an illegal sole target prevents the search and restriction, an indestructible target still permits the target controller's optional search, and the no-flying restriction is evaluated against creatures' current characteristics.
- **Pinned deck evidence:** Both Oracle identities occur as `face` entries with their exact joined names in the pinned four-deck map `build/deck-coverage/oracle-map.tsv` (rows 32 and 250; SHA-256 `1162a2f4aaa535e26d052fc72128ae5dad4bbb5e050c409766171918b0b10c55`), under the Bello deck. Moxfield's per-section API returned HTTP 403 in the pinned capture, so `mainboard` placement is inferred from each card being a nonlegendary modal double-faced card that cannot be the commander. No sideboard or other section is admitted.
- **Whole-card readiness:** `Ready` for `bridgeworks_battle_tanglespan_bridgeworks` and `Ready` for `sundering_eruption_volcanic_fissure` across both faces, their spell clauses, land entry and mana abilities, metadata, and required Rust/CardData gates. Manual GUI acceptance was not performed and is not claimed.

## Semantic test coverage

- `scenario deck_coverage_mh3_mdfcs::both_mh3_land_faces_offer_three_life_and_produce_their_exact_color` — `tricerules/tricerules-core/tests/scenario/deck_coverage_mh3_mdfcs.rs#both_mh3_land_faces_offer_three_life_and_produce_their_exact_color` — `Exercised`: for each card, accepts a selectable payment at 20 life and asserts 17 life plus an untapped battlefield land; declines at 20 and asserts a tapped land with no life loss; at 2 life rejects payment and asserts decline still enters tapped; then checks the exact single printed mana color and zero in every other color.
- `deck_coverage_mh3_mdfcs_registry::pilot3_registers_both_complete_mdfc_identities_and_land_faces` — `tricerules/tricerules-cards/tests/deck_coverage_mh3_mdfcs_registry.rs#pilot3_registers_both_complete_mdfc_identities_and_land_faces` — `Exercised`: both whole-card identities have two modal faces; both land backs have the exact three-life self-entry replacement, tap cost, and their single printed mana option.
- **Uncovered behavior:** Interactions of this entry cost with another simultaneous entry replacement, life totals outside the tested payable/unpayable boundaries, conditional or global enters-tapped effects, and exact presentation in a live GUI are outside the cited tests.
- **Inapplicable cases:** `N/A` — targets, target departure, hidden library choices, and protocol/client rules decisions for the shared entry behavior. The front-face interactions are covered separately in each card's review map and scenario tests.

## Presentation prerequisites

The land-face abilities map the entry replacement to Oracle line 1 and the mana ability to line 2. The engine creates a face-named payment prompt and a resolution choice; the same engine scenario checks that the choice is offered to the entering permanent's controller. No new protocol or client consumer was added. The automated evidence does not claim visible GUI acceptance.

## Review provenance

- **Reviewed revision:** `28b399c0db5ce664fb21ab1bf73c945f2ef4e665`.
- **Reviewed paths:**
  - `tricerules/tricerules-cards/src/primitives/abilities.rs`
  - `tricerules/tricerules-cards/src/registry.rs`
  - `tricerules/tricerules-core/src/engine/replacement.rs`
  - `tricerules/tricerules-cards/src/bin/gen_cards/recipes.rs`
  - `tricerules/tricerules-cards/data/bridgeworks_battle_tanglespan_bridgeworks.ron`
  - `tricerules/tricerules-cards/data/sundering_eruption_volcanic_fissure.ron`
  - `tricerules/tricerules-cards/authoring/review-maps/bridgeworks_battle_tanglespan_bridgeworks.json`
  - `tricerules/tricerules-cards/authoring/review-maps/sundering_eruption_volcanic_fissure.json`
  - `tricerules/tricerules-cards/tests/deck_coverage_mh3_mdfcs_registry.rs`
  - `tricerules/tricerules-core/tests/scenario/deck_coverage_mh3_mdfcs.rs`
  - `tricerules/tricerules-core/tests/scenario.rs`
  - `tricerules/tricerules-core/tests/conformance/baseline.tsv`
  - `tricerules/tricerules-cards/presentation/oracle_fingerprints.tsv`
  - `tricerules/CARDS.md`
- **Review note:** Sol independently approved the frozen authored definitions and semantic fixtures. The focused scenario distinguishes pay, decline, unaffordability, and exact color output. The full Rust/CardData gate passed at the reviewed code revision. Source and rulings snapshots are dated 2026-09-27; the current official Comprehensive Rules PDF is the September 25, 2026 edition, including CR 118.3b, 614.1c/614.12, 611.2a-c, 509.1b, 701.14, 701.23-24, 608.2b, and 712.11b-c/712.12: [official rules PDF](https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.pdf). No additional primitive or engine behavior was introduced in this batch.
