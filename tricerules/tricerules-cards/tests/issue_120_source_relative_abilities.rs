use tricerules_cards::primitives::{EffectSubject, SpellEffectKind, TargetController, TargetKind};
use tricerules_cards::{AbilityCost, CardRegistry, CounterKind};

#[test]
fn issue_120_creatures_use_source_excluding_sacrifice_costs() {
    for (id, activation_mana) in [("hungry_ghoul", "{1}"), ("unburied_earthcarver", "{2}")] {
        let definition = CardRegistry::global().get(id).expect("card is registered");
        assert!(definition.partial.is_none());
        let face = definition.primary_face();
        assert_eq!((face.power, face.toughness), (Some(2), Some(2)));
        let [ability] = face.activated_abilities.as_slice() else {
            panic!("card has one activated ability");
        };
        let [AbilityCost::Mana(mana), AbilityCost::SacrificePermanent { filter }] =
            ability.costs.as_slice()
        else {
            panic!("ability uses mana plus a selected sacrifice");
        };
        assert_eq!(mana.to_string(), activation_mana);
        assert_eq!(filter.kind, TargetKind::Creature);
        assert_eq!(filter.controller, TargetController::You);
        assert!(filter
            .excluded_objects
            .contains(&tricerules_cards::TargetObjectExclusion::Source));
        assert!(matches!(
            ability.effect.as_slice(),
            [SpellEffectKind::PutCounters {
                counter: CounterKind::PlusOnePlusOne,
                count: 1,
                subject: EffectSubject::Source,
            }]
        ));
    }
}

#[test]
fn issue_120_wingspan_stride_returns_its_untargeted_source() {
    let definition = CardRegistry::global()
        .get("wingspan_stride")
        .expect("Wingspan Stride is registered");
    assert!(definition.partial.is_none());
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{U}");
    assert_eq!(face.types, ["Enchantment", "Aura"]);
    let [ability] = face.activated_abilities.as_slice() else {
        panic!("Wingspan Stride has one activated ability");
    };
    assert!(matches!(
        ability.effect.as_slice(),
        [SpellEffectKind::ReturnToOwnersHand {
            subject: EffectSubject::Source,
        }]
    ));
    assert!(!ability.effect[0].needs_target());
}
