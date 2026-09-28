use tricerules_cards::primitives::{
    AbilityCost, AbilitySourceZone, CardTypeFilter, SearchDestination, SpellEffectKind,
    ZoneCardFilter,
};
use tricerules_cards::{AbilityPresentation, CardRegistry, Layout};

#[test]
fn urzas_cave_registers_its_full_type_line_and_both_activated_abilities() {
    let registry = CardRegistry::global();
    let card = registry.get("urzas_cave").expect("Urza's Cave");

    assert_eq!(card.name, "Urza's Cave");
    assert_eq!(registry.id_for_name("Urza's Cave"), Some("urzas_cave"));
    assert_eq!(card.layout, Layout::Normal);
    assert_eq!(card.face_count(), 1);

    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), "urzas_cave");
    assert_eq!(face.mana_cost.to_string(), "");
    assert_eq!(face.types, ["Land", "Urza's", "Cave"]);
    assert!(face.colors().is_empty());
    assert!(face.spell_effect.is_empty());

    let [mana, search] = face.activated_abilities.as_slice() else {
        panic!("Urza's Cave has one mana ability and one land-search ability");
    };
    assert_eq!(mana.ability_id.as_str(), "activated_01");
    assert_eq!(mana.presentation, AbilityPresentation::OracleLines(vec![1]));
    assert_eq!(mana.source_zone, AbilitySourceZone::Battlefield);
    assert!(matches!(mana.costs.as_slice(), [AbilityCost::Tap]));
    assert!(matches!(
        mana.effect.as_slice(),
        [SpellEffectKind::ProduceMana { options, .. }]
            if options.as_slice()
                == [tricerules_cards::primitives::ManaAmount {
                    c: 1,
                    ..Default::default()
                }]
    ));

    assert_eq!(search.ability_id.as_str(), "activated_02");
    assert_eq!(
        search.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(search.source_zone, AbilitySourceZone::Battlefield);
    assert!(matches!(
        search.costs.as_slice(),
        [AbilityCost::Mana(cost), AbilityCost::Tap, AbilityCost::SacrificeSelf]
            if cost.to_string() == "{3}"
    ));
    assert!(matches!(
        search.effect.as_slice(),
        [SpellEffectKind::SearchLibrary {
            filter: Some(ZoneCardFilter {
                card_type: Some(CardTypeFilter::Land),
                ..
            }),
            destination: SearchDestination::Battlefield { tapped: true },
            shuffle: true,
            reveal: false,
            ..
        }]
    ));
}
