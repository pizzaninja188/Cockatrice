use tricerules_card_model::{AbilityDefinitionId, ProgramOwnerId};

#[test]
fn issue_507_replicating_ring_threshold_trigger_is_program_backed() {
    let registry = tricerules_cards::registry::global();
    let face = registry
        .get("replicating_ring")
        .and_then(|card| card.face(0))
        .expect("the exact Replicating Ring face is registered");
    let [trigger] = face.triggered_abilities.as_slice() else {
        panic!("Replicating Ring has one upkeep trigger");
    };
    assert!(
        trigger.effect.is_empty(),
        "the Ring trigger no longer duplicates its Program leaves in a legacy effect list"
    );
    assert!(
        trigger.program.is_some(),
        "the Ring trigger owns the complete threshold and token Program"
    );

    let owner = ProgramOwnerId::ability(AbilityDefinitionId {
        card_id: "replicating_ring".into(),
        face_id: face.face_id.clone(),
        ability_path: vec![trigger.ability_id.clone()],
    });
    let compiled = registry
        .compiled_program_for_owner(&owner)
        .expect("the exact triggered ability has a compiled owner");
    assert_eq!(
        compiled.context,
        tricerules_cards::primitives::EffectContext::Ability
    );
    assert_eq!(compiled.body.steps.len(), 2);
}
