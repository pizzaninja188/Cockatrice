//! Issue #370 registry conformance for the three retained graveyard-count cost-reduction
//! identities. Chitin Gravestalker, Gargantuan Leech, and Tolarian Terror are complete: every
//! printed clause is covered by an exact recipe, and the cost modifier resolves from the
//! controller's public graveyard at cost determination (CR 601.2f, 404.2).
//!
//! Diamond Weapon (Immune), Hollow Marauder (per-opponent discard rider), The Dawning Archaic
//! (graveyard cast permission), Serpent of the Pass (conditional flash permission), and
//! Fugitive Codebreaker (Disguise, #345) stay unretained. Their exact cost-reduction recipes are
//! catalog-tested, but their identities keep their existing blockers. The permanent-card and
//! noncreature-nonland payloads therefore have no retained card consumer; the final test proves
//! those exact emitted shapes are registry-loadable.

use tricerules_cards::primitives::{
    Amount, BattlefieldPermanentFilter, CardTypeFilter, CountExpression, QuantityTerm,
    RelativePlayerSet, SpellCostModifier, ZoneCardFilter,
};
use tricerules_cards::{AbilityCost, AbilitySourceZone, CardRegistry, Keyword};

fn creature_card_filter() -> ZoneCardFilter {
    ZoneCardFilter {
        card_type: Some(CardTypeFilter::Creature),
        ..ZoneCardFilter::default()
    }
}

fn permanent_card_filter() -> ZoneCardFilter {
    ZoneCardFilter {
        excluded_card_types: vec![CardTypeFilter::Instant, CardTypeFilter::Sorcery],
        ..ZoneCardFilter::default()
    }
}

fn noncreature_nonland_card_filter() -> ZoneCardFilter {
    ZoneCardFilter {
        excluded_card_types: vec![CardTypeFilter::Creature, CardTypeFilter::Land],
        ..ZoneCardFilter::default()
    }
}

fn graveyard_reduction(filter: ZoneCardFilter) -> SpellCostModifier {
    SpellCostModifier::GenericReduction {
        amount: Amount::Count(CountExpression::GraveyardCards {
            owners: RelativePlayerSet::Controller,
            filter: Some(filter),
        }),
    }
}

fn registry_with(card: &str) -> Result<CardRegistry, String> {
    CardRegistry::from_chunks_and_tokens(&[card], &[]).map_err(|error| error.to_string())
}

#[test]
fn issue_370_registers_the_three_completed_identities() {
    let registry = CardRegistry::global();
    for (id, name, face_id, mana_cost, types, stats, keywords) in [
        (
            "chitin_gravestalker",
            "Chitin Gravestalker",
            "chitin_gravestalker",
            "{5}{B}",
            vec!["Creature", "Insect", "Warrior"],
            (Some(5), Some(4)),
            vec![],
        ),
        (
            "gargantuan_leech",
            "Gargantuan Leech",
            "gargantuan_leech",
            "{7}{B}",
            vec!["Creature", "Leech"],
            (Some(5), Some(5)),
            vec![Keyword::Lifelink],
        ),
        (
            "tolarian_terror",
            "Tolarian Terror",
            "tolarian_terror",
            "{6}{U}",
            vec!["Creature", "Serpent"],
            (Some(5), Some(5)),
            vec![],
        ),
    ] {
        let definition = registry
            .get(id)
            .unwrap_or_else(|| panic!("missing retained card {id}"));
        assert_eq!(definition.name, name);
        assert_eq!(registry.id_for_name(name), Some(id));
        let face = definition.primary_face();
        assert_eq!(face.face_id.as_str(), face_id);
        assert_eq!(face.mana_cost.to_string(), mana_cost);
        assert_eq!(face.types, types);
        assert_eq!(face.supertypes, Vec::<String>::new());
        assert_eq!((face.power, face.toughness), stats);
        assert_eq!(face.keywords, keywords);
    }
}

