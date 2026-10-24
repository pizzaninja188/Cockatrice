use tricerules_cards::primitives::{
    AbilityCost, CounterKind, EffectSubject, InstructionKind, Keyword, ProgramTargetRole,
    SpellEffectKind, TargetController, TargetKind,
};
use tricerules_cards::Amount;

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
                subject: EffectSubject::Chosen(filter)
            } if filter.kind == TargetKind::Creature
                && filter.controller == TargetController::Opponent)
    ));
    assert!(matches!(
        &etb_program.body.steps[1].kind,
        InstructionKind::Perform(effect)
            if matches!(effect.as_ref(), SpellEffectKind::PutCounters {
                counter: CounterKind::Stun,
                count: Amount::Fixed(1),
                subject: EffectSubject::Chosen(filter),
            } if filter.kind == TargetKind::Creature
                && filter.controller == TargetController::Opponent)
    ));
    assert_eq!(etb_program.target_bindings.len(), 2);
    assert!(etb_program.target_bindings.iter().all(|binding| {
        binding.role == ProgramTargetRole::Target && binding.target_group_index == 0
    }));
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
