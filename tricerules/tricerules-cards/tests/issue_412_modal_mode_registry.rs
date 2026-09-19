//! Issue #412 registry conformance for the eight retained modal identities.
//!
//! The cohort spans modal spells, Teamwork modal spells, and modal ETB triggers (CR 700.2,
//! CR 702.194, CR 603.3c). Every generated identity must expose its exact printed mana cost,
//! types, keywords, and mode structure; modal-only bullets never leak into ordinary
//! `spell_effect`/`effect` slots.

mod common;
use common::FaceExpectation;

use tricerules_cards::primitives::{
    CardTypeFilter, CastCostOptionDef, EffectSubject, GraveyardDestination, GraveyardFilter,
    GraveyardOwner, LibraryPartitionKind, ObjectCastCostKind, ObjectContributionKind,
    ObjectPaymentConstraint, PermanentTypeFilter, PlayerRecipient, SpellEffectKind,
    StackSpellFilter, TargetController, TargetFilter, TargetKind, TargetSchema,
};
use tricerules_cards::{Amount, CardRegistry, CounterKind, Keyword, ModalDef, TriggerCondition};

fn modal(card_id: &str) -> ModalDef {
    CardRegistry::global()
        .get(card_id)
        .unwrap_or_else(|| panic!("{card_id} is registered"))
        .primary_face()
        .modal_spell
        .clone()
        .unwrap_or_else(|| panic!("{card_id} is a modal spell"))
}

