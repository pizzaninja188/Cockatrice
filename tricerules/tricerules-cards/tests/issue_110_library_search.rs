use tricerules_cards::primitives::{
    AbilityCost, CardProgram, CardSearchZone, CardTypeFilter, CompiledChooseRequirement,
    CompiledInstructionKind, GraveyardDestination, LibraryBottomOrder, LibraryPlacement,
    PlayerRecipient, PowerComparison, ResolutionCost, SearchDestination, SearchSelectionBounds,
    SearchZoneSelection, SpellEffectKind, ZoneCardFilter,
};
use tricerules_cards::{AbilityDefinitionId, ProgramOwnerId};

fn card(id: &str) -> &'static tricerules_cards::CardDefinition {
    tricerules_cards::registry::global()
        .get(id)
        .expect("issue #110 card is registered")
}

#[test]
fn issue_110_filters_cover_exact_name_recursive_type_or_and_printed_power() {
    let name = ZoneCardFilter {
        exact_name: Some("Tempest Hawk".into()),
        ..Default::default()
    };
    assert!(name.validate().is_ok());

    let creature_or_land = ZoneCardFilter {
        any_of: Some(vec![
            ZoneCardFilter {
                card_type: Some(CardTypeFilter::Creature),
                ..Default::default()
            },
            ZoneCardFilter {
                card_type: Some(CardTypeFilter::Land),
                ..Default::default()
            },
        ]),
        ..Default::default()
    };
    assert!(creature_or_land.validate().is_ok());

    let small_creature = ZoneCardFilter {
        card_type: Some(CardTypeFilter::Creature),
        printed_power: Some(PowerComparison::AtMost(2)),
        ..Default::default()
    };
    assert!(small_creature.validate().is_ok());
    assert!(ZoneCardFilter::default().validate().is_err());
}

#[test]
fn issue_110_search_and_look_cards_use_the_shared_primitives() {
    let registry = tricerules_cards::registry::global();
    let tempest_card = card("tempest_hawk");
    let tempest = tempest_card.primary_face();
    let [tempest_ability] = tempest.triggered_abilities.as_slice() else {
        panic!("Tempest Hawk has one combat-damage trigger");
    };
    assert!(tempest_ability.effect.is_empty());
    assert!(tempest_ability.program.is_some());
    let owner = ProgramOwnerId::ability(AbilityDefinitionId {
        card_id: "tempest_hawk".to_owned(),
        face_id: tempest.face_id.clone(),
        ability_path: vec![tempest_ability.ability_id.clone()],
    });
    let program = registry
        .compiled_program_for_owner(&owner)
        .expect("Tempest Hawk's exact triggered ability owns its search program");
    assert!(program.required_inputs.is_empty());
    assert!(program.target_bindings.is_empty());
    assert!(program.slots.is_empty());
    let [choice_step] = program.body.steps.as_slice() else {
        panic!("Tempest Hawk has one top-level search choice");
    };
    let CompiledInstructionKind::Choose {
        choice,
        chooser,
        optional,
        branches,
        otherwise,
        ..
    } = &choice_step.kind
    else {
        panic!("Tempest Hawk's trigger compiles to a structural Choose");
    };
    assert_eq!(choice.as_str(), "search");
    assert_eq!(*chooser, PlayerRecipient::Controller);
    assert!(*optional);
    assert!(otherwise.steps.is_empty() && otherwise.yields.is_empty());
    let [branch] = branches.as_slice() else {
        panic!("Tempest Hawk has one exact-name search branch");
    };
    assert_eq!(branch.branch_id.as_str(), "search_for_tempest_hawk");
    assert_eq!(branch.cost, ResolutionCost::None);
    assert!(matches!(
        &branch.requirement,
        CompiledChooseRequirement::Always
    ));
    let [search_step] = branch.body.steps.as_slice() else {
        panic!("Tempest Hawk's search branch has one operation");
    };
    let CompiledInstructionKind::Perform(effect) = &search_step.kind else {
        panic!("Tempest Hawk's search branch is a Perform instruction");
    };
    assert!(matches!(
        effect.as_ref(),
        SpellEffectKind::SearchLibrary {
            who: PlayerRecipient::Controller,
            optional: false,
            count: 1,
            count_by_cast_cost: None,
            filter: Some(ZoneCardFilter { exact_name: Some(name), .. }),
            selection_constraint: None,
            slots,
            zones: SearchZoneSelection::Fixed(zones),
            destination: SearchDestination::Hand,
            conditional_destination: None,
            shuffle: true,
            reveal: true,
            result_id: None,
            selection_bounds: Some(SearchSelectionBounds { min: 0, max: 1 }),
        } if name == "Tempest Hawk"
            && zones == &[CardSearchZone::Library]
            && slots.is_empty()
    ));

    let living = card("living_phone").primary_face();
    assert!(living.triggered_abilities.iter().any(|ability| matches!(
        ability.effect.as_slice(),
        [SpellEffectKind::LookChooseToHand {
            count: 5,
            filter: Some(ZoneCardFilter {
                card_type: Some(CardTypeFilter::Creature),
                printed_power: Some(PowerComparison::AtMost(2)),
                ..
            }),
            bottom_order: LibraryBottomOrder::Random,
            ..
        }]
    )));
}

