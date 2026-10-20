//! Issue #424 registry conformance for the eight retained triggered-modal identities.
//!
//! Every generated identity must expose its exact printed mana cost, types, keywords, and
//! `When this creature enters, choose one —` structure (CR 603.3c, 700.2b). Modal-only bullets
//! never leak into ordinary `effect` slots, and the two cohort cards whose unsupported modes lack
//! shipped vocabulary stay unregistered.

use tricerules_cards::primitives::{
    Amount, CardTypeFilter, EffectSubject, GraveyardDestination, GraveyardFilter, GraveyardOwner,
    PermanentTypeFilter, ResolutionCost, SpellEffectKind, TargetFilter, TargetKind, TargetSchema,
    ZoneCardFilter,
};
use tricerules_cards::{CounterKind, ModalDef, TriggerCondition};

fn modal_ability(card_id: &str) -> tricerules_cards::TriggeredAbilityDef {
    let definition = tricerules_cards::registry::global()
        .get(card_id)
        .unwrap_or_else(|| panic!("{card_id} is registered"));
    let face = definition.primary_face();
    let [ability] = face.triggered_abilities.as_slice() else {
        panic!("{card_id} has exactly one triggered ability")
    };
    assert_eq!(
        ability.trigger,
        TriggerCondition::WhenSelfEntersBattlefield,
        "{card_id}"
    );
    assert!(ability.effect.is_empty(), "{card_id}");
    assert!(ability.targeting.is_none(), "{card_id}");
    assert!(!ability.may, "{card_id}");
    assert!(ability.intervening_if.is_none(), "{card_id}");
    ability.clone()
}

fn modal(card_id: &str) -> ModalDef {
    modal_ability(card_id)
        .modal
        .unwrap_or_else(|| panic!("{card_id} is a modal trigger"))
}

fn assert_modal_schema(card_id: &str, modal: &ModalDef) {
    for mode in &modal.modes {
        TargetSchema::compile(&mode.effects, mode.targeting.as_ref()).unwrap_or_else(|error| {
            panic!("{card_id} mode {} target schema: {error}", mode.mode_id)
        });
    }
}