fn triggered_modal(card_id: &str) -> ModalDef {
    let definition = CardRegistry::global()
        .get(card_id)
        .unwrap_or_else(|| panic!("{card_id} is registered"));
    let [ability] = definition.primary_face().triggered_abilities.as_slice() else {
        panic!("{card_id} owns exactly one triggered ability");
    };
    assert_eq!(ability.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert!(ability.effect.is_empty(), "{card_id}");
    ability
        .modal
        .clone()
        .unwrap_or_else(|| panic!("{card_id} modal ETB"))
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

fn enchantment_destroy() -> SpellEffectKind {
    SpellEffectKind::Destroy {
        subject: EffectSubject::Chosen(Box::new(TargetFilter {
            kind: TargetKind::AnyPermanent,
            permanent_types: vec![PermanentTypeFilter::Enchantment],
            ..TargetFilter::default()
        })),
    }
}

fn artifact_destroy() -> SpellEffectKind {
    SpellEffectKind::Destroy {
        subject: EffectSubject::Chosen(Box::new(TargetFilter {
            kind: TargetKind::AnyPermanent,
            permanent_types: vec![PermanentTypeFilter::Artifact],
            ..TargetFilter::default()
        })),
    }
}

#[test]
fn issue_412_registers_the_eight_modal_identities() {
    for (id, name, face_id, mana_cost, types, keywords, power_toughness) in [
        (
            "coliseum_behemoth",
            "Coliseum Behemoth",
            "coliseum_behemoth",
            "{5}{G}{G}",
            &["Creature", "Beast"][..],
            &[Keyword::Trample][..],
            Some((7, 7)),
        ),
        (
            "fangkeepers_familiar",
            "Fangkeeper's Familiar",
            "fangkeeper_s_familiar",
            "{1}{B}{G}{U}",
            &["Creature", "Snake"][..],
            &[Keyword::Flash][..],
            Some((3, 3)),
        ),
        (
            "go_nuts!",
            "Go Nuts!",
            "go_nuts",
            "{G}",
            &["Sorcery"][..],
            &[][..],
            None,
        ),
        (
            "heritage_reclamation",
            "Heritage Reclamation",
            "heritage_reclamation",
            "{1}{G}",
            &["Instant"][..],
            &[][..],
            None,
        ),
        (
            "hulk_smash!",
            "HULK SMASH!",
            "hulk_smash",
            "{1}{R}",
            &["Instant"][..],
            &[][..],
            None,
        ),
        (
            "pawpatch_formation",
            "Pawpatch Formation",
            "pawpatch_formation",
            "{1}{G}",
            &["Instant"][..],
            &[][..],
            None,
        ),
        (
            "plow_through",
            "Plow Through",
            "plow_through",
            "{G}",
            &["Sorcery"][..],
            &[][..],
            None,
        ),
        (
            "unforgiving_aim",
            "Unforgiving Aim",
            "unforgiving_aim",
            "{2}{G}",
            &["Instant"][..],
            &[][..],
            None,
        ),
    ] {
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
fn issue_412_two_mode_spells_expose_exact_modes_and_targets() {
    let plow = modal("plow_through");
    assert_eq!((plow.min_modes, plow.max_modes), (1, 1));
    assert_eq!(plow.modes.len(), 2);
    assert_eq!(
        plow.modes[0].effects,
        [SpellEffectKind::Fight {
            first: EffectSubject::Chosen(Box::new(creature_filter(TargetController::You))),
            second: EffectSubject::Chosen(Box::new(creature_filter(TargetController::Opponent))),
        }]
    );
    let fight_groups = &plow.modes[0]
        .targeting
        .as_ref()
        .expect("fight groups")
        .groups;
    assert_eq!(fight_groups.len(), 2);
    assert_eq!(fight_groups[0].prompt, "Choose target creature you control");
    assert_eq!(
        fight_groups[1].prompt,
        "Choose target creature an opponent controls"
    );
    assert_eq!(
        plow.modes[1].effects,
        [SpellEffectKind::Destroy {
            subject: EffectSubject::Chosen(Box::new(TargetFilter {
                kind: TargetKind::AnyPermanent,
                required_subtypes: vec!["Vehicle".into()],
                ..TargetFilter::default()
            })),
        }]
    );
    assert_modal_schema("plow_through", &plow);
}

#[test]
fn issue_412_three_mode_spells_expose_every_bullet() {
    let heritage = modal("heritage_reclamation");
    assert_eq!((heritage.min_modes, heritage.max_modes), (1, 1));
    assert_eq!(heritage.modes.len(), 3);
    assert_eq!(heritage.modes[0].effects, [artifact_destroy()]);
    assert_eq!(heritage.modes[1].effects, [enchantment_destroy()]);
    assert_eq!(
        heritage.modes[2].effects,
        [
            SpellEffectKind::MoveGraveyardCards {
                filter: GraveyardFilter {
                    owner: GraveyardOwner::AnyPlayer,
                    ..GraveyardFilter::default()
                },
                destination: GraveyardDestination::Exile,
                linked_exile_id: None,
            },
            SpellEffectKind::Draw {
                who: PlayerRecipient::Controller,
                count: Amount::Fixed(1),
            },
        ]
    );
    let exile_groups = &heritage.modes[2]
        .targeting
        .as_ref()
        .expect("exile group")
        .groups;
    assert_eq!((exile_groups[0].min, exile_groups[0].max), (0, 1));
    assert_eq!(
        exile_groups[0].prompt,
        "Choose up to one target card from a graveyard"
    );
    assert_modal_schema("heritage_reclamation", &heritage);

    let pawpatch = modal("pawpatch_formation");
    assert_eq!((pawpatch.min_modes, pawpatch.max_modes), (1, 1));
    assert_eq!(pawpatch.modes.len(), 3);
    assert!(matches!(
        pawpatch.modes[0].effects.as_slice(),
        [SpellEffectKind::Destroy { subject }]
            if matches!(subject, EffectSubject::Chosen(filter)
                if filter.required_keywords.contains(&Keyword::Flying))
    ));
    assert_eq!(pawpatch.modes[1].effects, [enchantment_destroy()]);
    assert_eq!(
        pawpatch.modes[2].effects,
        [
            SpellEffectKind::Draw {
                who: PlayerRecipient::Controller,
                count: Amount::Fixed(1),
            },
            SpellEffectKind::CreateTokens {
                token: "food".into(),
                count: Amount::Fixed(1),
                who: PlayerRecipient::Controller,
                tapped: false,
                sacrifice_timing: None,
            },
        ]
    );
    assert_modal_schema("pawpatch_formation", &pawpatch);

    let aim = modal("unforgiving_aim");
    assert_eq!((aim.min_modes, aim.max_modes), (1, 1));
    assert_eq!(aim.modes.len(), 3);
    assert!(matches!(
        aim.modes[0].effects.as_slice(),
        [SpellEffectKind::Destroy { subject }]
            if matches!(subject, EffectSubject::Chosen(filter)
                if filter.required_keywords.contains(&Keyword::Flying))
    ));
    assert_eq!(aim.modes[1].effects, [enchantment_destroy()]);
    assert_eq!(
        aim.modes[2].effects,
        [SpellEffectKind::CreateTokens {
            token: "elf_bg_2_2".into(),
            count: Amount::Fixed(1),
            who: PlayerRecipient::Controller,
            tapped: false,
            sacrifice_timing: None,
        }]
    );
    assert_modal_schema("unforgiving_aim", &aim);
}

#[test]
fn issue_412_teamwork_modal_spells_expose_the_cost_and_both_mode_allowance() {
    for (card_id, power, first_mode, second_mode) in [
        (
            "go_nuts!",
            3,
            SpellEffectKind::PutCounters {
                counter: CounterKind::PlusOnePlusOne,
                count: Amount::Fixed(1),
                subject: EffectSubject::Chosen(Box::new(TargetFilter::default_creature())),
            },
            SpellEffectKind::Fight {
                first: EffectSubject::Chosen(Box::new(creature_filter(TargetController::You))),
                second: EffectSubject::Chosen(Box::new(creature_filter(
                    TargetController::Opponent,
                ))),
            },
        ),
        (
            "hulk_smash!",
            4,
            SpellEffectKind::Destroy {
                subject: EffectSubject::Chosen(Box::new(TargetFilter {
                    kind: TargetKind::AnyPermanent,
                    permanent_types: vec![PermanentTypeFilter::Artifact],
                    excluded_permanent_types: vec![PermanentTypeFilter::Creature],
                    ..TargetFilter::default()
                })),
            },
            SpellEffectKind::CreatureDealsDamageEqualToPower {
                source: creature_filter(TargetController::You),
                target: creature_filter(TargetController::Opponent),
            },
        ),
    ] {
        let definition = CardRegistry::global()
            .get(card_id)
            .unwrap_or_else(|| panic!("{card_id} is registered"));
        let face = definition.primary_face();
        assert_eq!(face.cast_cost_groups.len(), 1, "{card_id}");
        let group = &face.cast_cost_groups[0];
        assert_eq!(group.group_id.as_str(), "teamwork", "{card_id}");
        assert_eq!(
            group.options,
            [CastCostOptionDef::TapPermanents {
                option_id: tricerules_cards::ChoiceId::new(format!("teamwork_{power}")).unwrap(),
                presentation: tricerules_cards::AbilityPresentation::OracleLines(vec![1]),
                kind: ObjectCastCostKind::Teamwork,
                constraint: ObjectPaymentConstraint::AggregateMinimum {
                    minimum: power,
                    contribution: ObjectContributionKind::CurrentPower,
                },
                filter: Box::new(creature_filter(TargetController::You)),
            }],
            "{card_id}"
        );
        let modal = modal(card_id);
        assert_eq!((modal.min_modes, modal.max_modes), (1, 2), "{card_id}");
        assert_eq!(
            modal.all_modes_cast_cost.as_ref().map(|reference| {
                (
                    reference.group_id.as_str().to_string(),
                    reference.option_id.as_str().to_string(),
                )
            }),
            Some(("teamwork".into(), format!("teamwork_{power}"))),
            "{card_id}"
        );
        assert_eq!(modal.modes[0].effects, [first_mode], "{card_id}");
        assert_eq!(modal.modes[1].effects, [second_mode], "{card_id}");
        assert_modal_schema(card_id, &modal);
    }
}

#[test]
fn issue_412_etb_modal_creatures_expose_the_triggered_mode_structure() {
    assert_eq!(
        CardRegistry::global()
            .get("coliseum_behemoth")
            .expect("Coliseum Behemoth")
            .primary_face()
            .keywords,
        [Keyword::Trample]
    );
    let coliseum = triggered_modal("coliseum_behemoth");
    assert_eq!((coliseum.min_modes, coliseum.max_modes), (1, 1));
    assert_eq!(coliseum.modes.len(), 2);
    assert_eq!(
        coliseum.modes[0].effects,
        [SpellEffectKind::Destroy {
            subject: EffectSubject::Chosen(Box::new(TargetFilter {
                kind: TargetKind::AnyPermanent,
                permanent_types: vec![
                    PermanentTypeFilter::Artifact,
                    PermanentTypeFilter::Enchantment
                ],
                ..TargetFilter::default()
            })),
        }]
    );
    assert_eq!(
        coliseum.modes[1].effects,
        [SpellEffectKind::Draw {
            who: PlayerRecipient::Controller,
            count: Amount::Fixed(1),
        }]
    );
    assert_modal_schema("coliseum_behemoth", &coliseum);

    assert_eq!(
        CardRegistry::global()
            .get("fangkeepers_familiar")
            .expect("Fangkeeper's Familiar")
            .primary_face()
            .keywords,
        [Keyword::Flash]
    );
    let fangkeeper = triggered_modal("fangkeepers_familiar");
    assert_eq!((fangkeeper.min_modes, fangkeeper.max_modes), (1, 1));
    assert_eq!(fangkeeper.modes.len(), 3);
    assert_eq!(
        fangkeeper.modes[0].effects,
        [
            SpellEffectKind::GainLife {
                amount: Amount::Fixed(3),
            },
            SpellEffectKind::LibraryPartition {
                count: 3,
                top_min: 0,
                top_max: None,
                kind: LibraryPartitionKind::Surveil,
            },
        ]
    );
    assert_eq!(fangkeeper.modes[1].effects, [enchantment_destroy()]);
    assert_eq!(
        fangkeeper.modes[2].effects,
        [SpellEffectKind::CounterTargetSpell {
            spell_filter: StackSpellFilter {
                card_type: Some(CardTypeFilter::Creature),
                ..StackSpellFilter::default()
            },
            unless_controller_pays: None,
            unless_controller_pays_by_cast_cost: None,
        }]
    );
    assert_modal_schema("fangkeepers_familiar", &fangkeeper);
}
