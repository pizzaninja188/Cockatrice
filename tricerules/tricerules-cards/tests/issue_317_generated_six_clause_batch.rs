//! Registry and presentation conformance for issue #317.
//!
//! The six exact generator templates must register the six reviewed Standard identities with
//! their complete typed payloads: CR 508/603 attack triggers, CR 111/701.16 Treasure and Clue
//! tokens, CR 404/115 graveyard-card return targeting, CR 605/601.2h tap-creature mana costs, CR
//! 509.1b power-bounded blocking restrictions, CR 113.6/602 graveyard-zone activations, and the
//! Adventure two-face structure.

use tricerules_cards::primitives::{
    CardTypeFilter, EffectSubject, GraveyardDestination, GraveyardFilter, GraveyardOwner,
    ObjectContributionKind, ObjectPaymentConstraint, PermanentTypeFilter, PlayerRecipient,
    PowerComparison, SpellEffectKind, TargetFilter, TargetGroupDef, TargetKind, TargetSchema,
    ZoneCardFilter,
};
use tricerules_cards::{
    AbilityCost, AbilityPresentation, AbilitySourceZone, Amount, CardRegistry, Color, Layout,
    ManaCost, TriggerCondition,
};

fn single_group(targeting: &tricerules_cards::primitives::TargetingDef) -> &TargetGroupDef {
    let [group] = targeting.groups.as_slice() else {
        panic!("expected exactly one authored target group");
    };
    group
}

fn graveyard_creature_card_filter() -> ZoneCardFilter {
    ZoneCardFilter {
        card_type: Some(CardTypeFilter::Creature),
        ..ZoneCardFilter::default()
    }
}

#[test]
fn issue_317_registers_the_six_reviewed_identities() {
    let registry = CardRegistry::global();
    for (id, name, face_count, layout) in [
        (
            "careening_mine_cart",
            "Careening Mine Cart",
            1,
            Layout::Normal,
        ),
        ("fight_on!", "Fight On!", 1, Layout::Normal),
        ("springleaf_drum", "Springleaf Drum", 1, Layout::Normal),
        (
            "stormkeld_vanguard_bear_down",
            "Stormkeld Vanguard // Bear Down",
            2,
            Layout::Adventure,
        ),
        ("cunning_maneuver", "Cunning Maneuver", 1, Layout::Normal),
        (
            "project_deathlok_soldier",
            "Project Deathlok Soldier",
            1,
            Layout::Normal,
        ),
    ] {
        let definition = registry
            .get(id)
            .unwrap_or_else(|| panic!("missing reviewed card {id}"));
        assert_eq!(definition.name, name);
        assert_eq!(registry.id_for_name(name), Some(id));
        assert_eq!(definition.layout, layout);
        assert_eq!(definition.face_count(), face_count);
    }
}

#[test]
fn issue_317_careening_mine_cart_keeps_crew_and_attacks_for_treasure() {
    let definition = CardRegistry::global()
        .get("careening_mine_cart")
        .expect("Careening Mine Cart");
    let face = definition.primary_face();
    assert_eq!(face.face_id.as_str(), "careening_mine_cart");
    assert_eq!(face.mana_cost.to_string(), "{3}");
    assert_eq!(face.types, ["Artifact", "Vehicle"]);
    assert_eq!((face.power, face.toughness), (Some(3), Some(3)));
    assert!(face.colors().is_empty());
    assert!(face.keywords.is_empty());

    let [crew] = face.activated_abilities.as_slice() else {
        panic!("Careening Mine Cart must keep exactly one Crew ability");
    };
    assert_eq!(crew.ability_id.as_str(), "activated_01");
    assert_eq!(crew.presentation, AbilityPresentation::OracleLines(vec![2]));
    assert_eq!(crew.source_zone, AbilitySourceZone::Battlefield);
    assert_eq!(
        crew.costs,
        [AbilityCost::TapPermanents {
            constraint: ObjectPaymentConstraint::AggregateMinimum {
                minimum: 1,
                contribution: ObjectContributionKind::CurrentPower,
            },
            filter: TargetFilter {
                kind: TargetKind::Creature,
                controller: tricerules_cards::primitives::TargetController::You,
                ..TargetFilter::default()
            },
            exclude_source: true,
        }]
    );
    assert!(crew.targeting.is_none());

    let [ability] = face.triggered_abilities.as_slice() else {
        panic!("Careening Mine Cart must have exactly one attack trigger");
    };
    assert_eq!(ability.ability_id.as_str(), "triggered_01");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(
        ability.trigger,
        TriggerCondition::WheneverSelfAttacks {
            minimum_other_attackers: 0,
        }
    );
    assert!(!ability.may);
    assert!(ability.intervening_if.is_none());
    assert_eq!(
        ability.effect,
        [SpellEffectKind::CreateTokens {
            token: "treasure".into(),
            count: Amount::Fixed(1),
            who: PlayerRecipient::Controller,
            tapped: false,
            sacrifice_timing: None,
        }]
    );
    assert!(ability.targeting.is_none());
}

