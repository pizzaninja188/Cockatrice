use tricerules_cards::primitives::{
    Amount, CardSearchZone, CardTypeFilter, CompiledChooseRequirement, CompiledInstructionKind,
    PlayerRecipient, ResolutionCost, SearchDestination, SearchSelectionBounds, SearchZoneSelection,
    SpellEffectKind, TriggerCondition, ZoneCardFilter,
};
use tricerules_cards::{AbilityDefinitionId, AbilityPresentation, Layout, ProgramOwnerId};

#[test]
fn solemn_simulacrum_registers_both_optional_triggered_abilities() {
    let registry = tricerules_cards::registry::global();
    let card = registry
        .get("solemn_simulacrum")
        .expect("Solemn Simulacrum registry definition");

    assert_eq!(card.name, "Solemn Simulacrum");
    assert_eq!(
        registry.id_for_name("Solemn Simulacrum"),
        Some("solemn_simulacrum")
    );
    assert_eq!(card.layout, Layout::Normal);
    assert_eq!(card.face_count(), 1);

    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), "solemn_simulacrum");
    assert_eq!(face.mana_cost.to_string(), "{4}");
    assert_eq!(face.types, ["Artifact", "Creature", "Golem"]);
    assert!(face.colors().is_empty());
    assert_eq!((face.power, face.toughness), (Some(2), Some(2)));
    assert!(face.spell_effect.is_empty());
    assert!(face.activated_abilities.is_empty());
    assert_eq!(face.triggered_abilities.len(), 2);

    let entry = &face.triggered_abilities[0];
    assert_eq!(entry.ability_id.as_str(), "triggered_01");
    assert_eq!(
        entry.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(entry.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert!(entry.effect.is_empty());
    assert!(entry.program.is_some());
    let owner = ProgramOwnerId::ability(AbilityDefinitionId {
        card_id: "solemn_simulacrum".to_owned(),
        face_id: face.face_id.clone(),
        ability_path: vec![entry.ability_id.clone()],
    });
    let program = registry
        .compiled_program_for_owner(&owner)
        .expect("Solemn ETB-search trigger compiled program");
    assert_eq!(program.owner, owner);
    assert!(program.required_inputs.is_empty());
    assert!(program.target_bindings.is_empty());
    assert!(program.slots.is_empty());
    assert!(program.body.yields.is_empty());
    let [instruction] = program.body.steps.as_slice() else {
        panic!("Solemn ETB-search trigger has one top-level Choose instruction");
    };
    let CompiledInstructionKind::Choose {
        choice,
        chooser,
        optional,
        branches,
        otherwise,
        ..
    } = &instruction.kind
    else {
        panic!("Solemn ETB-search trigger compiles as Choose");
    };
    assert_eq!(choice.as_str(), "solemn_etb_search");
    assert_eq!(*chooser, PlayerRecipient::Controller);
    assert!(*optional);
    assert!(otherwise.steps.is_empty() && otherwise.yields.is_empty());
    let [branch] = branches.as_slice() else {
        panic!("Solemn ETB-search trigger has one search branch");
    };
    assert_eq!(branch.branch_id.as_str(), "search_for_a_basic_land");
    assert_eq!(branch.cost, ResolutionCost::None);
    assert_eq!(
        branch.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert!(matches!(
        &branch.requirement,
        CompiledChooseRequirement::Always
    ));
    assert!(branch.body.yields.is_empty());
    let [leaf] = branch.body.steps.as_slice() else {
        panic!("Solemn ETB-search branch has one SearchLibrary leaf");
    };
    let CompiledInstructionKind::Perform(effect) = &leaf.kind else {
        panic!("Solemn ETB-search leaf is Perform");
    };
    assert_eq!(
        **effect,
        SpellEffectKind::SearchLibrary {
            who: PlayerRecipient::Controller,
            optional: false,
            count: 1,
            count_by_cast_cost: None,
            filter: Some(ZoneCardFilter {
                card_type: Some(CardTypeFilter::BasicLand),
                ..ZoneCardFilter::default()
            }),
            selection_constraint: None,
            slots: Vec::new(),
            zones: SearchZoneSelection::Fixed(vec![CardSearchZone::Library]),
            destination: SearchDestination::Battlefield { tapped: true },
            conditional_destination: None,
            shuffle: true,
            reveal: false,
            selection_bounds: Some(SearchSelectionBounds { min: 0, max: 1 }),
        }
    );

    let death = &face.triggered_abilities[1];
    assert_eq!(death.ability_id.as_str(), "triggered_02");
    assert_eq!(
        death.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(death.trigger, TriggerCondition::WhenSelfDies);
    assert!(death.effect.is_empty());
    assert!(death.program.is_some());
    let owner = ProgramOwnerId::ability(AbilityDefinitionId {
        card_id: "solemn_simulacrum".to_owned(),
        face_id: face.face_id.clone(),
        ability_path: vec![death.ability_id.clone()],
    });
    let program = registry
        .compiled_program_for_owner(&owner)
        .expect("Solemn death trigger compiled program");
    assert_eq!(program.owner, owner);
    assert!(program.required_inputs.is_empty());
    assert!(program.target_bindings.is_empty());
    assert!(program.slots.is_empty());
    assert!(program.body.yields.is_empty());
    let [instruction] = program.body.steps.as_slice() else {
        panic!("Solemn death trigger has one top-level Choose instruction");
    };
    let CompiledInstructionKind::Choose {
        choice,
        chooser,
        optional,
        branches,
        otherwise,
        ..
    } = &instruction.kind
    else {
        panic!("Solemn death trigger compiles as Choose");
    };
    assert_eq!(choice.as_str(), "solemn_death_draw");
    assert_eq!(*chooser, PlayerRecipient::Controller);
    assert!(*optional);
    assert!(otherwise.steps.is_empty() && otherwise.yields.is_empty());
    let [branch] = branches.as_slice() else {
        panic!("Solemn death trigger has one draw branch");
    };
    assert_eq!(branch.branch_id.as_str(), "draw_a_card");
    assert_eq!(branch.cost, ResolutionCost::None);
    assert_eq!(
        branch.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert!(matches!(
        &branch.requirement,
        CompiledChooseRequirement::Always
    ));
    assert!(branch.body.yields.is_empty());
    let [leaf] = branch.body.steps.as_slice() else {
        panic!("Solemn death draw branch has one Perform leaf");
    };
    let CompiledInstructionKind::Perform(effect) = &leaf.kind else {
        panic!("Solemn death draw leaf is Perform");
    };
    assert_eq!(
        **effect,
        SpellEffectKind::Draw {
            who: PlayerRecipient::Controller,
            count: Amount::Fixed(1),
        }
    );
}
