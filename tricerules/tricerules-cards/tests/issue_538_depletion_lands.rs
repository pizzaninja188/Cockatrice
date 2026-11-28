//! Registry conformance for the depletion-counter lands (#538).
use tricerules_cards::primitives::{
    AbilityCost, Amount, CounterKind, EffectSubject, GameCondition, ManaAbilityRider,
    SpellEffectKind, StaticAbilityDef, TriggerCondition,
};
use tricerules_cards::{registry, AbilityPresentation, Layout, ManaAmount};

fn mana(symbol: char, count: u32) -> ManaAmount {
    let mut amount = ManaAmount::default();
    match symbol {
        'W' => amount.w = count,
        'U' => amount.u = count,
        'B' => amount.b = count,
        'R' => amount.r = count,
        'G' => amount.g = count,
        _ => unreachable!(),
    }
    amount
}

const TWO_COUNTER_LANDS: [(&str, &str, char); 5] = [
    ("hickory_woodlot", "Hickory Woodlot", 'G'),
    ("remote_farm", "Remote Farm", 'W'),
    ("peat_bog", "Peat Bog", 'B'),
    ("sandstone_needle", "Sandstone Needle", 'R'),
    ("saprazzan_skerry", "Saprazzan Skerry", 'U'),
];

const RECHARGING_LANDS: [(&str, &str, char, char); 5] = [
    ("veldt", "Veldt", 'G', 'W'),
    ("lava_tubes", "Lava Tubes", 'B', 'R'),
    ("river_delta", "River Delta", 'U', 'B'),
    ("timberline_ridge", "Timberline Ridge", 'R', 'G'),
    ("land_cap", "Land Cap", 'W', 'U'),
];

fn uncharged() -> GameCondition {
    GameCondition::SourceCounterCount {
        counter: CounterKind::Depletion,
        min: None,
        max: Some(0),
    }
}

#[test]
fn depletion_lands_are_typed_mana_abilities_with_source_riders() {
    for (id, name, color) in TWO_COUNTER_LANDS {
        let definition = registry::global().get(id).expect("registered");
        assert_eq!(definition.layout, Layout::Normal);
        assert_eq!(registry::global().id_for_name(name), Some(id));
        let face = definition.primary_face();
        assert_eq!(face.name, name);
        assert_eq!(face.types, ["Land"], "{name}");
        assert!(face.triggered_abilities.is_empty(), "{name}");
        let [tapped, counters] = face.static_abilities.as_slice() else {
            panic!("{name}: enters tapped with counters");
        };
        assert!(matches!(
            tapped.definition,
            StaticAbilityDef::EntersTapped { .. }
        ));
        assert!(matches!(
            counters.definition,
            StaticAbilityDef::EntersWithCounters {
                counter: CounterKind::Depletion,
                amount: Amount::Fixed(2),
                ..
            }
        ));
        let [ability] = face.activated_abilities.as_slice() else {
            panic!("{name}: one mana ability");
        };
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![2])
        );
        assert!(matches!(
            ability.costs.as_slice(),
            [
                AbilityCost::Tap,
                AbilityCost::RemoveCounters {
                    counter: Some(CounterKind::Depletion),
                    count: 1,
                    ..
                }
            ]
        ));
        assert!(ability.is_mana_ability(), "{name}: CR 605.1a mana ability");
        assert_eq!(
            ability.mana_options(),
            Some(&vec![mana(color, 2)]),
            "{name}"
        );
        assert_eq!(
            ability.mana_ability_riders(),
            [ManaAbilityRider::SacrificeSourceWithoutCounters {
                counter: CounterKind::Depletion
            }],
            "{name}"
        );
        assert!(matches!(
            &ability.effect[1],
            SpellEffectKind::Conditional { condition, effect }
                if *condition == uncharged()
                    && matches!(**effect, SpellEffectKind::Sacrifice { subject: EffectSubject::Source })
        ));
    }

    for (id, name, first, second) in RECHARGING_LANDS {
        let definition = registry::global().get(id).expect("registered");
        assert_eq!(registry::global().id_for_name(name), Some(id));
        let face = definition.primary_face();
        assert_eq!(face.types, ["Land"], "{name}");
        let [untap] = face.static_abilities.as_slice() else {
            panic!("{name}: one untap restriction");
        };
        assert_eq!(
            untap.presentation,
            AbilityPresentation::OracleLines(vec![1])
        );
        assert!(matches!(
            &untap.definition,
            StaticAbilityDef::SelfDoesntUntapDuringUntapStepUnless { condition }
                if *condition == uncharged()
        ));
        let [upkeep] = face.triggered_abilities.as_slice() else {
            panic!("{name}: one upkeep trigger");
        };
        assert_eq!(
            upkeep.presentation,
            AbilityPresentation::OracleLines(vec![2])
        );
        assert!(matches!(
            upkeep.trigger,
            TriggerCondition::AtBeginningOfUpkeep { .. }
        ));
        assert!(matches!(
            upkeep.effect.as_slice(),
            [SpellEffectKind::RemoveCounters {
                counter: CounterKind::Depletion,
                count: 1,
                subject: EffectSubject::Source,
            }]
        ));
        let [ability] = face.activated_abilities.as_slice() else {
            panic!("{name}: one mana ability");
        };
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![3])
        );
        assert_eq!(ability.costs, [AbilityCost::Tap]);
        assert!(ability.is_mana_ability(), "{name}: CR 605.1a mana ability");
        assert_eq!(
            ability.mana_options(),
            Some(&vec![mana(first, 1), mana(second, 1)]),
            "{name}"
        );
        assert_eq!(
            ability.mana_ability_riders(),
            [ManaAbilityRider::PutSourceCounters {
                counter: CounterKind::Depletion,
                count: 1
            }],
            "{name}"
        );
    }
}
