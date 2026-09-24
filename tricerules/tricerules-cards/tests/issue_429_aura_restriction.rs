//! Issue #429 registry and presentation conformance for the six newly eligible Aura identities.
//!
//! All four identities print their reviewed clauses in the pinned Scryfall snapshot
//! `27bf3214-1271-490b-bdfe-c0be6c23d02e`; exact-name records and `rulings_uri` responses were
//! fetched 2026-09-20. New Horizons returned two rulings (casting with no creatures is legal; a
//! fizzled Aura never triggers), Friendly Neighborhood returned the resolution-time creature
//! count ruling, and Stop Cold returned the later-granted-ability ruling. Stuck in Summoner's
//! Sanctum and Petrify had no Scryfall rulings; Wizards' set release notes clarify that activated
//! abilities contain a colon, including keyword abilities such as equip. The expectations below
//! are the reviewed printed Oracle behavior and the shipped typed vocabulary, not a copy of
//! generator output. CR 303.4/702.5 (Aura attach), 603.6a (entry triggers), 701.26 (tap), 122.1
//! (counters), 111.1 (tokens), 113.10a/605.1a (granted mana ability), 602.2b/611.2c (granted
//! activated ability and its resolution-time count), 613.1f/613.7 (layer-6 ability removal and
//! timestamps), 502.3 (untap-step restriction), and 602.5 (prohibited activations) govern the
//! asserted shapes.

mod common;

use common::FaceExpectation;
use tricerules_cards::primitives::{
    AbilitySourceZone, Amount, CombatRestriction, CountExpression, EffectSubject,
    PermanentTypeFilter, PlayerRecipient, PtScale, PtScaleBasis, RelativePlayerSet,
    SpellEffectKind, StaticAbilityDef, TargetController, TargetFilter, TargetKind,
    TriggerCondition,
};
use tricerules_cards::{
    AbilityCost, AbilityPresentation, ActivatedAbilityDef, ActivationTiming, CardFace,
    CardRegistry, CounterKind, Keyword, ManaAmount, ManaCost,
};

fn face(id: &str) -> &'static CardFace {
    CardRegistry::global()
        .get(id)
        .unwrap_or_else(|| panic!("missing reviewed card {id}"))
        .primary_face()
}

fn creature_filter(controller: TargetController) -> TargetFilter {
    TargetFilter {
        kind: TargetKind::Creature,
        controller,
        ..TargetFilter::default()
    }
}

fn static_definition<'a>(face: &'a CardFace, ability_id: &str, line: u16) -> &'a StaticAbilityDef {
    let [static_ability] = face.static_abilities.as_slice() else {
        panic!("{} must print exactly one static ability", face.name);
    };
    assert_eq!(static_ability.ability_id.as_str(), ability_id);
    assert_eq!(
        static_ability.presentation,
        AbilityPresentation::OracleLines(vec![line])
    );
    &static_ability.definition
}

fn granted_ability(definition: &StaticAbilityDef) -> &ActivatedAbilityDef {
    let StaticAbilityDef::AttachedModifier {
        activated_abilities,
        ..
    } = definition
    else {
        panic!("reviewed Aura static must be an attached modifier");
    };
    activated_abilities
        .first()
        .expect("granted ability must be present")
}

fn two_any_one_color() -> Vec<ManaAmount> {
    let mut options = Vec::new();
    for color in 0..5 {
        let mut amount = ManaAmount::default();
        match color {
            0 => amount.w = 2,
            1 => amount.u = 2,
            2 => amount.b = 2,
            3 => amount.r = 2,
            _ => amount.g = 2,
        }
        options.push(amount);
    }
    options
}

#[test]
fn issue_429_registers_exactly_the_reviewed_six() {
    let registry = CardRegistry::global();
    for (id, name, face_id, mana_cost, types, keywords, power_toughness) in [
        (
            "new_horizons",
            "New Horizons",
            "new_horizons",
            "{2}{G}",
            &["Enchantment", "Aura"][..],
            &[][..],
            None,
        ),
        (
            "friendly_neighborhood",
            "Friendly Neighborhood",
            "friendly_neighborhood",
            "{3}{W}",
            &["Enchantment", "Aura"][..],
            &[][..],
            None,
        ),
        (
            "flood_the_engine",
            "Flood the Engine",
            "flood_the_engine",
            "{2}{U}",
            &["Enchantment", "Aura"][..],
            &[][..],
            None,
        ),
        (
            "stop_cold",
            "Stop Cold",
            "stop_cold",
            "{3}{U}",
            &["Enchantment", "Aura"][..],
            &[Keyword::Flash][..],
            None,
        ),
        (
            "stuck_in_summoners_sanctum",
            "Stuck in Summoner's Sanctum",
            "stuck_in_summoners_sanctum",
            "{2}{U}",
            &["Enchantment", "Aura"][..],
            &[Keyword::Flash][..],
            None,
        ),
        (
            "petrify",
            "Petrify",
            "petrify",
            "{1}{W}",
            &["Enchantment", "Aura"][..],
            &[][..],
            None,
        ),
    ] {
        assert_eq!(registry.id_for_name(name), Some(id), "{id}");
        FaceExpectation {
            id,
            name,
            face_id,
            mana_cost,
            types,
            keywords,
            power_toughness,
        }
        .check();
    }
}

