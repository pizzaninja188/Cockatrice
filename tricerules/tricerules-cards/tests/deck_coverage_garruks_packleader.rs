use tricerules_cards::primitives::{
    Amount, CastTriggerPlayer, CreatureEventFilter, PermanentTypeFilter, PlayerRecipient,
    PowerComparison, SpellEffectKind, TriggerCondition,
};
use tricerules_cards::{AbilityPresentation, CardRegistry, Layout};

#[test]
fn garruks_packleader_registers_its_power_filtered_optional_draw_trigger() {
    let registry = CardRegistry::global();
    let card = registry
        .get("garruks_packleader")
        .expect("Garruk's Packleader registry definition");

    assert_eq!(card.name, "Garruk's Packleader");
    assert_eq!(
        registry.id_for_name("Garruk's Packleader"),
        Some("garruks_packleader")
    );
    assert_eq!(card.layout, Layout::Normal);
    assert_eq!(card.face_count(), 1);

    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), "garruks_packleader");
    assert_eq!(face.mana_cost.to_string(), "{4}{G}");
    assert_eq!(face.types, ["Creature", "Beast"]);
    assert_eq!(face.power, Some(4));
    assert_eq!(face.toughness, Some(4));
    assert!(face.spell_effect.is_empty());
    assert!(face.activated_abilities.is_empty());

    let [ability] = face.triggered_abilities.as_slice() else {
        panic!("Garruk's Packleader has exactly one triggered ability");
    };
    assert!(matches!(
        &ability.presentation,
        AbilityPresentation::OracleLines(lines) if lines == &[1]
    ));
    assert!(matches!(
        &ability.trigger,
        TriggerCondition::WheneverPermanentEntersBattlefield {
            controller: CastTriggerPlayer::Controller,
            filter,
            creature_filter: Some(CreatureEventFilter {
                power: Some(PowerComparison::AtLeast(3)),
                ..
            }),
        } if filter.permanent_type == Some(PermanentTypeFilter::Creature)
            && filter.exclude_source
    ));
    assert!(matches!(
        ability.effect.as_slice(),
        [SpellEffectKind::Draw {
            who: PlayerRecipient::Controller,
            count: Amount::Fixed(1),
        }]
    ));
    assert!(ability.may);
}
