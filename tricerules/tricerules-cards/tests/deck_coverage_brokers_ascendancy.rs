use tricerules_cards::primitives::{
    CastTriggerPlayer, CreatureScopeController, CreatureScopeFilter, SpellEffectKind,
};
use tricerules_cards::{Amount, Color, CounterKind, Layout, TriggerCondition};

#[test]
fn brokers_ascendancy_registers_its_characteristics_and_end_step_mapping() {
    let registry = tricerules_cards::registry::global();
    assert_eq!(
        registry.id_for_name("Brokers Ascendancy"),
        Some("brokers_ascendancy")
    );
    let card = registry
        .get("brokers_ascendancy")
        .expect("Brokers Ascendancy");
    assert_eq!(card.name, "Brokers Ascendancy");
    assert_eq!(card.layout, Layout::Normal);
    assert_eq!(card.face_count(), 1);

    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), "brokers_ascendancy");
    assert_eq!(face.mana_cost.to_string(), "{G}{W}{U}");
    assert_eq!(face.types, ["Enchantment"]);
    assert!(face.supertypes.is_empty());
    assert!(face.keywords.is_empty());
    let colors = face.colors();
    assert_eq!(colors.len(), 3);
    assert!(colors.contains(&Color::Green));
    assert!(colors.contains(&Color::White));
    assert!(colors.contains(&Color::Blue));
    assert!(face.spell_effect.is_empty());
    assert!(face.static_abilities.is_empty());
    assert!(face.activated_abilities.is_empty());

    let [ability] = face.triggered_abilities.as_slice() else {
        panic!("Brokers Ascendancy has exactly one triggered ability");
    };
    assert_eq!(ability.ability_id.as_str(), "triggered_01");
    assert!(matches!(
        &ability.presentation,
        tricerules_cards::AbilityPresentation::OracleLines(lines) if lines == &[1]
    ));
    assert_eq!(
        ability.trigger,
        TriggerCondition::AtBeginningOfEndStep {
            player: CastTriggerPlayer::Controller,
        }
    );
    assert_eq!(
        ability.effect.as_slice(),
        [
            SpellEffectKind::PutCountersAll {
                counter: CounterKind::PlusOnePlusOne,
                count: Amount::Fixed(1),
                filter: CreatureScopeFilter {
                    controller: Some(CreatureScopeController::YouControl),
                    ..CreatureScopeFilter::default()
                },
            },
            SpellEffectKind::PutCountersAllPlaneswalkers {
                counter: CounterKind::Loyalty,
                count: Amount::Fixed(1),
                exclude_self: false,
            },
        ]
    );
    assert!(ability.targeting.is_none());
    assert!(!ability.may);
}