#[test]
fn issue_429_still_excludes_aura_identities_with_other_unsupported_clauses() {
    let registry = CardRegistry::global();
    for (id, name) in [
        ("tractor_beam", "Tractor Beam"),
        ("buried_in_the_garden", "Buried in the Garden"),
        ("shimmerwilds_growth", "Shimmerwilds Growth"),
        ("malfunction", "Malfunction"),
        ("psychic_overload", "Psychic Overload"),
    ] {
        assert!(
            registry.get(id).is_none(),
            "{id} still prints unsupported clauses and must stay unregistered"
        );
        assert!(
            registry.id_for_name(name).is_none(),
            "{id} must not resolve by name"
        );
    }
}

#[test]
fn issue_429_attach_targets_match_the_printed_enchant_lines() {
    assert_eq!(
        face("new_horizons").spell_effect,
        [SpellEffectKind::AuraAttach {
            target: TargetFilter {
                kind: TargetKind::AnyPermanent,
                permanent_types: vec![PermanentTypeFilter::Land],
                ..TargetFilter::default()
            },
        }]
    );
    assert_eq!(
        face("friendly_neighborhood").spell_effect,
        face("new_horizons").spell_effect
    );
    assert_eq!(
        face("stop_cold").spell_effect,
        [SpellEffectKind::AuraAttach {
            target: TargetFilter {
                any_of: Some(vec![
                    TargetFilter {
                        kind: TargetKind::AnyPermanent,
                        permanent_types: vec![PermanentTypeFilter::Artifact],
                        ..TargetFilter::default()
                    },
                    TargetFilter {
                        kind: TargetKind::Creature,
                        ..TargetFilter::default()
                    },
                ]),
                ..TargetFilter::default()
            },
        }]
    );
    assert_eq!(
        face("flood_the_engine").spell_effect,
        [SpellEffectKind::AuraAttach {
            target: TargetFilter {
                any_of: Some(vec![
                    TargetFilter {
                        kind: TargetKind::Creature,
                        ..TargetFilter::default()
                    },
                    TargetFilter {
                        kind: TargetKind::AnyPermanent,
                        required_subtypes: vec!["Vehicle".to_string()],
                        ..TargetFilter::default()
                    },
                ]),
                ..TargetFilter::default()
            },
        }]
    );
    let artifact_or_creature = [SpellEffectKind::AuraAttach {
        target: TargetFilter {
            any_of: Some(vec![
                TargetFilter {
                    kind: TargetKind::AnyPermanent,
                    permanent_types: vec![PermanentTypeFilter::Artifact],
                    ..TargetFilter::default()
                },
                TargetFilter {
                    kind: TargetKind::Creature,
                    ..TargetFilter::default()
                },
            ]),
            ..TargetFilter::default()
        },
    }];
    assert_eq!(
        face("stuck_in_summoners_sanctum").spell_effect,
        artifact_or_creature
    );
    assert_eq!(face("petrify").spell_effect, artifact_or_creature);
}

