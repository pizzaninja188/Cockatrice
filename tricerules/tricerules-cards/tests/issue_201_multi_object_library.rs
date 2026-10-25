use tricerules_cards::primitives::{
    AbilityCost, CompiledProgram, CounterKind, EffectSubject, InstructionKind, Keyword,
    ProgramTargetRole, SpellEffectKind, TargetController, TargetKind,
};
use tricerules_cards::{AbilityDefinitionId, Amount, CardProgram, EffectContext, ProgramOwnerId};

#[test]
fn floodpits_drowner_has_current_oracle_shape() {
    let definition = tricerules_cards::registry::global()
        .get("floodpits_drowner")
        .expect("Floodpits Drowner must be registered");
    let face = definition.primary_face();

    assert_eq!(definition.name, "Floodpits Drowner");
    assert_eq!(face.mana_cost.to_string(), "{1}{U}");
    assert_eq!(face.types, ["Creature", "Merfolk"]);
    assert_eq!((face.power, face.toughness), (Some(2), Some(1)));
    assert!(face.keywords.contains(&Keyword::Flash));
    assert!(face.keywords.contains(&Keyword::Vigilance));

    let etb = &face.triggered_abilities[0];
    assert!(etb.effect.is_empty());
    let etb_program = etb.program.as_ref().expect("ETB uses a typed Program");
    assert_eq!(etb_program.body.steps.len(), 2);
    assert!(matches!(
        &etb_program.body.steps[0].kind,
        InstructionKind::Perform(effect)
            if matches!(effect.as_ref(), SpellEffectKind::Tap {
                subject: EffectSubject::TargetAlias(name)
            } if name == "target_creature")
    ));
    assert!(matches!(
        &etb_program.body.steps[1].kind,
        InstructionKind::Perform(effect)
            if matches!(effect.as_ref(), SpellEffectKind::PutCounters {
                counter: CounterKind::Stun,
                count: Amount::Fixed(1),
                subject: EffectSubject::TargetAlias(name),
            } if name == "target_creature")
    ));
    assert!(etb_program.target_bindings.is_empty());
    let [target_alias] = etb_program.target_aliases.as_slice() else {
        panic!("Floodpits Drowner declares one owner-local target alias");
    };
    assert_eq!(target_alias.name, "target_creature");
    assert_eq!(target_alias.group_index, 0);
    assert_eq!(target_alias.filter.kind, TargetKind::Creature);
    assert_eq!(target_alias.filter.controller, TargetController::Opponent);
    let etb_targeting = etb.targeting.as_ref().expect("one required target");
    assert!(etb_targeting.groups[0].effect_indices.is_empty());

    let activated = &face.activated_abilities[0];
    assert_eq!(activated.costs.len(), 2);
    assert!(matches!(&activated.costs[0], AbilityCost::Mana(cost) if cost.to_string() == "{1}{U}"));
    assert!(matches!(activated.costs[1], AbilityCost::Tap));
    assert!(activated.effect.is_empty());
    let program = activated
        .program
        .as_ref()
        .expect("activated ability uses Program");
    let [shuffle] = program.body.steps.as_slice() else {
        panic!("activated ability has one atomic multi-subject instruction");
    };
    let InstructionKind::Perform(effect) = &shuffle.kind else {
        panic!("activated ability performs the multi-subject shuffle");
    };
    let SpellEffectKind::ShufflePermanentsIntoOwnersLibraries { subjects } = &**effect else {
        panic!("activated ability uses the atomic multi-subject primitive");
    };
    assert!(matches!(subjects[0], EffectSubject::Source));
    assert!(matches!(
        &subjects[1],
        EffectSubject::Chosen(filter)
            if filter.kind == TargetKind::Creature
                && filter.controller == TargetController::Any
                && filter.required_counter == Some(CounterKind::Stun)
    ));
    let targeting = activated.targeting.as_ref().expect("one required target");
    assert_eq!(targeting.groups.len(), 1);
    assert_eq!((targeting.groups[0].min, targeting.groups[0].max), (1, 1));
    assert!(targeting.groups[0].effect_indices.is_empty());
    assert_eq!(program.target_bindings.len(), 1);
    assert_eq!(
        program.target_bindings[0].instruction_id,
        "shuffle_source_and_target"
    );
    assert_eq!(program.target_bindings[0].role, ProgramTargetRole::Target);
    assert_eq!(program.target_bindings[0].target_group_index, 0);
}

