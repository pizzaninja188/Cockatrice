use tricerules_cards::primitives::{
    CastTriggerPlayer, EffectSubject, GameCondition, LibraryPartitionKind, SpellCastFilter,
    SpellManaSpentComparison,
};
use tricerules_cards::{
    AbilityPresentation, Amount, CardRegistry, CounterKind, Keyword, SpellEffectKind,
    TriggerCondition,
};

fn assert_increment(ability: &tricerules_cards::TriggeredAbilityDef, oracle_line: u16) {
    assert_eq!(ability.ability_id.as_str(), "triggered_01");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![oracle_line])
    );
    assert_eq!(
        ability.trigger,
        TriggerCondition::WheneverPlayerCastsSpell {
            caster: CastTriggerPlayer::Controller,
            filter: SpellCastFilter::default(),
            ordinal: None,
            ordinal_scope: Default::default(),
        }
    );
    assert_eq!(
        ability.intervening_if,
        Some(GameCondition::TriggeringSpellManaSpent {
            comparison: SpellManaSpentComparison::GreaterThanSourcePowerOrToughness,
        })
    );
    assert_eq!(
        ability.effect,
        [SpellEffectKind::PutCounters {
            counter: CounterKind::PlusOnePlusOne,
            count: Amount::Fixed(1),
            subject: EffectSubject::Source,
        }]
    );
}

#[test]
fn issue_280_registers_the_complete_increment_cohort() {
    let registry = CardRegistry::global();

    let cuboid = registry
        .get("cuboid_colony")
        .expect("Cuboid Colony should be generated")
        .primary_face();
    assert_eq!(cuboid.face_id.as_str(), "cuboid_colony");
    assert_eq!(cuboid.name, "Cuboid Colony");
    assert_eq!(cuboid.mana_cost.to_string(), "{G}{U}");
    assert_eq!(cuboid.types, ["Creature", "Insect"]);
    assert_eq!((cuboid.power, cuboid.toughness), (Some(1), Some(1)));
    assert_eq!(
        cuboid.keywords,
        [Keyword::Flash, Keyword::Flying, Keyword::Trample]
    );
    let [cuboid_increment] = cuboid.triggered_abilities.as_slice() else {
        panic!("Cuboid Colony should have one Increment ability")
    };
    assert_increment(cuboid_increment, 3);

    let textbook = registry
        .get("textbook_tabulator")
        .expect("Textbook Tabulator should be generated")
        .primary_face();
    assert_eq!(textbook.face_id.as_str(), "textbook_tabulator");
    assert_eq!(textbook.name, "Textbook Tabulator");
    assert_eq!(textbook.mana_cost.to_string(), "{2}{U}");
    assert_eq!(textbook.types, ["Creature", "Frog", "Wizard"]);
    assert_eq!((textbook.power, textbook.toughness), (Some(0), Some(3)));
    assert!(textbook.keywords.is_empty());
    let [textbook_increment, textbook_etb] = textbook.triggered_abilities.as_slice() else {
        panic!("Textbook Tabulator should have Increment and ETB abilities")
    };
    assert_increment(textbook_increment, 1);
    assert_eq!(textbook_etb.ability_id.as_str(), "triggered_02");
    assert_eq!(
        textbook_etb.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(
        textbook_etb.trigger,
        TriggerCondition::WhenSelfEntersBattlefield
    );
    assert_eq!(
        textbook_etb.effect,
        [SpellEffectKind::LibraryPartition {
            count: 2,
            top_min: 0,
            top_max: None,
            kind: LibraryPartitionKind::Surveil,
        }]
    );
}
