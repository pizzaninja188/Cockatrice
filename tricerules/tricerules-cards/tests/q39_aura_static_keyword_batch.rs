//! Registry conformance for the queue-v39 Aura static keyword cohort.
use tricerules_cards::primitives::{Keyword, SpellEffectKind, StaticAbilityDef};
use tricerules_cards::{AbilityPresentation, Layout};

struct Card {
    id: &'static str,
    name: &'static str,
    mana: &'static str,
    power: i32,
    toughness: i32,
    keywords: &'static [Keyword],
}

const CARDS: &[Card] = &[
    Card {
        id: "wings_of_aesthir",
        name: "Wings of Aesthir",
        mana: "{W}{U}",
        power: 1,
        toughness: 0,
        keywords: &[Keyword::Flying, Keyword::FirstStrike],
    },
    Card {
        id: "prodigious_growth",
        name: "Prodigious Growth",
        mana: "{4}{G}{G}",
        power: 7,
        toughness: 7,
        keywords: &[Keyword::Trample],
    },
    Card {
        id: "candlelight_vigil",
        name: "Candlelight Vigil",
        mana: "{3}{W}",
        power: 3,
        toughness: 2,
        keywords: &[Keyword::Vigilance],
    },
    Card {
        id: "mark_of_the_vampire",
        name: "Mark of the Vampire",
        mana: "{3}{B}",
        power: 2,
        toughness: 2,
        keywords: &[Keyword::Lifelink],
    },
    Card {
        id: "gift_of_orzhova",
        name: "Gift of Orzhova",
        mana: "{1}{W/B}{W/B}",
        power: 1,
        toughness: 1,
        keywords: &[Keyword::Flying, Keyword::Lifelink],
    },
    Card {
        id: "mythic_proportions",
        name: "Mythic Proportions",
        mana: "{4}{G}{G}{G}",
        power: 8,
        toughness: 8,
        keywords: &[Keyword::Trample],
    },
];

#[test]
fn q39_aura_static_keyword_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for card in CARDS {
        let definition = registry
            .get(card.id)
            .unwrap_or_else(|| panic!("missing {}", card.id));
        assert_eq!(definition.layout, Layout::Normal, "{}", card.id);
        assert_eq!(registry.id_for_name(card.name), Some(card.id));
        let f = definition.primary_face();
        assert_eq!(f.face_id.as_str(), card.id);
        assert_eq!(f.name, card.name);
        assert_eq!(f.mana_cost.to_string(), card.mana);
        assert_eq!(f.types, ["Enchantment", "Aura"]);
        assert_eq!((f.power, f.toughness), (None, None));
        assert!(f.keywords.is_empty());
        assert!(f.activated_abilities.is_empty());
        assert!(f.triggered_abilities.is_empty());
        assert!(matches!(
            f.spell_effect.as_slice(),
            [SpellEffectKind::AuraAttach { .. }]
        ));
        assert_eq!(f.static_abilities.len(), 1);
        let static_ability = &f.static_abilities[0];
        assert_eq!(
            static_ability.presentation,
            AbilityPresentation::OracleLines(vec![2])
        );
        let StaticAbilityDef::AttachedModifier {
            delta_power,
            delta_toughness,
            keywords,
            ..
        } = &static_ability.definition
        else {
            panic!("{}: expected AttachedModifier", card.id);
        };
        assert_eq!(
            (*delta_power, *delta_toughness),
            (card.power, card.toughness)
        );
        assert_eq!(keywords, card.keywords);
    }
}
