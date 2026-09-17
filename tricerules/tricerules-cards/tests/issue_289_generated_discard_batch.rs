use tricerules_cards::primitives::{
    Amount, CastTriggerPlayer, CounterKind, EffectSubject, SpellEffectKind, TriggerCondition,
};
use tricerules_cards::{
    AbilityCost, AbilityPresentation, AbilitySourceZone, CardRegistry, Keyword,
};

#[test]
fn issue_289_registers_both_cards_with_exact_discard_batch_triggers() {
    let registry = CardRegistry::global();
    let cases = [
        (
            "scrounging_skyray",
            "Scrounging Skyray",
            "{1}{U}",
            vec!["Creature", "Fish", "Pirate"],
            (1, 2),
            2,
            3,
            vec![Keyword::Flying],
        ),
        (
            "marauding_mako",
            "Marauding Mako",
            "{R}",
            vec!["Creature", "Shark", "Pirate"],
            (1, 1),
            1,
            2,
            Vec::new(),
        ),
    ];
    for (id, name, mana_cost, types, stats, trigger_line, cycling_line, keywords) in cases {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.name, name);
        assert_eq!(registry.id_for_name(name), Some(id));
        let face = definition.primary_face();
        assert_eq!(face.face_id.as_str(), id);
        assert_eq!(face.mana_cost.to_string(), mana_cost);
        assert_eq!(face.types, types);
        assert_eq!(face.power.zip(face.toughness), Some(stats));
        assert_eq!(face.keywords, keywords);

        let [ability] = face.triggered_abilities.as_slice() else {
            panic!("{id} must have exactly one triggered ability");
        };
        assert_eq!(ability.ability_id.as_str(), "triggered_01");
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![trigger_line])
        );
        assert_eq!(
            ability.trigger,
            TriggerCondition::WheneverPlayerDiscardsOneOrMoreCards {
                player: CastTriggerPlayer::Controller,
            }
        );
        assert_eq!(
            ability.effect,
            [SpellEffectKind::PutCounters {
                counter: CounterKind::PlusOnePlusOne,
                count: Amount::EventCount,
                subject: EffectSubject::Source,
            }]
        );

        let [cycling] = face.activated_abilities.as_slice() else {
            panic!("{id} must have exactly one Cycling ability");
        };
        assert_eq!(cycling.ability_id.as_str(), "activated_01");
        assert_eq!(
            cycling.presentation,
            AbilityPresentation::OracleLines(vec![cycling_line])
        );
        assert_eq!(cycling.source_zone, AbilitySourceZone::Hand);
        assert!(matches!(
            cycling.costs.as_slice(),
            [AbilityCost::Mana(cost), AbilityCost::DiscardSelf] if cost.to_string() == "{2}"
        ));
        assert!(matches!(
            cycling.effect.as_slice(),
            [SpellEffectKind::Draw { .. }]
        ));
    }
}
