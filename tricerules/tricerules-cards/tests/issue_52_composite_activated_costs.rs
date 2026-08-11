use tricerules_cards::primitives::{SpellEffectKind, TargetController, TargetKind};
use tricerules_cards::{AbilityCost, Amount, CardRegistry, Keyword};

#[test]
fn issue_52_cards_have_complete_composite_cost_definitions() {
    let registry = CardRegistry::global();
    for (id, mana, damage, target_kind) in [
        ("explosive_apparatus", "{3}", 2, TargetKind::AnyTarget),
        ("vial_of_dragonfire", "{2}", 2, TargetKind::Creature),
        ("silent_dart", "{4}", 3, TargetKind::Creature),
    ] {
        let definition = registry.get(id).unwrap();
        assert!(definition.partial.is_none(), "{id} must be full coverage");
        let face = definition.primary_face();
        assert_eq!(face.activated_abilities.len(), 1);
        let ability = &face.activated_abilities[0];
        assert!(matches!(
            ability.costs.as_slice(),
            [
                AbilityCost::Mana(_),
                AbilityCost::Tap,
                AbilityCost::SacrificeSelf
            ]
        ));
        let AbilityCost::Mana(cost) = &ability.costs[0] else {
            unreachable!()
        };
        assert_eq!(cost.to_string(), mana);
        assert!(matches!(
            &ability.effect[0],
            SpellEffectKind::DamageTarget { amount, target }
                if *amount == Amount::Fixed(damage) && target.kind == target_kind
        ));
    }

    let vine_definition = registry.get("portcullis_vine").unwrap();
    assert!(vine_definition.partial.is_none());
    let vine = vine_definition.primary_face();
    assert_eq!(vine.keywords, [Keyword::Defender]);
    let AbilityCost::SacrificePermanent { filter } = &vine.activated_abilities[0].costs[2] else {
        panic!("Portcullis Vine needs a filtered sacrifice component");
    };
    assert_eq!(filter.kind, TargetKind::Creature);
    assert_eq!(filter.controller, TargetController::You);
    assert_eq!(filter.required_keywords, [Keyword::Defender]);

    let constrictor_definition = registry.get("noose_constrictor").unwrap();
    assert!(constrictor_definition.partial.is_none());
    let constrictor = constrictor_definition.primary_face();
    assert_eq!(constrictor.keywords, [Keyword::Reach]);
    assert_eq!(
        constrictor.activated_abilities[0].costs,
        [AbilityCost::Discard]
    );
    assert!(matches!(
        constrictor.activated_abilities[0].effect.as_slice(),
        [SpellEffectKind::PumpTarget {
            power: 1,
            toughness: 1,
            ..
        }]
    ));
}