#[test]
fn issue_424_registers_the_seven_triggered_modal_identities() {
    let registry = tricerules_cards::registry::global();
    for (id, name, face_id, mana_cost, types, keywords, power, toughness) in [
        (
            "daily_bugle_reporters",
            "Daily Bugle Reporters",
            "daily_bugle_reporters",
            "{3}{W}",
            &["Creature", "Human", "Citizen"][..],
            &[][..],
            2,
            3,
        ),
        (
            "damage_control_crew",
            "Damage Control Crew",
            "damage_control_crew",
            "{3}{G}",
            &["Creature", "Human", "Citizen"][..],
            &[][..],
            3,
            3,
        ),
        (
            "gearbane_orangutan",
            "Gearbane Orangutan",
            "gearbane_orangutan",
            "{2}{R}",
            &["Creature", "Ape"][..],
            &["Reach"][..],
            2,
            2,
        ),
        (
            "giant-sized_flying_ant",
            "Giant-Sized Flying Ant",
            "giant_sized_flying_ant",
            "{3}{U}",
            &["Creature", "Insect"][..],
            &["Flash", "Flying"][..],
            3,
            2,
        ),
        (
            "glamermite",
            "Glamermite",
            "glamermite",
            "{2}{U}",
            &["Creature", "Faerie", "Rogue"][..],
            &["Flash", "Flying"][..],
            2,
            2,
        ),
        (
            "oltec_archaeologists",
            "Oltec Archaeologists",
            "oltec_archaeologists",
            "{4}{W}",
            &["Creature", "Human", "Artificer", "Scout"][..],
            &[][..],
            4,
            4,
        ),
        (
            "qutrub_forayer",
            "Qutrub Forayer",
            "qutrub_forayer",
            "{2}{B}",
            &["Creature", "Zombie", "Horror"][..],
            &[][..],
            3,
            2,
        ),
        (
            "lord_skitters_butcher",
            "Lord Skitter's Butcher",
            "lord_skitter_s_butcher",
            "{2}{B}",
            &["Creature", "Rat", "Peasant"][..],
            &[][..],
            2,
            3,
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
        assert_eq!(
            face.keywords
                .iter()
                .map(|keyword| keyword.as_str())
                .collect::<Vec<_>>(),
            keywords,
            "{id}"
        );
        assert_eq!(
            (face.power, face.toughness),
            (Some(power), Some(toughness)),
            "{id}"
        );
        let modal = modal(id);
        assert_eq!((modal.min_modes, modal.max_modes), (1, 1), "{id}");
        let expected_modes = if id == "lord_skitters_butcher" { 3 } else { 2 };
        assert_eq!(modal.modes.len(), expected_modes, "{id}");
        assert!(modal.all_modes_cast_cost.is_none(), "{id}");
    }

    for excluded in [
        "blade_of_the_swarm",
        "kutzils_flanker",
        "charming_prince",
        "charming_scoundrel",
    ] {
        assert!(
            registry.get(excluded).is_none(),
            "{excluded} must stay unregistered while a printed mode lacks shipped vocabulary"
        );
    }
}

#[test]
fn issue_424_up_to_two_counter_mode_is_bounded_and_targeted() {
    let bugle = modal("daily_bugle_reporters");
    assert_eq!(
        bugle.modes[0].effects,
        [SpellEffectKind::PutCounters {
            counter: CounterKind::PlusOnePlusOne,
            count: Amount::Fixed(1),
            subject: EffectSubject::Chosen(Box::new(TargetFilter::default_creature())),
        }]
    );
    let group = &bugle.modes[0]
        .targeting
        .as_ref()
        .expect("counter group")
        .groups[0];
    assert_eq!((group.min, group.max), (0, 2));
    assert_eq!(group.prompt, "Choose up to two target creatures");
    assert_eq!(group.effect_indices, vec![0]);
    assert!(!group.same_graveyard);
    assert_modal_schema("daily_bugle_reporters", &bugle);
}

#[test]
fn issue_424_typed_graveyard_returns_expose_printed_predicates() {
    let bugle = modal("daily_bugle_reporters");
    assert_eq!(
        bugle.modes[1].effects,
        [SpellEffectKind::MoveGraveyardCards {
            filter: GraveyardFilter {
                card: Some(ZoneCardFilter {
                    card_type: Some(CardTypeFilter::Creature),
                    max_mana_value: Some(2),
                    ..ZoneCardFilter::default()
                }),
                ..GraveyardFilter::default()
            },
            destination: GraveyardDestination::Hand,
            linked_exile_id: None,
            entry_modifiers: Vec::new(),
        }]
    );
    assert_eq!(
        bugle.modes[1]
            .targeting
            .as_ref()
            .expect("return group")
            .groups[0]
            .prompt,
        "Choose target creature card with mana value 2 or less from your graveyard"
    );
    assert_modal_schema("daily_bugle_reporters", &bugle);

    let crew = modal("damage_control_crew");
    assert_eq!(
        crew.modes[0].effects,
        [SpellEffectKind::MoveGraveyardCards {
            filter: GraveyardFilter {
                card: Some(ZoneCardFilter {
                    min_mana_value: Some(4),
                    ..ZoneCardFilter::default()
                }),
                ..GraveyardFilter::default()
            },
            destination: GraveyardDestination::Hand,
            linked_exile_id: None,
            entry_modifiers: Vec::new(),
        }]
    );

    let oltec = modal("oltec_archaeologists");
    assert_eq!(
        oltec.modes[0].effects,
        [SpellEffectKind::MoveGraveyardCards {
            filter: GraveyardFilter {
                card: Some(ZoneCardFilter {
                    card_type: Some(CardTypeFilter::Artifact),
                    ..ZoneCardFilter::default()
                }),
                ..GraveyardFilter::default()
            },
            destination: GraveyardDestination::Hand,
            linked_exile_id: None,
            entry_modifiers: Vec::new(),
        }]
    );
    assert_eq!(
        oltec.modes[1].effects,
        [SpellEffectKind::Scry {
            count: Amount::Fixed(3),
        }]
    );
    assert_modal_schema("oltec_archaeologists", &oltec);
}

#[test]
fn issue_424_exile_modes_use_the_printed_type_union_and_single_graveyard_group() {
    let crew = modal("damage_control_crew");
    assert_eq!(
        crew.modes[1].effects,
        [SpellEffectKind::Exile {
            subject: EffectSubject::Chosen(Box::new(TargetFilter {
                kind: TargetKind::AnyPermanent,
                permanent_types: vec![
                    PermanentTypeFilter::Artifact,
                    PermanentTypeFilter::Enchantment,
                ],
                ..TargetFilter::default()
            })),
            linked_exile_id: None,
        }]
    );
    assert_eq!(
        crew.modes[1]
            .targeting
            .as_ref()
            .expect("exile group")
            .groups[0]
            .prompt,
        "Choose target artifact or enchantment"
    );
    assert_modal_schema("damage_control_crew", &crew);

    let qutrub = modal("qutrub_forayer");
    assert_eq!(
        qutrub.modes[1].effects,
        [SpellEffectKind::MoveGraveyardCards {
            filter: GraveyardFilter {
                owner: GraveyardOwner::AnyPlayer,
                ..GraveyardFilter::default()
            },
            destination: GraveyardDestination::Exile,
            linked_exile_id: None,
            entry_modifiers: Vec::new(),
        }]
    );
    let group = &qutrub.modes[1]
        .targeting
        .as_ref()
        .expect("single-graveyard group")
        .groups[0];
    assert_eq!((group.min, group.max), (0, 2));
    assert!(group.same_graveyard);
    assert_eq!(
        group.prompt,
        "Choose up to two target cards from a single graveyard"
    );
    assert_modal_schema("qutrub_forayer", &qutrub);
}

#[test]
fn issue_424_gearbane_modes_bind_the_up_to_one_destroy_and_mandatory_sacrifice() {
    use tricerules_cards::primitives::{
        CompiledChooseRequirement, CompiledInstructionKind, ProgramTargetProjection,
    };

    let registry = tricerules_cards::registry::global();
    let ability = modal_ability("gearbane_orangutan");
    let orangutan = ability.modal.expect("Gearbane has its two triggered modes");
    assert_eq!(orangutan.modes.len(), 2);
    let ability_id = tricerules_cards::AbilityDefinitionId {
        card_id: "gearbane_orangutan".into(),
        face_id: tricerules_cards::CardFaceId::new("gearbane_orangutan").unwrap(),
        ability_path: vec![tricerules_cards::AbilityId::new("triggered_01").unwrap()],
    };

    for mode in &orangutan.modes {
        assert!(
            mode.effects.is_empty(),
            "{} has no legacy effects",
            mode.mode_id
        );
        let owner = tricerules_cards::ProgramOwnerId::ability_mode(
            ability_id.clone(),
            mode.mode_id.clone(),
        );
        let program = registry
            .compiled_program_for_owner(&owner)
            .unwrap_or_else(|| panic!("missing compiled Program owner {owner:?}"));
        assert_eq!(program.owner, owner);
        assert_eq!(
            program.target_projection,
            ProgramTargetProjection::ModeLocal {
                mode_id: mode.mode_id.clone(),
            }
        );
        assert!(program.body.yields.is_empty());
        TargetSchema::compile_program(program, mode.targeting.as_ref())
            .unwrap_or_else(|error| panic!("{} target schema: {error}", mode.mode_id));
    }

    let destroy_mode = &orangutan.modes[0];
    assert_eq!(destroy_mode.mode_id.as_str(), "mode_01");
    let group = &destroy_mode
        .targeting
        .as_ref()
        .expect("up-to-one artifact group")
        .groups[0];
    assert_eq!((group.min, group.max), (0, 1));
    assert_eq!(group.prompt, "Choose up to one target artifact");
    assert!(group.effect_indices.is_empty());
    let destroy_owner = tricerules_cards::ProgramOwnerId::ability_mode(
        ability_id.clone(),
        destroy_mode.mode_id.clone(),
    );
    let destroy_program = registry
        .compiled_program_for_owner(&destroy_owner)
        .expect("mode 01 owns its resultless Destroy Program");
    let [destroy] = destroy_program.body.steps.as_slice() else {
        panic!("mode 01 performs one Destroy operation");
    };
    assert!(destroy.outputs.is_empty());
    let artifact_filter = TargetFilter {
        kind: TargetKind::AnyPermanent,
        permanent_types: vec![PermanentTypeFilter::Artifact],
        ..TargetFilter::default()
    };
    assert!(matches!(
        &destroy.kind,
        CompiledInstructionKind::Perform(effect)
            if matches!(effect.as_ref(), SpellEffectKind::Destroy {
                subject: EffectSubject::Chosen(filter)
            } if filter.as_ref() == &artifact_filter)
    ));
    assert_eq!(destroy_program.target_bindings.len(), 1);
    assert_eq!(
        destroy_program.target_bindings[0].instruction_address,
        destroy.instruction_address
    );
    assert_eq!(destroy_program.target_bindings[0].target_group_index, 0);
    assert_eq!(destroy_program.target_bindings[0].role_index, 0);

    let sacrifice_mode = &orangutan.modes[1];
    assert_eq!(sacrifice_mode.mode_id.as_str(), "mode_02");
    assert!(sacrifice_mode.targeting.is_none());
    let sacrifice_owner =
        tricerules_cards::ProgramOwnerId::ability_mode(ability_id, sacrifice_mode.mode_id.clone());
    let sacrifice_program = registry
        .compiled_program_for_owner(&sacrifice_owner)
        .expect("mode 02 owns its mandatory sacrifice Program");
    let [choice] = sacrifice_program.body.steps.as_slice() else {
        panic!("mode 02 has one mandatory sacrifice choice");
    };
    let CompiledInstructionKind::Choose {
        choice: choice_id,
        chooser,
        optional,
        branches,
        otherwise,
    } = &choice.kind
    else {
        panic!("mode 02 compiles to a resultless Choose instruction");
    };
    assert_eq!(choice_id.as_str(), "sacrifice_an_artifact");
    assert_eq!(
        *chooser,
        tricerules_cards::primitives::PlayerRecipient::Controller
    );
    assert!(!optional, "the printed sacrifice is mandatory when legal");
    assert!(otherwise.steps.is_empty() && otherwise.yields.is_empty());
    let [branch] = branches.as_slice() else {
        panic!("the sacrifice mode has exactly one branch")
    };
    assert_eq!(branch.branch_id.as_str(), "sacrifice_an_artifact");
    assert_eq!(
        branch.presentation,
        tricerules_cards::AbilityPresentation::OracleLines(vec![4])
    );
    assert!(matches!(
        &branch.requirement,
        CompiledChooseRequirement::Always
    ));
    assert_eq!(
        branch.cost,
        ResolutionCost::SacrificePermanent {
            filter: TargetFilter {
                kind: TargetKind::AnyPermanent,
                controller: tricerules_cards::primitives::TargetController::You,
                permanent_types: vec![PermanentTypeFilter::Artifact],
                ..TargetFilter::default()
            },
            source_only: false,
        },
        "the mandatory artifact sacrifice keeps its payer-scoped filter"
    );
    assert!(branch.body.yields.is_empty());
    let [put_counters] = branch.body.steps.as_slice() else {
        panic!("the paid branch performs one counter instruction")
    };
    assert!(put_counters.outputs.is_empty());
    assert!(matches!(
        &put_counters.kind,
        CompiledInstructionKind::Perform(effect)
            if matches!(effect.as_ref(), SpellEffectKind::PutCounters {
                counter: CounterKind::PlusOnePlusOne,
                count: Amount::Fixed(2),
                subject: EffectSubject::Source,
            })
    ));
}

#[test]
fn issue_424_tap_and_untap_pairs_keep_their_printed_scopes() {
    for (card_id, clause_filter, prompt) in [
        (
            "giant-sized_flying_ant",
            TargetFilter {
                kind: TargetKind::AnyPermanent,
                excluded_permanent_types: vec![PermanentTypeFilter::Land],
                ..TargetFilter::default()
            },
            "Choose target nonland permanent",
        ),
        (
            "glamermite",
            TargetFilter::default_creature(),
            "Choose target creature",
        ),
    ] {
        let modal = modal(card_id);
        assert_eq!(
            modal.modes[0].effects,
            [SpellEffectKind::Tap {
                subject: EffectSubject::Chosen(Box::new(clause_filter.clone())),
            }],
            "{card_id}"
        );
        assert_eq!(
            modal.modes[1].effects,
            [SpellEffectKind::Untap {
                subject: EffectSubject::Chosen(Box::new(clause_filter)),
            }],
            "{card_id}"
        );
        for mode in &modal.modes {
            let group = &mode.targeting.as_ref().expect("tap group").groups[0];
            assert_eq!((group.min, group.max), (1, 1), "{card_id}");
            assert_eq!(group.prompt, prompt, "{card_id}");
        }
        assert_modal_schema(card_id, &modal);
    }
}

#[test]
fn issue_424_qutrub_destroy_uses_the_damaged_this_turn_predicate() {
    let qutrub = modal("qutrub_forayer");
    assert_eq!(
        qutrub.modes[0].effects,
        [SpellEffectKind::Destroy {
            subject: EffectSubject::Chosen(Box::new(TargetFilter {
                kind: TargetKind::Creature,
                was_dealt_damage_this_turn: Some(true),
                ..TargetFilter::default()
            })),
        }]
    );
    assert_eq!(
        qutrub.modes[0]
            .targeting
            .as_ref()
            .expect("damaged-creature group")
            .groups[0]
            .prompt,
        "Choose target creature that was dealt damage this turn"
    );
    assert_modal_schema("qutrub_forayer", &qutrub);
}

#[test]
fn issue_424_butcher_modes_bind_the_rat_token_another_sacrifice_and_team_menace() {
    use tricerules_cards::primitives::{
        CompiledChooseRequirement, CompiledInstructionKind, ProgramTargetProjection,
    };

    let butcher = modal("lord_skitters_butcher");
    assert_eq!(butcher.modes.len(), 3);
    assert_eq!(
        butcher
            .modes
            .iter()
            .map(|mode| mode.mode_id.as_str())
            .collect::<Vec<_>>(),
        ["mode_01", "mode_02", "mode_03"]
    );

    let registry = tricerules_cards::registry::global();
    let ability = modal_ability("lord_skitters_butcher");
    let ability_id = tricerules_cards::AbilityDefinitionId {
        card_id: "lord_skitters_butcher".into(),
        face_id: tricerules_cards::CardFaceId::new("lord_skitter_s_butcher").unwrap(),
        ability_path: vec![ability.ability_id],
    };
    let programs = butcher
        .modes
        .iter()
        .map(|mode| {
            assert!(
                mode.effects.is_empty(),
                "{} has no legacy effects",
                mode.mode_id
            );
            let owner = tricerules_cards::ProgramOwnerId::ability_mode(
                ability_id.clone(),
                mode.mode_id.clone(),
            );
            let program = registry
                .compiled_program_for_owner(&owner)
                .unwrap_or_else(|| panic!("missing compiled Program owner {owner:?}"));
            assert_eq!(program.owner, owner);
            assert_eq!(
                program.target_projection,
                ProgramTargetProjection::ModeLocal {
                    mode_id: mode.mode_id.clone(),
                }
            );
            assert!(program.body.yields.is_empty());
            (mode, program)
        })
        .collect::<Vec<_>>();

    let [rat_mode, sacrifice_mode, menace_mode] = programs.as_slice() else {
        panic!("all three printed modes have compiled Programs")
    };
    assert_eq!(rat_mode.0.mode_id.as_str(), "mode_01");
    let [rat] = rat_mode.1.body.steps.as_slice() else {
        panic!("the Rat mode performs one token creation")
    };
    assert!(rat.outputs.is_empty());
    assert!(matches!(
        &rat.kind,
        CompiledInstructionKind::Perform(effect)
            if matches!(effect.as_ref(), SpellEffectKind::CreateTokens {
                token,
                count: Amount::Fixed(1),
                who: tricerules_cards::primitives::PlayerRecipient::Controller,
                tapped: false,
                sacrifice_timing: None,
            } if token == "rat_b_1_1_cant_block")
    ));

    assert_eq!(sacrifice_mode.0.mode_id.as_str(), "mode_02");
    let [choice] = sacrifice_mode.1.body.steps.as_slice() else {
        panic!("the sacrifice mode has one optional resolution choice")
    };
    let CompiledInstructionKind::Choose {
        choice: choice_id,
        chooser,
        optional,
        branches,
        otherwise,
    } = &choice.kind
    else {
        panic!("the sacrifice mode compiles as Choose")
    };
    assert_eq!(choice_id.as_str(), "sacrifice_another_creature");
    assert_eq!(
        *chooser,
        tricerules_cards::primitives::PlayerRecipient::Controller
    );
    assert!(*optional, "the printed sacrifice is optional");
    assert!(otherwise.steps.is_empty() && otherwise.yields.is_empty());
    let [branch] = branches.as_slice() else {
        panic!("the sacrifice mode has exactly one branch")
    };
    assert_eq!(branch.branch_id.as_str(), "sacrifice_another_creature");
    assert_eq!(
        branch.presentation,
        tricerules_cards::AbilityPresentation::OracleLines(vec![3])
    );
    assert!(matches!(
        &branch.requirement,
        CompiledChooseRequirement::Always
    ));
    assert_eq!(
        branch.cost,
        ResolutionCost::SacrificePermanent {
            filter: TargetFilter {
                kind: TargetKind::Creature,
                controller: tricerules_cards::primitives::TargetController::You,
                excluded_objects: vec![tricerules_cards::TargetObjectExclusion::Source],
                ..TargetFilter::default()
            },
            source_only: false,
        },
        "the sacrifice keeps the source-relative another-creature exclusion"
    );
    assert!(branch.body.yields.is_empty());
    let [scry, draw] = branch.body.steps.as_slice() else {
        panic!("the paid branch scries 2 then draws 1")
    };
    assert!(matches!(
        &scry.kind,
        CompiledInstructionKind::Perform(effect)
            if matches!(effect.as_ref(), SpellEffectKind::Scry {
                count: Amount::Fixed(2),
            })
    ));
    assert!(matches!(
        &draw.kind,
        CompiledInstructionKind::Perform(effect)
            if matches!(effect.as_ref(), SpellEffectKind::Draw {
                who: tricerules_cards::primitives::PlayerRecipient::Controller,
                count: Amount::Fixed(1),
            })
    ));

    assert_eq!(menace_mode.0.mode_id.as_str(), "mode_03");
    let [grant] = menace_mode.1.body.steps.as_slice() else {
        panic!("the Menace mode performs one team grant")
    };
    assert!(grant.outputs.is_empty());
    assert!(matches!(
        &grant.kind,
        CompiledInstructionKind::Perform(effect)
            if matches!(effect.as_ref(), SpellEffectKind::GrantKeywordsAll {
                filter: tricerules_cards::primitives::CreatureScopeFilter {
                    controller: Some(tricerules_cards::primitives::CreatureScopeController::YouControl),
                    ..
                },
                keywords,
            } if keywords == &[tricerules_cards::Keyword::Menace])
    ));
    assert_modal_schema("lord_skitters_butcher", &butcher);
}
