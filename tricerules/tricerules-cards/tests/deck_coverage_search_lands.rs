use tricerules_cards::primitives::{
    CardTypeFilter, PlayerRecipient, SearchDestination, SpellEffectKind,
};
use tricerules_cards::{CardRegistry, Color, Layout};

fn assert_search_defaults(
    card_id: &str,
    face_id: &str,
    expected_name: &str,
    expected_destination: SearchDestination,
) -> tricerules_cards::primitives::ZoneCardFilter {
    let registry = CardRegistry::global();
    let card = registry
        .get(card_id)
        .unwrap_or_else(|| panic!("missing {card_id}"));
    assert_eq!(card.name, expected_name, "{card_id}");
    assert_eq!(card.layout, Layout::Normal, "{card_id}");
    assert_eq!(card.face_count(), 1, "{card_id}");

    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), face_id, "{card_id}");
    assert_eq!(face.types, ["Sorcery"], "{card_id}");
    assert_eq!(face.mana_cost.to_string(), "{1}{G}", "{card_id}");
    assert_eq!(face.colors(), [Color::Green], "{card_id}");
    assert_eq!(face.spell_effect.len(), 1, "{card_id}");
    let SpellEffectKind::SearchLibrary {
        who,
        optional,
        count,
        filter: Some(filter),
        destination,
        shuffle,
        reveal,
        ..
    } = &face.spell_effect[0]
    else {
        panic!("{card_id} has a filtered SearchLibrary instruction");
    };
    assert_eq!(*who, PlayerRecipient::Controller, "{card_id}");
    assert!(
        !optional,
        "{card_id} does not let its controller decline to search"
    );
    assert_eq!(*count, 1, "{card_id}");
    assert_eq!(*destination, expected_destination, "{card_id}");
    assert!(*shuffle, "{card_id}");
    assert!(!reveal, "{card_id} does not reveal the found card");
    filter.clone()
}

#[test]
fn nature_s_lore_searches_for_a_forest_subtype_and_enters_untapped() {
    assert_eq!(
        CardRegistry::global().id_for_name("Nature's Lore"),
        Some("natures_lore")
    );
    let filter = assert_search_defaults(
        "natures_lore",
        "nature_s_lore",
        "Nature's Lore",
        SearchDestination::Battlefield { tapped: false },
    );
    assert_eq!(filter.required_subtypes, ["Forest"]);
    assert_eq!(filter.card_type, None);
}

#[test]
fn three_visits_searches_for_a_forest_subtype_and_enters_untapped() {
    assert_eq!(
        CardRegistry::global().id_for_name("Three Visits"),
        Some("three_visits")
    );
    let filter = assert_search_defaults(
        "three_visits",
        "three_visits",
        "Three Visits",
        SearchDestination::Battlefield { tapped: false },
    );
    assert_eq!(filter.required_subtypes, ["Forest"]);
    assert_eq!(filter.card_type, None);
}

#[test]
fn rampant_growth_searches_for_a_basic_land_and_enters_tapped() {
    assert_eq!(
        CardRegistry::global().id_for_name("Rampant Growth"),
        Some("rampant_growth")
    );
    let filter = assert_search_defaults(
        "rampant_growth",
        "rampant_growth",
        "Rampant Growth",
        SearchDestination::Battlefield { tapped: true },
    );
    assert_eq!(filter.card_type, Some(CardTypeFilter::BasicLand));
}

#[test]
fn farseek_uses_four_or_branches_for_land_subtypes_and_enters_tapped() {
    assert_eq!(
        CardRegistry::global().id_for_name("Farseek"),
        Some("farseek")
    );
    let filter = assert_search_defaults(
        "farseek",
        "farseek",
        "Farseek",
        SearchDestination::Battlefield { tapped: true },
    );
    assert!(filter.card_type.is_none());
    assert_eq!(filter.required_subtypes, Vec::<String>::new());
    let branches = filter
        .any_of
        .expect("Farseek's listed land types are alternatives");
    assert_eq!(
        branches
            .iter()
            .map(|branch| branch.required_subtypes.as_slice())
            .collect::<Vec<_>>(),
        vec![
            &["Plains".to_string()][..],
            &["Island".to_string()][..],
            &["Swamp".to_string()][..],
            &["Mountain".to_string()][..],
        ]
    );
    assert!(branches.iter().all(|branch| branch.any_of.is_none()));
}
