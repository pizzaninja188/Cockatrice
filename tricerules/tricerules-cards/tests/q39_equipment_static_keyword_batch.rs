//! Registry conformance for the queue-v39 fixed Equipment cohort.
use tricerules_cards::primitives::{Keyword, SpellEffectKind, StaticAbilityDef};
use tricerules_cards::{AbilityCost, AbilityPresentation, Layout};

struct Card {
    id: &'static str,
    name: &'static str,
    mana: &'static str,
    power: i32,
    toughness: i32,
    keywords: &'static [Keyword],
    equip: &'static str,
}

const CARDS: &[Card] = &[
    Card {
        id: "kitesail",
        name: "Kitesail",
        mana: "{2}",
        power: 1,
        toughness: 0,
        keywords: &[Keyword::Flying],
        equip: "{2}",
    },
    Card {
        id: "aeronauts_wings",
        name: "Aeronaut's Wings",
        mana: "{2}",
        power: 1,
        toughness: 0,
        keywords: &[Keyword::Flying],
        equip: "{2}",
    },
    Card {
        id: "cobbled_wings",
        name: "Cobbled Wings",
        mana: "{2}",
        power: 0,
        toughness: 0,
        keywords: &[Keyword::Flying],
        equip: "{1}",
    },
    Card {
        id: "neurok_hoversail",
        name: "Neurok Hoversail",
        mana: "{1}",
        power: 0,
        toughness: 0,
        keywords: &[Keyword::Flying],
        equip: "{2}",
    },
    Card {
        id: "rangers_longbow",
        name: "Ranger's Longbow",
        mana: "{1}{G}",
        power: 2,
        toughness: 1,
        keywords: &[Keyword::Reach],
        equip: "{3}",
    },
    Card {
        id: "spidersilk_net",
        name: "Spidersilk Net",
        mana: "{0}",
        power: 0,
        toughness: 2,
        keywords: &[Keyword::Reach],
        equip: "{2}",
    },
    Card {
        id: "behemoth_sledge",
        name: "Behemoth Sledge",
        mana: "{1}{G}{W}",
        power: 2,
        toughness: 2,
        keywords: &[Keyword::Trample, Keyword::Lifelink],
        equip: "{3}",
    },
    Card {
        id: "loxodon_warhammer",
        name: "Loxodon Warhammer",
        mana: "{3}",
        power: 3,
        toughness: 0,
        keywords: &[Keyword::Trample, Keyword::Lifelink],
        equip: "{3}",
    },
    Card {
        id: "viridian_claw",
        name: "Viridian Claw",
        mana: "{2}",
        power: 1,
        toughness: 0,
        keywords: &[Keyword::FirstStrike],
        equip: "{1}",
    },
    Card {
        id: "bladed_pinions",
        name: "Bladed Pinions",
        mana: "{2}",
        power: 0,
        toughness: 0,
        keywords: &[Keyword::Flying, Keyword::FirstStrike],
        equip: "{2}",
    },
];

#[test]
fn q39_equipment_static_keyword_maps_definitions() {
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
        assert_eq!(f.types, ["Artifact", "Equipment"]);
        assert_eq!((f.power, f.toughness), (None, None));
        assert!(f.keywords.is_empty());
        assert!(f.spell_effect.is_empty());
        assert!(f.triggered_abilities.is_empty());
        assert_eq!(f.static_abilities.len(), 1);
        let static_ability = &f.static_abilities[0];
        assert_eq!(
            static_ability.presentation,
            AbilityPresentation::OracleLines(vec![1])
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
        assert_eq!(f.activated_abilities.len(), 1);
        let activated = &f.activated_abilities[0];
        assert_eq!(
            activated.presentation,
            AbilityPresentation::OracleLines(vec![2])
        );
        assert_eq!(activated.costs.len(), 1);
        assert!(
            matches!(&activated.costs[0], AbilityCost::Mana(cost) if cost.to_string() == card.equip)
        );
        assert!(
            matches!(activated.effect.as_slice(), [SpellEffectKind::Equip { target }] if target.controller == tricerules_cards::primitives::TargetController::You)
        );
    }
}