#[test]
fn issue_370_chitin_gravestalker_cost_modifier_payload_is_exact() {
    let definition = CardRegistry::global()
        .get("chitin_gravestalker")
        .expect("Chitin Gravestalker");
    let face = definition.primary_face();
    assert_eq!(
        face.cost_modifiers.as_slice(),
        [graveyard_reduction(ZoneCardFilter {
            any_of: Some(vec![
                ZoneCardFilter {
                    card_type: Some(CardTypeFilter::Artifact),
                    ..ZoneCardFilter::default()
                },
                creature_card_filter(),
            ]),
            ..ZoneCardFilter::default()
        })],
        "artifact and/or creature cards is a pure-OR graveyard predicate"
    );

    let [cycling] = face.activated_abilities.as_slice() else {
        panic!("Chitin Gravestalker must keep exactly its Cycling activated ability");
    };
    assert_eq!(cycling.ability_id.as_str(), "activated_01");
    assert_eq!(cycling.source_zone, AbilitySourceZone::Hand);
    assert_eq!(
        cycling.costs,
        [
            AbilityCost::Mana(tricerules_cards::ManaCost::parse("{2}").expect("Cycling {2}")),
            AbilityCost::DiscardSelf
        ]
    );
}

#[test]
fn issue_370_gargantuan_leech_cost_modifier_payload_is_exact() {
    let definition = CardRegistry::global()
        .get("gargantuan_leech")
        .expect("Gargantuan Leech");
    let face = definition.primary_face();
    assert_eq!(
        face.cost_modifiers.as_slice(),
        [SpellCostModifier::GenericReduction {
            amount: Amount::Count(CountExpression::Affine {
                constant: 0,
                terms: vec![
                    QuantityTerm {
                        coefficient: 1,
                        quantity: CountExpression::BattlefieldPermanents {
                            filter: BattlefieldPermanentFilter {
                                token: None,
                                any_of: None,
                                controllers: RelativePlayerSet::Controller,
                                card_type: None,
                                color: None,
                                name: None,
                                required_subtypes: vec!["Cave".to_string()],
                                exclude_source: false,
                            },
                        },
                    },
                    QuantityTerm {
                        coefficient: 1,
                        quantity: CountExpression::GraveyardCards {
                            owners: RelativePlayerSet::Controller,
                            filter: Some(ZoneCardFilter {
                                required_subtypes: vec!["Cave".to_string()],
                                ..ZoneCardFilter::default()
                            }),
                        },
                    },
                ],
            }),
        }],
        "Caves you control plus Cave cards in your graveyard is one affine amount"
    );
}

#[test]
fn issue_370_tolarian_terror_cost_modifier_payload_is_exact() {
    let definition = CardRegistry::global()
        .get("tolarian_terror")
        .expect("Tolarian Terror");
    let face = definition.primary_face();
    assert_eq!(
        face.cost_modifiers.as_slice(),
        [graveyard_reduction(ZoneCardFilter {
            card_type: Some(CardTypeFilter::InstantOrSorcery),
            ..ZoneCardFilter::default()
        })],
        "instant and sorcery cards are the shipped InstantOrSorcery union"
    );

    let [ward] = face.triggered_abilities.as_slice() else {
        panic!("Tolarian Terror must keep exactly its Ward triggered ability");
    };
    assert_eq!(ward.ability_id.as_str(), "triggered_01");
    assert_eq!(
        ward.trigger,
        tricerules_cards::TriggerCondition::WheneverSelfBecomesTarget {
            source: tricerules_cards::primitives::TargetingSourceFilter::SpellOrAbility,
            source_controller: tricerules_cards::CastTriggerPlayer::Opponent,
        }
    );
}

#[test]
fn issue_370_excludes_the_unretained_identities() {
    let registry = CardRegistry::global();
    for name in [
        "Diamond Weapon",
        "Hollow Marauder",
        "The Dawning Archaic",
        "Serpent of the Pass",
        "Fugitive Codebreaker",
    ] {
        assert_eq!(
            registry.id_for_name(name),
            None,
            "{name} has an unsupported clause and must stay unretained"
        );
    }
}

