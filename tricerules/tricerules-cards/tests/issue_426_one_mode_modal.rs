//! Issue #426 registry conformance for the ten retained one-mode-away modal identities.
//!
//! Every generated identity must expose its exact printed mana cost, types, and two-mode
//! structure (CR 700.2). Modal-only bullets never leak into ordinary `spell_effect` slots, and
//! Confounding Riddle stays unregistered because its look-four mode has no shipped destination
//! shape.

use tricerules_cards::primitives::{
    Amount, BattlefieldCreatureCountFilter, BattlefieldPermanentFilter, CardTypeFilter,
    CountExpression, EffectSubject, HandCardAction, HandCardChooser, HandChoiceVisibility,
    PermanentTypeFilter, PtScale, PtScaleBasis, RelativePlayerSet, SpellEffectKind,
    TargetController, TargetFilter, TargetKind, TargetSchema, TypeLineAddition,
};
use tricerules_cards::{CardRegistry, CounterKind, Keyword, ModalDef};

fn modal(card_id: &str) -> ModalDef {
    CardRegistry::global()
        .get(card_id)
        .unwrap_or_else(|| panic!("{card_id} is registered"))
        .primary_face()
        .modal_spell
        .clone()
        .unwrap_or_else(|| panic!("{card_id} is a modal spell"))
}

fn assert_modal_schema(card_id: &str, modal: &ModalDef) {
    for mode in &modal.modes {
        TargetSchema::compile(&mode.effects, mode.targeting.as_ref()).unwrap_or_else(|error| {
            panic!("{card_id} mode {} target schema: {error}", mode.mode_id)
        });
    }
}

fn creature_filter(controller: TargetController) -> TargetFilter {
    TargetFilter {
        kind: TargetKind::Creature,
        controller,
        ..TargetFilter::default()
    }
}

fn creature_or_planeswalker() -> TargetFilter {
    TargetFilter {
        kind: TargetKind::AnyPermanent,
        permanent_types: vec![
            PermanentTypeFilter::Creature,
            PermanentTypeFilter::Planeswalker,
        ],
        ..TargetFilter::default()
    }
}

#[test]
fn issue_426_registers_the_ten_one_mode_away_identities() {
    let registry = CardRegistry::global();
    for (id, name, face_id, mana_cost, types) in [
        (
            "cerebral_confiscation",
            "Cerebral Confiscation",
            "cerebral_confiscation",
            "{2}{B}",
            &["Sorcery"][..],
        ),
        (
            "collision_course",
            "Collision Course",
            "collision_course",
            "{1}{W}",
            &["Sorcery"][..],
        ),
        (
            "coordinated_maneuver",
            "Coordinated Maneuver",
            "coordinated_maneuver",
            "{1}{W}",
            &["Instant"][..],
        ),
        (
            "crash_and_burn",
            "Crash and Burn",
            "crash_and_burn",
            "{3}{R}",
            &["Instant"][..],
        ),
        (
            "frontline_rush",
            "Frontline Rush",
            "frontline_rush",
            "{R}{W}",
            &["Instant"][..],
        ),
        (
            "keep_out",
            "Keep Out",
            "keep_out",
            "{1}{W}",
            &["Instant"][..],
        ),
        (
            "moment_of_valor",
            "Moment of Valor",
            "moment_of_valor",
            "{2}{W}",
            &["Instant"][..],
        ),
        (
            "over_the_edge",
            "Over the Edge",
            "over_the_edge",
            "{1}{G}",
            &["Sorcery"][..],
        ),
        (
            "spectacular_tactics",
            "Spectacular Tactics",
            "spectacular_tactics",
            "{1}{W}",
            &["Instant"][..],
        ),
        (
            "stone_by_sunlight",
            "Stone by Sunlight",
            "stone_by_sunlight",
            "{1}{W}",
            &["Instant"][..],
        ),
    ] {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {name}"));
        assert_eq!(definition.name, name, "{id}");
        let face = definition.primary_face();
        assert_eq!(face.face_id.as_str(), face_id, "{id}");
        assert_eq!(face.mana_cost.to_string(), mana_cost, "{id}");
        assert_eq!(
            face.types.iter().map(String::as_str).collect::<Vec<_>>(),
            types,
            "{id}"
        );
        assert!(face.spell_effect.is_empty(), "{id}");
        assert!(face.targeting.is_none(), "{id}");
        let modal = modal(id);
        assert_eq!((modal.min_modes, modal.max_modes), (1, 1), "{id}");
        assert_eq!(modal.modes.len(), 2, "{id}");
        assert!(modal.all_modes_cast_cost.is_none(), "{id}");
    }
    assert!(
        registry.get("confounding_riddle").is_none(),
        "Confounding Riddle must stay unsupported while no instruction sends one looked-at card \
         to hand and the remainder to the graveyard"
    );
}

