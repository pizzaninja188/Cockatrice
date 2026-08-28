use tricerules_cards::primitives::{
    CounterKind, EffectSubject, SpellEffectKind, TargetController, TargetKind,
};
use tricerules_cards::CardRegistry;

#[test]
fn issue_85_cards_share_the_grouped_creature_damage_primitive() {
    let registry = CardRegistry::global();

    let rabid_definition = registry
        .get("rabid_bite")
        .expect("Rabid Bite is registered");
    assert!(rabid_definition.partial.is_none());
    let rabid = rabid_definition.primary_face();
    assert_eq!(rabid.mana_cost.to_string(), "{1}{G}");
    assert_eq!(rabid.types, ["Sorcery"]);
    assert!(matches!(
        &rabid.spell_effect[..],
        [SpellEffectKind::CreatureDealsDamageEqualToPower { source, target }]
            if source.kind == TargetKind::Creature
                && source.controller == TargetController::You
                && target.kind == TargetKind::Creature
                && target.controller == TargetController::NotYou
    ));
    let rabid_groups = &rabid.targeting.as_ref().expect("grouped targeting").groups;
    assert_eq!(rabid_groups.len(), 2);
    assert_eq!(rabid_groups[0].effect_indices, [0]);
    assert_eq!(rabid_groups[1].effect_indices, [0]);
    assert_eq!(rabid_groups[1].distinct_from, [0]);

    let hunter_definition = registry
        .get("hunters_edge")
        .expect("Hunter's Edge is registered");
    assert!(hunter_definition.partial.is_none());
    let hunter = hunter_definition.primary_face();
    assert_eq!(hunter.mana_cost.to_string(), "{3}{G}");
    assert_eq!(hunter.types, ["Sorcery"]);
    assert!(matches!(
        &hunter.spell_effect[..],
        [
            SpellEffectKind::PutCounters {
                counter: CounterKind::PlusOnePlusOne,
                count: 1,
                subject: EffectSubject::Chosen(source_for_counter),
            },
            SpellEffectKind::CreatureDealsDamageEqualToPower { source, target },
        ] if source_for_counter.as_ref() == source
            && source.kind == TargetKind::Creature
            && source.controller == TargetController::You
            && target.kind == TargetKind::Creature
            && target.controller == TargetController::NotYou
    ));
    let hunter_groups = &hunter.targeting.as_ref().expect("grouped targeting").groups;
    assert_eq!(hunter_groups.len(), 2);
    assert_eq!(hunter_groups[0].effect_indices, [0, 1]);
    assert_eq!(hunter_groups[1].effect_indices, [1]);
    assert_eq!(hunter_groups[1].distinct_from, [0]);
}