#[test]
fn issue_429_stuck_and_petrify_keep_each_printed_static_clause() {
    let stuck = face("stuck_in_summoners_sanctum");
    let [stuck_untap, stuck_activation] = stuck.static_abilities.as_slice() else {
        panic!("Stuck in Summoner's Sanctum must have the two printed static clauses");
    };
    assert_eq!(stuck_untap.ability_id.as_str(), "static_01");
    assert_eq!(
        stuck_untap.presentation,
        AbilityPresentation::OracleLines(vec![4])
    );
    let StaticAbilityDef::AttachedModifier {
        doesnt_untap_during_untap_step,
        remove_all_abilities,
        restriction,
        ..
    } = &stuck_untap.definition
    else {
        panic!("Stuck's untap clause must be an attached modifier");
    };
    assert!(*doesnt_untap_during_untap_step);
    assert!(!remove_all_abilities);
    assert_eq!(*restriction, CombatRestriction::default());
    assert_eq!(stuck_activation.ability_id.as_str(), "static_02");
    assert_eq!(
        stuck_activation.presentation,
        AbilityPresentation::OracleLines(vec![4])
    );
    assert_eq!(
        stuck_activation.definition,
        StaticAbilityDef::ProhibitActivatedAbilitiesOfAttachedPermanent
    );
    let [entry] = stuck.triggered_abilities.as_slice() else {
        panic!("Stuck in Summoner's Sanctum must have one entry trigger");
    };
    assert_eq!(entry.ability_id.as_str(), "triggered_01");
    assert_eq!(
        entry.presentation,
        AbilityPresentation::OracleLines(vec![3])
    );
    assert_eq!(entry.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert_eq!(
        entry.effect,
        [SpellEffectKind::Tap {
            subject: EffectSubject::AttachedObject,
        }]
    );

    let petrify = face("petrify");
    let [petrify_combat, petrify_activation] = petrify.static_abilities.as_slice() else {
        panic!("Petrify must have the two printed static clauses");
    };
    assert_eq!(petrify_combat.ability_id.as_str(), "static_01");
    assert_eq!(
        petrify_combat.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    let StaticAbilityDef::AttachedModifier { restriction, .. } = &petrify_combat.definition else {
        panic!("Petrify's combat clause must be an attached modifier");
    };
    assert_eq!(
        *restriction,
        CombatRestriction {
            cant_attack: true,
            cant_block: true,
            ..CombatRestriction::default()
        }
    );
    assert_eq!(petrify_activation.ability_id.as_str(), "static_02");
    assert_eq!(
        petrify_activation.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(
        petrify_activation.definition,
        StaticAbilityDef::ProhibitActivatedAbilitiesOfAttachedPermanent
    );
    assert!(petrify.triggered_abilities.is_empty());
}

#[test]
fn issue_429_new_horizons_counters_and_grants_two_mana_of_one_color() {
    let face = face("new_horizons");
    let [trigger] = face.triggered_abilities.as_slice() else {
        panic!("New Horizons must print exactly one entry trigger");
    };
    assert_eq!(trigger.ability_id.as_str(), "triggered_01");
    assert_eq!(
        trigger.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(
        trigger.effect,
        [SpellEffectKind::PutCounters {
            counter: CounterKind::PlusOnePlusOne,
            count: Amount::Fixed(1),
            subject: EffectSubject::Chosen(Box::new(creature_filter(TargetController::You))),
        }]
    );
    let targeting = trigger
        .targeting
        .as_ref()
        .expect("the counter trigger targets a creature you control");
    assert_eq!(targeting.groups.len(), 1);
    assert_eq!(targeting.groups[0].min, 1);
    assert_eq!(targeting.groups[0].max, 1);
    assert_eq!(
        targeting.groups[0].prompt,
        "Choose target creature you control"
    );
    assert_eq!(targeting.groups[0].effect_indices, vec![0]);

    let definition = static_definition(face, "static_01", 3);
    assert_eq!(
        *definition,
        StaticAbilityDef::AttachedModifier {
            condition: None,
            add_types: Default::default(),
            set_types: None,
            set_name: None,
            set_colors: None,
            delta_power: 0,
            delta_toughness: 0,
            count: None,
            power_per_match: 0,
            toughness_per_match: 0,
            set_power: None,
            set_toughness: None,
            remove_all_abilities: false,
            keywords: Vec::new(),
            triggered_abilities: Vec::new(),
            activated_abilities: vec![ActivatedAbilityDef {
                ability_id: tricerules_cards::AbilityId::new("activated_01").unwrap(),
                presentation: AbilityPresentation::Fallback,
                cost_modifiers: Vec::new(),
                source_zone: AbilitySourceZone::Battlefield,
                costs: vec![AbilityCost::Tap],
                effect: vec![SpellEffectKind::ProduceMana {
                    options: two_any_one_color(),
                    restriction: None,
                    conditional: None,
                }],
                targeting: None,
                timing: ActivationTiming::Normal,
                conditions: Vec::new(),
                activation_limit: None,
            }],
            restriction: Default::default(),
            doesnt_untap_during_untap_step: false,
            cant_untap: false,
        }
    );
    let granted = granted_ability(definition);
    assert_eq!(granted.presentation, AbilityPresentation::Fallback);
    assert_eq!(granted.costs, [AbilityCost::Tap]);
}

#[test]
fn issue_429_friendly_neighborhood_creates_citizens_and_pumps_per_creature() {
    let face = face("friendly_neighborhood");
    let [trigger] = face.triggered_abilities.as_slice() else {
        panic!("Friendly Neighborhood must print exactly one entry trigger");
    };
    assert_eq!(trigger.ability_id.as_str(), "triggered_01");
    assert_eq!(
        trigger.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(
        trigger.effect,
        [SpellEffectKind::CreateTokens {
            token: "human_citizen_gw_1_1".into(),
            count: Amount::Fixed(3),
            who: PlayerRecipient::Controller,
            tapped: false,
            sacrifice_timing: None,
        }]
    );
    assert!(trigger.targeting.is_none());

    let definition = static_definition(face, "static_01", 3);
    assert_eq!(
        *definition,
        StaticAbilityDef::AttachedModifier {
            condition: None,
            add_types: Default::default(),
            set_types: None,
            set_name: None,
            set_colors: None,
            delta_power: 0,
            delta_toughness: 0,
            count: None,
            power_per_match: 0,
            toughness_per_match: 0,
            set_power: None,
            set_toughness: None,
            remove_all_abilities: false,
            keywords: Vec::new(),
            triggered_abilities: Vec::new(),
            activated_abilities: vec![ActivatedAbilityDef {
                ability_id: tricerules_cards::AbilityId::new("activated_01").unwrap(),
                presentation: AbilityPresentation::Fallback,
                cost_modifiers: Vec::new(),
                source_zone: AbilitySourceZone::Battlefield,
                costs: vec![
                    AbilityCost::Mana(ManaCost::parse("{1}").unwrap()),
                    AbilityCost::Tap,
                ],
                effect: vec![SpellEffectKind::PumpTarget {
                    power: 0,
                    toughness: 0,
                    scale: Some(PtScale {
                        basis: PtScaleBasis::Amount(Amount::Count(
                            CountExpression::BattlefieldCreatures {
                                filter:
                                    tricerules_cards::primitives::BattlefieldCreatureCountFilter {
                                        controllers: RelativePlayerSet::Controller,
                                        ..Default::default()
                                    },
                            },
                        )),
                        power_per_unit: 1,
                        toughness_per_unit: 1,
                    }),
                    subject: EffectSubject::Chosen(Box::new(TargetFilter::default_creature())),
                }],
                targeting: Some(tricerules_cards::primitives::TargetingDef {
                    groups: vec![tricerules_cards::primitives::TargetGroupDef {
                        min: 1,
                        max: 1,
                        prompt: "Choose target creature".into(),
                        effect_indices: vec![0],
                        distinct_from: Vec::new(),
                        same_graveyard: false,
                        cast_cost_expansion: None,
                    }],
                }),
                timing: ActivationTiming::SorcerySpeed,
                conditions: Vec::new(),
                activation_limit: None,
            }],
            restriction: Default::default(),
            doesnt_untap_during_untap_step: false,
            cant_untap: false,
        }
    );
    let granted = granted_ability(definition);
    assert_eq!(granted.timing, ActivationTiming::SorcerySpeed);

    let token = CardRegistry::global()
        .get("human_citizen_gw_1_1")
        .expect("the Human Citizen token definition must be registered");
    let token_face = token.primary_face();
    assert_eq!(token_face.name, "Human Citizen");
    assert_eq!(token_face.types, ["Creature", "Human", "Citizen"]);
    assert_eq!(
        token_face.colors(),
        [
            tricerules_cards::primitives::Color::White,
            tricerules_cards::primitives::Color::Green
        ]
    );
    assert_eq!(token_face.power, Some(1));
    assert_eq!(token_face.toughness, Some(1));
}

#[test]
fn issue_429_flood_the_engine_and_stop_cold_remove_abilities_and_lock_untap() {
    let remove_and_lock = StaticAbilityDef::AttachedModifier {
        condition: None,
        add_types: Default::default(),
        set_types: None,
        set_name: None,
        set_colors: None,
        delta_power: 0,
        delta_toughness: 0,
        count: None,
        power_per_match: 0,
        toughness_per_match: 0,
        set_power: None,
        set_toughness: None,
        remove_all_abilities: true,
        keywords: Vec::new(),
        triggered_abilities: Vec::new(),
        activated_abilities: Vec::new(),
        restriction: Default::default(),
        doesnt_untap_during_untap_step: true,
        cant_untap: false,
    };

    for (id, line) in [("flood_the_engine", 3), ("stop_cold", 4)] {
        let face = face(id);
        let [trigger] = face.triggered_abilities.as_slice() else {
            panic!("{id} must print exactly one entry trigger");
        };
        assert_eq!(trigger.ability_id.as_str(), "triggered_01", "{id}");
        assert_eq!(
            trigger.presentation,
            AbilityPresentation::OracleLines(vec![line - 1]),
            "{id}"
        );
        assert_eq!(
            trigger.effect,
            [SpellEffectKind::Tap {
                subject: EffectSubject::AttachedObject,
            }],
            "{id}"
        );
        assert!(trigger.targeting.is_none(), "{id}");
        assert_eq!(*static_definition(face, "static_01", line), remove_and_lock);
    }
}
