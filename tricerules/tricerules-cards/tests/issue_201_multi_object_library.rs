use tricerules_cards::primitives::{
    AbilityCost, CounterKind, EffectSubject, Keyword, SpellEffectKind, TargetController, TargetKind,
};
use tricerules_cards::{Amount, CardRegistry};

#[test]
fn floodpits_drowner_has_current_oracle_shape() {
    let definition = CardRegistry::global()
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
    assert_eq!(etb.effect.len(), 2);
    assert!(matches!(etb.effect[0], SpellEffectKind::Tap { .. }));
    assert!(matches!(
        etb.effect[1],
        SpellEffectKind::PutCounters {
            counter: CounterKind::Stun,
            count: Amount::Fixed(1),
            ..
        }
    ));

    let activated = &face.activated_abilities[0];
    assert_eq!(activated.costs.len(), 2);
    assert!(matches!(&activated.costs[0], AbilityCost::Mana(cost) if cost.to_string() == "{1}{U}"));
    assert!(matches!(activated.costs[1], AbilityCost::Tap));
    let [SpellEffectKind::ShufflePermanentsIntoOwnersLibraries { subjects }] =
        activated.effect.as_slice()
    else {
        panic!("activated ability must use the atomic multi-subject primitive");
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
    assert_eq!(targeting.groups[0].effect_indices, [0]);
}