#[test]
fn issue_110_say_its_name_and_altanak_are_complete() {
    let say = card("say_its_name");
    let expected_program: CardProgram = ron::from_str(
        r#"(body: (steps: [
            (id: "mill_three", bind: [], input: None,
                kind: Perform(Mill(count: 3, who: Controller))),
            (id: "choose_creature_or_land", bind: [], input: None,
                kind: Perform(ChooseCard(
                    source: WholeCurrentGraveyard,
                    filter: (any_of: Some([
                        (card_type: Some(Creature)),
                        (card_type: Some(Land)),
                    ])),
                    destination: Hand,
                    optional: true,
                ))),
        ], yields: []), target_bindings: [])"#,
    )
    .expect("Say Its Name's exact WholeCurrentGraveyard program is valid RON");
    let say_face = say.primary_face();
    assert!(
        say_face.spell_effect.is_empty()
            && say_face.spell_program.as_ref() == Some(&expected_program),
        "Say Its Name must mill three, then optionally return a current graveyard creature or land"
    );
    let ability = &say.primary_face().activated_abilities[0];
    assert!(matches!(
        ability.costs.as_slice(),
        [
            AbilityCost::ExileSelf,
            AbilityCost::ExileGraveyardCards {
                constraint: tricerules_cards::ObjectPaymentConstraint::ExactCount(2),
                exclude_source: true,
                ..
            }
        ]
    ));
    assert!(
        matches!(ability.effect.as_slice(), [SpellEffectKind::SearchLibrary {
        zones: SearchZoneSelection::PlayerChoice(zones),
        destination: SearchDestination::Battlefield { tapped: false },
        shuffle: true,
        ..
    }] if zones == &[CardSearchZone::Hand, CardSearchZone::Graveyard, CardSearchZone::Library])
    );

    let altanak = card("altanak,_the_thrice-called");
    assert!(matches!(
        altanak.primary_face().activated_abilities[0]
            .effect
            .as_slice(),
        [SpellEffectKind::MoveGraveyardCards {
            destination: GraveyardDestination::Battlefield { tapped: true },
            ..
        }]
    ));
}

