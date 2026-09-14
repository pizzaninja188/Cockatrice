use tricerules_cards::primitives::{CardTypeFilter, RelativePlayerSet};
use tricerules_cards::{
    AbilityPresentation, BattlefieldAggregate, CardRegistry, SpellCostModifier, SpellEffectKind,
    TriggerCondition,
};

#[test]
fn issue_277_affinity_artifact_cards_have_exact_registry_shapes() {
    let registry = CardRegistry::global();

    let memory = registry
        .get("memory_guardian")
        .expect("Memory Guardian should be generated");
    let memory_face = memory.primary_face();
    assert_eq!(memory_face.mana_cost.to_string(), "{4}{U}");
    assert_eq!(
        memory_face.types,
        ["Artifact", "Creature", "Robot", "Artificer"]
    );
    assert_eq!(memory_face.keywords, [tricerules_cards::Keyword::Flying]);
    assert_affinity_for_artifacts(memory_face.cost_modifiers.as_slice());

    let valkyrie = registry
        .get("valkyrie_aerial_unit")
        .expect("Valkyrie Aerial Unit should be generated");
    let valkyrie_face = valkyrie.primary_face();
    assert_eq!(valkyrie_face.mana_cost.to_string(), "{5}{U}{U}");
    assert_eq!(valkyrie_face.types, ["Artifact", "Creature", "Construct"]);
    assert_eq!(valkyrie_face.keywords, [tricerules_cards::Keyword::Flying]);
    assert_affinity_for_artifacts(valkyrie_face.cost_modifiers.as_slice());
    let [trigger] = valkyrie_face.triggered_abilities.as_slice() else {
        panic!("Valkyrie Aerial Unit should retain its ETB ability");
    };
    assert_eq!(
        trigger.presentation,
        AbilityPresentation::OracleLines(vec![3])
    );
    assert_eq!(trigger.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert_eq!(
        trigger.effect,
        [SpellEffectKind::LibraryPartition {
            count: 2,
            top_min: 0,
            top_max: None,
            kind: tricerules_cards::LibraryPartitionKind::Surveil,
        }]
    );
}

fn assert_affinity_for_artifacts(modifiers: &[SpellCostModifier]) {
    let [SpellCostModifier::BattlefieldCountGenericReduction {
        amount_per_match,
        filter,
        aggregate,
    }] = modifiers
    else {
        panic!("expected one affinity-for-artifacts cost modifier");
    };
    assert_eq!(*amount_per_match, 1);
    assert_eq!(filter.controllers, RelativePlayerSet::Controller);
    assert_eq!(filter.card_type, Some(CardTypeFilter::Artifact));
    assert_eq!(*aggregate, BattlefieldAggregate::Count);
    assert!(filter.required_subtypes.is_empty());
}
