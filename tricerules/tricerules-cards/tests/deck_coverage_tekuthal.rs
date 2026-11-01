//! Exact Tekuthal identity, proliferate replacement, and counter-removal activation.
use tricerules_cards::primitives::{
    AbilityCost, Amount, CounterKind, EffectSubject, Keyword, PermanentTypeFilter, SpellEffectKind,
    StaticAbilityDef, TargetController, TargetFilter, TargetKind, TargetObjectExclusion,
};
use tricerules_cards::{AbilityPresentation, ManaCost};

#[test]
fn tekuthal_exact_identity_and_three_counter_activation() {
    let registry = tricerules_cards::registry::global();
    let id = registry
        .id_for_name("Tekuthal, Inquiry Dominus")
        .expect("Tekuthal, Inquiry Dominus must be registered");
    assert_eq!(id, "tekuthal,_inquiry_dominus");

    let card = registry.get(id).unwrap();
    assert_eq!(card.face_count(), 1);
    let face = card.primary_face();
    assert_eq!(face.name, "Tekuthal, Inquiry Dominus");
    assert_eq!(face.face_id.as_str(), "tekuthal_inquiry_dominus");
    assert_eq!(face.mana_cost.to_string(), "{2}{U}{U}");
    assert_eq!(face.supertypes, ["Legendary"]);
    assert_eq!(face.types, ["Creature", "Phyrexian", "Horror"]);
    assert_eq!(face.power, Some(3));
    assert_eq!(face.toughness, Some(5));
    assert_eq!(face.keywords, [Keyword::Flying]);

    let [proliferation] = face.static_abilities.as_slice() else {
        panic!("Tekuthal has one static ability");
    };
    assert_eq!(proliferation.ability_id.as_str(), "static_01");
    assert_eq!(
        proliferation.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(
        proliferation.definition,
        StaticAbilityDef::DoubleControllerProliferations
    );

    let [activation] = face.activated_abilities.as_slice() else {
        panic!("Tekuthal has one activated ability");
    };
    assert_eq!(activation.ability_id.as_str(), "activated_01");
    assert_eq!(
        activation.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(
        activation.costs,
        [
            AbilityCost::Mana(ManaCost::parse("{1}{U/P}{U/P}").unwrap()),
            AbilityCost::RemoveCountersFromPermanents {
                count: 3,
                counter: None,
                filter: TargetFilter {
                    kind: TargetKind::AnyPermanent,
                    controller: TargetController::You,
                    permanent_types: vec![
                        PermanentTypeFilter::Artifact,
                        PermanentTypeFilter::Creature,
                        PermanentTypeFilter::Planeswalker,
                    ],
                    excluded_objects: vec![TargetObjectExclusion::Source],
                    ..TargetFilter::default()
                },
            },
        ]
    );
    assert_eq!(
        activation.effect,
        [SpellEffectKind::PutCounters {
            counter: CounterKind::Keyword(Keyword::Indestructible),
            count: Amount::Fixed(1),
            subject: EffectSubject::Source,
        }]
    );
}