/// Build one-cost-spell fixture RON around an exact `GenericReduction` amount so the registry
/// proves the unretained recipes' emitted shapes are loadable and keep their filters.
fn reduction_spell_fixture(id: &str, name: &str, amount: &str) -> String {
    format!(
        r#"(id: "{id}", name: "{name}", face_id: "{id}", mana_cost: "{{3}}{{B}}", types: ["Sorcery"],
            cost_modifiers: [GenericReduction(amount: {amount})],
            spell_effect: [GainLife(amount: 1)])"#
    )
}

#[test]
fn issue_370_unretained_recipe_payloads_validate_in_the_registry() {
    let cases = [
        (
            "gy_permanent",
            "Gy Permanent",
            "Count(GraveyardCards(owners: Controller, filter: Some((excluded_card_types: [Instant, Sorcery]))))",
            graveyard_reduction(permanent_card_filter()),
        ),
        (
            "gy_noncreature_nonland",
            "Gy Noncreature Nonland",
            "Count(GraveyardCards(owners: Controller, filter: Some((excluded_card_types: [Creature, Land]))))",
            graveyard_reduction(noncreature_nonland_card_filter()),
        ),
        (
            "gy_artifact_creature",
            "Gy Artifact Creature",
            "Count(GraveyardCards(owners: Controller, filter: Some((any_of: Some([(card_type: Some(Artifact)), (card_type: Some(Creature))])))))",
            graveyard_reduction(ZoneCardFilter {
                any_of: Some(vec![
                    ZoneCardFilter {
                        card_type: Some(CardTypeFilter::Artifact),
                        ..ZoneCardFilter::default()
                    },
                    creature_card_filter(),
                ]),
                ..ZoneCardFilter::default()
            }),
        ),
    ];
    for (id, name, amount, expected) in cases {
        let registry = registry_with(&reduction_spell_fixture(id, name, amount))
            .unwrap_or_else(|error| panic!("{id} payload must be registry-loadable: {error}"));
        assert_eq!(
            registry
                .get(id)
                .expect("fixture registered")
                .primary_face()
                .cost_modifiers
                .as_slice(),
            [expected],
            "{id}"
        );
    }

    let caves_amount = "Count(Affine(constant: 0, terms: [\
        (coefficient: 1, quantity: BattlefieldPermanents(filter: (token: None, any_of: None, \
            controllers: Controller, card_type: None, color: None, name: None, \
            required_subtypes: [\"Cave\"], exclude_source: false))), \
        (coefficient: 1, quantity: GraveyardCards(owners: Controller, \
            filter: Some((required_subtypes: [\"Cave\"]))))\
        ]))";
    let caves = registry_with(&reduction_spell_fixture(
        "gy_caves",
        "Gy Caves",
        caves_amount,
    ))
    .unwrap_or_else(|error| panic!("Cave affine payload must be registry-loadable: {error}"));
    assert_eq!(
        caves
            .get("gy_caves")
            .expect("fixture registered")
            .primary_face()
            .cost_modifiers
            .as_slice(),
        [SpellCostModifier::GenericReduction {
            amount: Amount::Count(CountExpression::Affine {
                constant: 0,
                terms: vec![
                    QuantityTerm {
                        coefficient: 1,
                        quantity: CountExpression::BattlefieldPermanents {
                            filter: BattlefieldPermanentFilter {
                                token: None,
                                any_of: None,
                                controllers: RelativePlayerSet::Controller,
                                card_type: None,
                                color: None,
                                name: None,
                                required_subtypes: vec!["Cave".to_string()],
                                exclude_source: false,
                            },
                        },
                    },
                    QuantityTerm {
                        coefficient: 1,
                        quantity: CountExpression::GraveyardCards {
                            owners: RelativePlayerSet::Controller,
                            filter: Some(ZoneCardFilter {
                                required_subtypes: vec!["Cave".to_string()],
                                ..ZoneCardFilter::default()
                            }),
                        },
                    },
                ],
            }),
        }]
    );
}