#[test]
fn named_target_aliases_match_pre_migration_programs_for_both_owners() {
    let registry = tricerules_cards::registry::global();

    let vow = registry
        .get("vow_to_erebor")
        .expect("Vow to Erebor must be registered");
    let vow_face = vow.primary_face();
    let vow_program = vow_face
        .spell_program
        .as_ref()
        .expect("Vow to Erebor has an authored spell Program");
    let vow_owner = ProgramOwnerId::face("vow_to_erebor", vow_face.face_id.clone());
    let registered_vow = registry
        .compiled_program_for_owner(&vow_owner)
        .expect("Vow to Erebor's face has a compiled Program");
    let pre_migration_vow: CardProgram = ron::from_str(
        r#"(body: (steps: [
            (id: "untap", bind: [], input: None,
                kind: Perform(Untap(subject: Chosen((kind: Creature, controller: You))))),
            (id: "pump", bind: [], input: None,
                kind: Perform(PumpTarget(power: 2, toughness: 2,
                    subject: Chosen((kind: Creature, controller: You))))),
            (id: "dwarf_gate", bind: [], input: None, kind: Choose(
                choice: "dwarf_gate", chooser: Controller, optional: false,
                branches: [(
                    branch_id: "dwarf", presentation: OracleLines([1]), cost: None,
                    requirement: GameCondition(ObjectMatches(
                        object: ChosenTarget(group_index: 0, target_index: 0),
                        filter: (kind: Creature, required_subtypes: ["Dwarf"]))),
                    body: (steps: [
                        (id: "choose_equipment", bind: ["selected_equipment"], input: None,
                            kind: Perform(ChoosePermanents(
                                chooser: Controller,
                                filter: (kind: AnyPermanent, controller: You,
                                    required_subtypes: ["Equipment"]),
                                min: 0, max: 1,
                                constraints: [EquipmentAttachableTo(recipient:
                                    ChosenTarget(group_index: 0, target_index: 0))]))),
                        (id: "attach_equipment", bind: [], input: Some("selected_equipment"),
                            kind: Perform(AttachEquipment(
                                equipment: PreviousEffectObject,
                                creature: Chosen((kind: Creature, controller: You))))),
                    ], yields: []),
                )], otherwise: (steps: [], yields: []))),
        ], yields: []), target_bindings: [
            (instruction_id: "untap", role: Target, target_group_index: 0),
            (instruction_id: "pump", role: Target, target_group_index: 0),
            (instruction_id: "attach_equipment", role: Target, target_group_index: 0),
        ])"#,
    )
    .expect("the independently recorded pre-alias Vow program parses");
    let expected_vow = CompiledProgram::compile_with_targeting(
        vow_owner,
        EffectContext::Spell,
        &pre_migration_vow,
        vow_face.targeting.as_ref(),
    )
    .expect("the pre-alias Vow program compiles");
    assert_eq!(
        &expected_vow, registered_vow,
        "Vow's full compiled owner must preserve its explicit legacy filters, coordinates, and bindings"
    );

    let drowner = registry
        .get("floodpits_drowner")
        .expect("Floodpits Drowner must be registered");
    let drowner_face = drowner.primary_face();
    let etb = &drowner_face.triggered_abilities[0];
    let etb_program = etb
        .program
        .as_ref()
        .expect("Floodpits Drowner's ETB has an authored Program");
    let etb_owner = ProgramOwnerId::ability(AbilityDefinitionId {
        card_id: "floodpits_drowner".into(),
        face_id: drowner_face.face_id.clone(),
        ability_path: vec![etb.ability_id.clone()],
    });
    let registered_etb = registry
        .compiled_program_for_owner(&etb_owner)
        .expect("Floodpits Drowner's ETB has a compiled Program");
    let pre_migration_etb: CardProgram = ron::from_str(
        r#"(body: (steps: [
            (id: "tap_target", bind: [], input: None,
                kind: Perform(Tap(subject: Chosen((kind: Creature, controller: Opponent))))),
            (id: "put_stun_counter", bind: [], input: None,
                kind: Perform(PutCounters(counter: Stun, count: 1,
                    subject: Chosen((kind: Creature, controller: Opponent))))),
        ], yields: []), target_bindings: [
            (instruction_id: "tap_target", role: Target, target_group_index: 0),
            (instruction_id: "put_stun_counter", role: Target, target_group_index: 0),
        ])"#,
    )
    .expect("the independently recorded pre-alias Floodpits ETB parses");
    let expected_etb = CompiledProgram::compile_with_targeting(
        etb_owner,
        EffectContext::Ability,
        &pre_migration_etb,
        etb.targeting.as_ref(),
    )
    .expect("the pre-alias Floodpits ETB compiles");
    assert_eq!(
        &expected_etb, registered_etb,
        "Floodpits ETB's full compiled owner must preserve its explicit legacy filters, coordinates, and bindings"
    );

    assert_eq!(vow_program.target_aliases.len(), 1);
    assert_eq!(etb_program.target_aliases.len(), 1);
}