#[test]
fn issue_317_fight_on_returns_up_to_two_creature_cards_from_own_graveyard() {
    let definition = CardRegistry::global().get("fight_on!").expect("Fight On!");
    let face = definition.primary_face();
    assert_eq!(face.face_id.as_str(), "fight_on");
    assert_eq!(face.name, "Fight On!");
    assert_eq!(face.mana_cost.to_string(), "{2}{B}");
    assert_eq!(face.types, ["Instant"]);
    assert_eq!(face.colors(), vec![Color::Black]);
    assert_eq!(
        face.spell_effect,
        [SpellEffectKind::MoveGraveyardCards {
            filter: GraveyardFilter {
                owner: GraveyardOwner::Controller,
                card: Some(graveyard_creature_card_filter()),
                ..GraveyardFilter::default()
            },
            destination: GraveyardDestination::Hand,
            linked_exile_id: None,
        }]
    );
    let targeting = face
        .targeting
        .as_ref()
        .expect("Fight On! targets optionally");
    let group = single_group(targeting);
    assert_eq!((group.min, group.max), (0, 2));
    assert_eq!(
        group.prompt,
        "Choose up to two target creature cards from your graveyard"
    );
    assert_eq!(group.effect_indices, [0]);
    assert!(!group.same_graveyard);
    assert!(TargetSchema::compile(&face.spell_effect, Some(targeting)).is_ok());
}

#[test]
fn issue_317_springleaf_drum_taps_a_creature_for_any_color() {
    let definition = CardRegistry::global()
        .get("springleaf_drum")
        .expect("Springleaf Drum");
    let face = definition.primary_face();
    assert_eq!(face.face_id.as_str(), "springleaf_drum");
    assert_eq!(face.mana_cost.to_string(), "{1}");
    assert_eq!(face.types, ["Artifact"]);
    assert!(face.power.is_none() && face.toughness.is_none());
    assert!(face.triggered_abilities.is_empty());
    assert!(face.static_abilities.is_empty());

    let [ability] = face.activated_abilities.as_slice() else {
        panic!("Springleaf Drum must have exactly one activated ability");
    };
    assert_eq!(ability.ability_id.as_str(), "activated_01");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(ability.source_zone, AbilitySourceZone::Battlefield);
    assert_eq!(
        ability.costs,
        [
            AbilityCost::Tap,
            AbilityCost::TapPermanents {
                constraint: ObjectPaymentConstraint::ExactCount(1),
                filter: TargetFilter {
                    kind: TargetKind::Creature,
                    controller: tricerules_cards::primitives::TargetController::You,
                    ..TargetFilter::default()
                },
                exclude_source: true,
            },
        ]
    );
    assert_eq!(
        ability.effect,
        [SpellEffectKind::ProduceMana {
            options: ['W', 'U', 'B', 'R', 'G']
                .into_iter()
                .map(|symbol| {
                    let mut amount = tricerules_cards::ManaAmount::default();
                    match symbol {
                        'W' => amount.w = 1,
                        'U' => amount.u = 1,
                        'B' => amount.b = 1,
                        'R' => amount.r = 1,
                        _ => amount.g = 1,
                    }
                    amount
                })
                .collect(),
            restriction: None,
            conditional: None,
        }]
    );
    assert!(ability.targeting.is_none());
}