#[test]
fn issue_426_modes_expose_exact_effects_and_target_prompts() {
    let cerebral = modal("cerebral_confiscation");
    assert_eq!(
        cerebral.modes[0].effects,
        [SpellEffectKind::ChooseHandCards {
            action: HandCardAction::Discard,
            count: 2,
            target: TargetFilter {
                kind: TargetKind::OpponentPlayer,
                ..TargetFilter::default()
            },
            chooser: HandCardChooser::AffectedPlayer,
            card_filter: None,
            optional: false,
            visibility: HandChoiceVisibility::PrivateLook,
        }]
    );
    assert_eq!(
        cerebral.modes[1].effects,
        [SpellEffectKind::ChooseHandCards {
            action: HandCardAction::Discard,
            count: 1,
            target: TargetFilter {
                kind: TargetKind::OpponentPlayer,
                ..TargetFilter::default()
            },
            chooser: HandCardChooser::Controller,
            card_filter: Some(CardTypeFilter::Nonland),
            optional: false,
            visibility: HandChoiceVisibility::PublicReveal,
        }]
    );
    assert_eq!(
        cerebral.modes[1]
            .targeting
            .as_ref()
            .expect("reveal target group")
            .groups[0]
            .prompt,
        "Choose target opponent"
    );
    assert_modal_schema("cerebral_confiscation", &cerebral);

    let creature = BattlefieldPermanentFilter {
        token: None,
        any_of: None,
        controllers: RelativePlayerSet::Controller,
        card_type: Some(CardTypeFilter::Creature),
        color: None,
        name: None,
        required_subtypes: Vec::new(),
        exclude_source: false,
    };
    let vehicle = BattlefieldPermanentFilter {
        token: None,
        any_of: None,
        controllers: RelativePlayerSet::Controller,
        card_type: None,
        color: None,
        name: None,
        required_subtypes: vec!["Vehicle".into()],
        exclude_source: false,
    };
    let collision = modal("collision_course");
    assert_eq!(
        collision.modes[0].effects,
        [SpellEffectKind::DamageTarget {
            amount: Amount::Count(CountExpression::BattlefieldPermanents {
                filter: BattlefieldPermanentFilter {
                    token: None,
                    any_of: Some(vec![creature, vehicle]),
                    controllers: RelativePlayerSet::Controller,
                    card_type: None,
                    color: None,
                    name: None,
                    required_subtypes: Vec::new(),
                    exclude_source: false,
                },
            }),
            target: TargetFilter::default_creature(),
        }]
    );
    assert_eq!(
        collision.modes[1].effects,
        [SpellEffectKind::Destroy {
            subject: EffectSubject::Chosen(Box::new(TargetFilter {
                kind: TargetKind::AnyPermanent,
                permanent_types: vec![PermanentTypeFilter::Artifact],
                ..TargetFilter::default()
            })),
        }]
    );
    assert_modal_schema("collision_course", &collision);

    let coordinated = modal("coordinated_maneuver");
    assert_eq!(
        coordinated.modes[0].effects,
        [SpellEffectKind::DamageTarget {
            amount: Amount::Count(CountExpression::BattlefieldCreatures {
                filter: BattlefieldCreatureCountFilter::default(),
            }),
            target: creature_or_planeswalker(),
        }]
    );
    assert_eq!(
        coordinated.modes[0]
            .targeting
            .as_ref()
            .expect("creature-or-planeswalker group")
            .groups[0]
            .prompt,
        "Choose target creature or planeswalker"
    );
    assert_modal_schema("coordinated_maneuver", &coordinated);

    let crash = modal("crash_and_burn");
    assert_eq!(
        crash.modes[0].effects,
        [SpellEffectKind::Destroy {
            subject: EffectSubject::Chosen(Box::new(TargetFilter {
                kind: TargetKind::AnyPermanent,
                required_subtypes: vec!["Vehicle".into()],
                ..TargetFilter::default()
            })),
        }]
    );
    assert_eq!(
        crash.modes[1].effects,
        [SpellEffectKind::DamageTarget {
            amount: Amount::Fixed(6),
            target: creature_or_planeswalker(),
        }]
    );
    assert_modal_schema("crash_and_burn", &crash);

    let frontline = modal("frontline_rush");
    assert_eq!(
        frontline.modes[0].effects,
        [SpellEffectKind::CreateTokens {
            token: "goblin_r_1_1".into(),
            count: Amount::Fixed(2),
            who: tricerules_cards::primitives::PlayerRecipient::Controller,
            tapped: false,
            sacrifice_timing: None,
        }]
    );
    assert_eq!(
        frontline.modes[1].effects,
        [SpellEffectKind::PumpTarget {
            power: 0,
            toughness: 0,
            scale: Some(PtScale {
                basis: PtScaleBasis::Amount(Amount::Count(CountExpression::BattlefieldCreatures {
                    filter: BattlefieldCreatureCountFilter::default(),
                },)),
                power_per_unit: 1,
                toughness_per_unit: 1,
            }),
            subject: EffectSubject::Chosen(Box::new(TargetFilter::default_creature())),
        }]
    );
    assert_modal_schema("frontline_rush", &frontline);

    let keep_out = modal("keep_out");
    assert_eq!(
        keep_out.modes[0].effects,
        [SpellEffectKind::DamageTarget {
            amount: Amount::Fixed(4),
            target: TargetFilter {
                kind: TargetKind::Creature,
                tapped: Some(true),
                ..TargetFilter::default()
            },
        }]
    );
    assert_eq!(
        keep_out.modes[0]
            .targeting
            .as_ref()
            .expect("tapped-creature group")
            .groups[0]
            .prompt,
        "Choose target tapped creature"
    );
    assert_modal_schema("keep_out", &keep_out);

    let moment = modal("moment_of_valor");
    assert_eq!(
        moment.modes[0].effects,
        [
            SpellEffectKind::Untap {
                subject: EffectSubject::Chosen(Box::new(TargetFilter::default_creature())),
            },
            SpellEffectKind::PumpTarget {
                power: 1,
                toughness: 0,
                scale: None,
                subject: EffectSubject::Chosen(Box::new(TargetFilter::default_creature())),
            },
            SpellEffectKind::GrantKeywords {
                subject: EffectSubject::Chosen(Box::new(TargetFilter::default_creature())),
                keywords: vec![Keyword::Indestructible],
            },
        ]
    );
    assert_eq!(
        moment.modes[0]
            .targeting
            .as_ref()
            .expect("untap group")
            .groups[0]
            .effect_indices,
        vec![0, 1, 2]
    );
    assert_modal_schema("moment_of_valor", &moment);

    let over = modal("over_the_edge");
    let explore_target = creature_filter(TargetController::You);
    assert_eq!(
        over.modes[1].effects,
        [
            SpellEffectKind::Explore {
                subject: EffectSubject::Chosen(Box::new(explore_target.clone())),
            },
            SpellEffectKind::Explore {
                subject: EffectSubject::Chosen(Box::new(explore_target)),
            },
        ]
    );
    assert_eq!(
        over.modes[1]
            .targeting
            .as_ref()
            .expect("explore group")
            .groups[0]
            .prompt,
        "Choose target creature you control"
    );
    assert_modal_schema("over_the_edge", &over);

    let tactics = modal("spectacular_tactics");
    let tactics_target = creature_filter(TargetController::You);
    assert_eq!(
        tactics.modes[0].effects,
        [
            SpellEffectKind::PutCounters {
                counter: CounterKind::PlusOnePlusOne,
                count: Amount::Fixed(1),
                subject: EffectSubject::Chosen(Box::new(tactics_target.clone())),
            },
            SpellEffectKind::GrantKeywords {
                subject: EffectSubject::Chosen(Box::new(tactics_target)),
                keywords: vec![Keyword::Hexproof],
            },
        ]
    );
    assert_modal_schema("spectacular_tactics", &tactics);

    let stone = modal("stone_by_sunlight");
    let stone_target = TargetFilter::default_creature();
    assert_eq!(
        stone.modes[1].effects,
        [
            SpellEffectKind::AddTypes {
                subject: EffectSubject::Chosen(Box::new(stone_target.clone())),
                addition: TypeLineAddition {
                    card_types: vec![PermanentTypeFilter::Artifact],
                    creature_types: Vec::new(),
                },
            },
            SpellEffectKind::GrantKeywords {
                subject: EffectSubject::Chosen(Box::new(stone_target)),
                keywords: vec![Keyword::Indestructible],
            },
        ]
    );
    assert_modal_schema("stone_by_sunlight", &stone);
}
