use tricerules_cards::primitives::{
    AbilityCost, CardTypeFilter, PlayerRecipient, SearchDestination, SpellEffectKind,
};
use tricerules_cards::{AbilityPresentation, CardRegistry, Color, Layout};

#[test]
fn sakura_tribe_elder_registers_its_complete_sacrifice_search_ability() {
    let registry = CardRegistry::global();
    assert_eq!(
        registry.id_for_name("Sakura-Tribe Elder"),
        Some("sakura-tribe_elder")
    );
    let card = registry
        .get("sakura-tribe_elder")
        .expect("Sakura-Tribe Elder");
    assert_eq!(card.name, "Sakura-Tribe Elder");
    assert_eq!(card.layout, Layout::Normal);
    assert_eq!(card.face_count(), 1);

    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), "sakura_tribe_elder");
    assert_eq!(face.types, ["Creature", "Snake", "Shaman"]);
    assert_eq!(face.mana_cost.to_string(), "{1}{G}");
    assert_eq!(face.colors(), [Color::Green]);
    assert_eq!(face.power, Some(1));
    assert_eq!(face.toughness, Some(1));

    let [ability] = face.activated_abilities.as_slice() else {
        panic!("Sakura-Tribe Elder has one activated ability");
    };
    assert_eq!(ability.ability_id.as_str(), "activated_01");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(ability.costs, [AbilityCost::SacrificeSelf]);

    let [SpellEffectKind::SearchLibrary {
        who,
        optional,
        count,
        filter: Some(filter),
        destination,
        shuffle,
        reveal,
        ..
    }] = ability.effect.as_slice()
    else {
        panic!("the ability searches for one basic land");
    };
    assert_eq!(*who, PlayerRecipient::Controller);
    assert!(!optional);
    assert_eq!(*count, 1);
    assert_eq!(filter.card_type, Some(CardTypeFilter::BasicLand));
    assert_eq!(
        *destination,
        SearchDestination::Battlefield { tapped: true }
    );
    assert!(*shuffle);
    assert!(!reveal);
}