#[test]
fn issue_317_stormkeld_vanguard_adventure_faces_carry_both_clauses() {
    let definition = CardRegistry::global()
        .get("stormkeld_vanguard_bear_down")
        .expect("Stormkeld Vanguard // Bear Down");
    assert_eq!(definition.layout, Layout::Adventure);

    let creature = definition.primary_face();
    assert_eq!(creature.face_id.as_str(), "stormkeld_vanguard");
    assert_eq!(creature.mana_cost.to_string(), "{4}{G}{G}");
    assert_eq!(creature.types, ["Creature", "Giant", "Warrior"]);
    assert_eq!((creature.power, creature.toughness), (Some(6), Some(7)));
    assert_eq!(creature.colors(), vec![Color::Green]);
    assert!(creature.keywords.is_empty());
    assert!(creature.spell_effect.is_empty());

    let [restriction] = creature.static_abilities.as_slice() else {
        panic!("Stormkeld Vanguard must have exactly one static restriction");
    };
    assert_eq!(restriction.ability_id.as_str(), "static_01");
    assert_eq!(
        restriction.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(
        restriction.definition,
        tricerules_cards::primitives::StaticAbilityDef::SelfCombatRestriction {
            restriction: tricerules_cards::primitives::CombatRestriction {
                cant_be_blocked_by: vec![TargetFilter {
                    kind: TargetKind::Creature,
                    power: Some(PowerComparison::AtMost(2)),
                    ..TargetFilter::default()
                }],
                ..tricerules_cards::primitives::CombatRestriction::default()
            },
            condition: None,
        }
    );

    let adventure = definition.face(1).expect("Bear Down face");
    assert_eq!(adventure.face_id.as_str(), "bear_down");
    assert_eq!(adventure.mana_cost.to_string(), "{1}{G}");
    assert_eq!(adventure.types, ["Sorcery", "Adventure"]);
    assert_eq!(adventure.colors(), vec![Color::Green]);
    assert!(adventure.triggered_abilities.is_empty());
    assert_eq!(
        adventure.spell_effect,
        [SpellEffectKind::Destroy {
            subject: EffectSubject::Chosen(Box::new(TargetFilter {
                kind: TargetKind::AnyPermanent,
                permanent_types: vec![
                    PermanentTypeFilter::Artifact,
                    PermanentTypeFilter::Enchantment,
                ],
                ..TargetFilter::default()
            })),
        }]
    );
    assert!(adventure.targeting.is_none());
    assert!(TargetSchema::compile(&adventure.spell_effect, None).is_ok());
}

#[test]
fn issue_317_cunning_maneuver_pumps_then_creates_a_clue() {
    let definition = CardRegistry::global()
        .get("cunning_maneuver")
        .expect("Cunning Maneuver");
    let face = definition.primary_face();
    assert_eq!(face.face_id.as_str(), "cunning_maneuver");
    assert_eq!(face.mana_cost.to_string(), "{1}{R}");
    assert_eq!(face.types, ["Instant"]);
    assert_eq!(face.colors(), vec![Color::Red]);
    assert!(face.targeting.is_none());
    assert_eq!(
        face.spell_effect,
        [
            SpellEffectKind::PumpTarget {
                power: 3,
                toughness: 1,
                scale: None,
                subject: EffectSubject::Chosen(Box::new(TargetFilter::default_creature())),
            },
            SpellEffectKind::CreateTokens {
                token: "clue".into(),
                count: Amount::Fixed(1),
                who: PlayerRecipient::Controller,
                tapped: false,
                sacrifice_timing: None,
            },
        ]
    );
    assert!(TargetSchema::compile(&face.spell_effect, None).is_ok());
}

#[test]
fn issue_317_project_deathlok_soldier_activates_from_the_graveyard() {
    let definition = CardRegistry::global()
        .get("project_deathlok_soldier")
        .expect("Project Deathlok Soldier");
    let face = definition.primary_face();
    assert_eq!(face.face_id.as_str(), "project_deathlok_soldier");
    assert_eq!(face.mana_cost.to_string(), "{B}");
    assert_eq!(face.types, ["Artifact", "Creature", "Zombie", "Soldier"]);
    assert_eq!((face.power, face.toughness), (Some(1), Some(2)));
    assert_eq!(face.colors(), vec![Color::Black]);
    assert!(face.triggered_abilities.is_empty());
    assert!(face.static_abilities.is_empty());

    let [ability] = face.activated_abilities.as_slice() else {
        panic!("Project Deathlok Soldier must have exactly one activated ability");
    };
    assert_eq!(ability.ability_id.as_str(), "activated_01");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(ability.source_zone, AbilitySourceZone::Graveyard);
    assert_eq!(
        ability.costs,
        [AbilityCost::Mana(
            ManaCost::parse("{2}{B}").expect("printed mana cost")
        )]
    );
    assert_eq!(
        ability.effect,
        [SpellEffectKind::ReturnToOwnersHand {
            subject: EffectSubject::Source,
        }]
    );
    assert!(ability.targeting.is_none());
}

#[test]
fn issue_317_fingerprint_rows_match_the_presentation_registry() {
    let registry = CardRegistry::global();
    let cases: &[(&str, &str, &str, &str)] = &[
        (
            "careening_mine_cart",
            "careening_mine_cart",
            "Careening Mine Cart",
            "Careening Mine Cart",
        ),
        ("fight_on!", "fight_on", "Fight On!", "Fight On!"),
        (
            "springleaf_drum",
            "springleaf_drum",
            "Springleaf Drum",
            "Springleaf Drum",
        ),
        (
            "stormkeld_vanguard_bear_down",
            "stormkeld_vanguard",
            "Stormkeld Vanguard // Bear Down",
            "Stormkeld Vanguard",
        ),
        (
            "stormkeld_vanguard_bear_down",
            "bear_down",
            "Stormkeld Vanguard // Bear Down",
            "Bear Down",
        ),
        (
            "cunning_maneuver",
            "cunning_maneuver",
            "Cunning Maneuver",
            "Cunning Maneuver",
        ),
        (
            "project_deathlok_soldier",
            "project_deathlok_soldier",
            "Project Deathlok Soldier",
            "Project Deathlok Soldier",
        ),
    ];
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, face_id, card_name, face_name) in cases {
        let presentation = registry
            .presentation_face(id, face_id)
            .unwrap_or_else(|| panic!("missing presentation metadata for {id}/{face_id}"));
        assert_eq!(presentation.card_name, *card_name);
        assert_eq!(presentation.face_name, *face_name);
        assert_eq!(presentation.oracle_text_sha256.len(), 64);

        let row = fingerprints
            .lines()
            .find(|line| {
                line.starts_with(&format!("{id}\t")) && line.contains(&format!("\t{face_id}\t"))
            })
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}/{face_id}"));
        assert!(
            row.ends_with(&presentation.oracle_text_sha256),
            "fingerprint drift for {id}/{face_id}: {row}"
        );
    }
}
