//! Vorinclex, Monstrous Raider's registered identity, keywords, and counter replacements.
use tricerules_cards::{AbilityPresentation, Keyword};

const VORINCLEX: &str = "vorinclex,_monstrous_raider";

#[test]
fn vorinclex_exact_identity_body_keywords_and_static_lines_are_registered() {
    let registry = tricerules_cards::registry::global();
    let card = registry
        .get(VORINCLEX)
        .expect("Vorinclex, Monstrous Raider must be registered");
    assert_eq!(
        registry.id_for_name("Vorinclex, Monstrous Raider"),
        Some(VORINCLEX)
    );
    assert_eq!(card.id, VORINCLEX);
    assert_eq!(card.name, "Vorinclex, Monstrous Raider");

    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), "vorinclex_monstrous_raider");
    assert_eq!(face.mana_cost.to_string(), "{4}{G}{G}");
    assert_eq!(face.supertypes, ["Legendary"]);
    assert_eq!(face.types, ["Creature", "Phyrexian", "Praetor"]);
    assert_eq!((face.power, face.toughness), (Some(6), Some(6)));
    assert!(face.keywords.contains(&Keyword::Trample));
    assert!(face.keywords.contains(&Keyword::Haste));

    assert_eq!(face.static_abilities.len(), 2);
    assert_eq!(
        face.static_abilities[0].presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(
        face.static_abilities[1].presentation,
        AbilityPresentation::OracleLines(vec![3])
    );
    let doubles = format!("{:?}", face.static_abilities[0].definition);
    assert!(
        doubles.contains("DoubleCountersYouWouldPut"),
        "unexpected line 2 static definition: {doubles}"
    );
    let halves = format!("{:?}", face.static_abilities[1].definition);
    assert!(
        halves.contains("HalveCountersOpponentsWouldPut"),
        "unexpected line 3 static definition: {halves}"
    );
}

#[test]
fn vorinclex_full_definition_is_admitted_for_ordinary_mainboard_use() {
    let card = tricerules_cards::registry::global()
        .get(VORINCLEX)
        .expect("Vorinclex, Monstrous Raider must be registered");
    assert!(
        !card.commander_setup_only,
        "the complete Vorinclex definition is admitted as an ordinary mainboard card"
    );
}
