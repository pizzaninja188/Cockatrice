//! Registry conformance for the queue-v39 targeted utility-land cohort.
use tricerules_cards::primitives::{AbilityCost, Keyword, SpellEffectKind};
use tricerules_cards::{AbilityPresentation, Layout};

struct Card {
    id: &'static str,
    name: &'static str,
    cost: &'static str,
    power: i32,
    toughness: i32,
    keywords: &'static [Keyword],
}

const CARDS: &[Card] = &[
    Card {
        id: "daru_encampment",
        name: "Daru Encampment",
        cost: "{W}",
        power: 1,
        toughness: 1,
        keywords: &[],
    },
    Card {
        id: "goblin_burrows",
        name: "Goblin Burrows",
        cost: "{1}{R}",
        power: 2,
        toughness: 0,
        keywords: &[],
    },
    Card {
        id: "okina,_temple_to_the_grandfathers",
        name: "Okina, Temple to the Grandfathers",
        cost: "{G}",
        power: 1,
        toughness: 1,
        keywords: &[],
    },
    Card {
        id: "shinka,_the_bloodsoaked_keep",
        name: "Shinka, the Bloodsoaked Keep",
        cost: "{R}",
        power: 0,
        toughness: 0,
        keywords: &[Keyword::FirstStrike],
    },
    Card {
        id: "slayers_stronghold",
        name: "Slayers' Stronghold",
        cost: "{R}{W}",
        power: 2,
        toughness: 0,
        keywords: &[Keyword::Vigilance, Keyword::Haste],
    },
    Card {
        id: "sunhome,_fortress_of_the_legion",
        name: "Sunhome, Fortress of the Legion",
        cost: "{2}{R}{W}",
        power: 0,
        toughness: 0,
        keywords: &[Keyword::DoubleStrike],
    },
];

#[test]
fn q39_targeted_utility_lands_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for card in CARDS {
        let definition = registry
            .get(card.id)
            .unwrap_or_else(|| panic!("missing {}", card.id));
        assert_eq!(definition.layout, Layout::Normal, "{}", card.id);
        assert_eq!(registry.id_for_name(card.name), Some(card.id));
        let f = definition.primary_face();
        let face = card.id.replace(',', "").replace(' ', "_");
        assert_eq!(f.face_id.as_str(), face);
        assert_eq!(f.name, card.name);
        assert_eq!(f.types, ["Land"]);
        assert_eq!(f.activated_abilities.len(), 2);
        assert_eq!(
            f.activated_abilities[0].presentation,
            AbilityPresentation::OracleLines(vec![1])
        );
        assert!(matches!(
            &f.activated_abilities[0].costs[..],
            [AbilityCost::Tap]
        ));
        assert_eq!(
            f.activated_abilities[1].presentation,
            AbilityPresentation::OracleLines(vec![2])
        );
        assert!(
            matches!(&f.activated_abilities[1].costs[0], AbilityCost::Mana(cost) if cost.to_string() == card.cost)
        );
        assert!(matches!(
            &f.activated_abilities[1].costs[1],
            AbilityCost::Tap
        ));
        let ability = &f.activated_abilities[1];
        assert!(matches!(
            ability.targeting.as_ref().map(|t| t.groups.len()),
            Some(1)
        ));
        let has_pump = ability.effect.iter().any(|effect| matches!(effect, SpellEffectKind::PumpTarget { power, toughness, .. } if *power == card.power && *toughness == card.toughness));
        if card.power != 0 || card.toughness != 0 || card.keywords.is_empty() {
            // Sunhome and Shinka have only a keyword grant.
            if !(card.id.starts_with("sunhome") || card.id.starts_with("shinka")) {
                assert!(has_pump, "{}: printed pump", card.id);
            }
        }
        for keyword in card.keywords {
            assert!(ability.effect.iter().any(|effect| matches!(effect, SpellEffectKind::GrantKeywords { keywords, .. } if keywords.contains(keyword))), "{}: {:?}", card.id, keyword);
        }
    }
}
