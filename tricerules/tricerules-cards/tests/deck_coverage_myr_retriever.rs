use tricerules_cards::TriggerCondition;

#[test]
fn myr_retriever_registers_its_targeted_death_trigger() {
    let registry = tricerules_cards::registry::global();
    let card = registry.get("myr_retriever").expect("Myr Retriever");
    let face = card.primary_face();
    assert_eq!(face.name, "Myr Retriever");
    assert_eq!(face.mana_cost.to_string(), "{2}");
    assert_eq!(face.types, ["Artifact", "Creature", "Myr"]);
    assert_eq!((face.power, face.toughness), (Some(1), Some(1)));

    let [trigger] = face.triggered_abilities.as_slice() else {
        panic!("Myr Retriever has one dies trigger");
    };
    assert_eq!(trigger.trigger, TriggerCondition::WhenSelfDies);
    let targeting = trigger.targeting.as_ref().expect("mandatory target group");
    let [group] = targeting.groups.as_slice() else {
        panic!("Myr Retriever has one target group");
    };
    assert_eq!((group.min, group.max), (1, 1));
    assert_eq!(
        group.prompt,
        "Choose target artifact card from your graveyard"
    );
    assert_eq!(group.effect_indices, [0]);
}