#[test]
fn issue_110_conditional_and_owner_choice_cards_are_complete() {
    let registry = tricerules_cards::registry::global();
    let embermouth = card("embermouth_sentinel");
    let face = embermouth.primary_face();
    let [ability] = face.triggered_abilities.as_slice() else {
        panic!("Embermouth Sentinel has one ETB search trigger");
    };
    assert!(ability.effect.is_empty());
    assert!(ability.program.is_some());
    let owner = ProgramOwnerId::ability(AbilityDefinitionId {
        card_id: "embermouth_sentinel".to_owned(),
        face_id: face.face_id.clone(),
        ability_path: vec![ability.ability_id.clone()],
    });
    let program = registry
        .compiled_program_for_owner(&owner)
        .expect("Embermouth's exact ETB ability owns the program");
    assert!(program.required_inputs.is_empty());
    assert!(program.target_bindings.is_empty());
    assert!(program.slots.is_empty());
    let [choice_step] = program.body.steps.as_slice() else {
        panic!("Embermouth has one top-level choice");
    };
    let CompiledInstructionKind::Choose {
        choice,
        chooser,
        optional,
        branches,
        otherwise,
        ..
    } = &choice_step.kind
    else {
        panic!("Embermouth's outer may-choice is a structural Choose");
    };
    assert_eq!(choice.as_str(), "search");
    assert_eq!(*chooser, PlayerRecipient::Controller);
    assert!(*optional);
    assert!(otherwise.steps.is_empty() && otherwise.yields.is_empty());
    let [branch] = branches.as_slice() else {
        panic!("Embermouth offers one search branch");
    };
    assert_eq!(branch.branch_id.as_str(), "search_for_a_basic_land");
    assert_eq!(branch.cost, ResolutionCost::None);
    assert!(matches!(
        &branch.requirement,
        CompiledChooseRequirement::Always
    ));
    let [search_step] = branch.body.steps.as_slice() else {
        panic!("Embermouth's branch contains one search");
    };
    assert!(search_step.outputs.is_empty());
    let CompiledInstructionKind::Perform(effect) = &search_step.kind else {
        panic!("Embermouth's branch contains a normal search leaf");
    };
    assert!(matches!(
        effect.as_ref(),
        SpellEffectKind::SearchLibrary {
            who: PlayerRecipient::Controller,
            optional: false,
            count: 1,
            count_by_cast_cost: None,
            filter: Some(filter),
            selection_constraint: None,
            slots,
            zones: SearchZoneSelection::Fixed(zones),
            destination: SearchDestination::TopOfLibrary,
            conditional_destination: Some(conditional),
            shuffle: true,
            reveal: true,
            result_id: None,
            selection_bounds: Some(SearchSelectionBounds { min: 0, max: 1 }),
        } if filter == &ZoneCardFilter {
                card_type: Some(CardTypeFilter::BasicLand),
                ..Default::default()
            }
            && zones == &[CardSearchZone::Library]
            && slots.is_empty()
            && conditional.destination == (SearchDestination::Battlefield { tapped: true })
            && matches!(
                &conditional.condition,
                tricerules_cards::primitives::GameCondition::BattlefieldAggregate {
                    filter: tricerules_cards::primitives::BattlefieldPermanentFilter {
                        controllers: tricerules_cards::primitives::RelativePlayerSet::Controller,
                        card_type: None,
                        required_subtypes,
                        exclude_source: false,
                        ..
                    },
                    aggregate: tricerules_cards::primitives::BattlefieldAggregate::Count,
                    min: Some(1),
                    max: None,
                    exclude_observed_object: false,
                } if required_subtypes == &["Dragon".to_owned()]
            )
    ));

    let voyage = card("uncharted_voyage").primary_face();
    assert!(matches!(
        voyage.spell_effect.as_slice(),
        [
            SpellEffectKind::PutInOwnersLibrary {
                placement: LibraryPlacement::OwnerChoiceTopOrBottom,
                ..
            },
            SpellEffectKind::LibraryPartition {
                count: 1,
                top_min: 0,
                ..
            }
        ]
    ));

    let riverwalk = card("riverwalk_technique").primary_face();
    let modal = riverwalk
        .modal_spell
        .as_ref()
        .expect("Riverwalk Technique is modal");
    assert_eq!(
        (modal.min_modes, modal.max_modes, modal.modes.len()),
        (1, 1, 2)
    );
    assert!(matches!(
        modal.modes[0].effects.as_slice(),
        [SpellEffectKind::PutInOwnersLibrary {
            placement: LibraryPlacement::OwnerChoiceTopOrBottom,
            ..
        }]
    ));
    assert!(matches!(
        modal.modes[1].effects.as_slice(),
        [SpellEffectKind::CounterTargetSpell { .. }]
    ));
}
