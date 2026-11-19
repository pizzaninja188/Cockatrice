//! Registry conformance for the queue-v39 tap-ping cohort.
use tricerules_cards::primitives::{AbilityCost, Amount, SpellEffectKind, TargetKind};
use tricerules_cards::{AbilityPresentation, Layout};

struct Card {
    id: &'static str,
    name: &'static str,
    mana: &'static str,
    power: u32,
    toughness: u32,
    keywords: &'static [tricerules_cards::Keyword],
    ping_mana: Option<&'static str>,
}

const CARDS: &[Card] = &[
    Card {
        id: "mawcor",
        name: "Mawcor",
        mana: "{3}{U}{U}",
        power: 3,
        toughness: 3,
        keywords: &[tricerules_cards::Keyword::Flying],
        ping_mana: None,
    },
    Card {
        id: "thornwind_faeries",
        name: "Thornwind Faeries",
        mana: "{1}{U}{U}",
        power: 1,
        toughness: 1,
        keywords: &[tricerules_cards::Keyword::Flying],
        ping_mana: None,
    },
    Card {
        id: "vulshok_sorcerer",
        name: "Vulshok Sorcerer",
        mana: "{1}{R}{R}",
        power: 1,
        toughness: 1,
        keywords: &[tricerules_cards::Keyword::Haste],
        ping_mana: None,
    },
    Card {
        id: "cunning_sparkmage",
        name: "Cunning Sparkmage",
        mana: "{2}{R}",
        power: 0,
        toughness: 1,
        keywords: &[tricerules_cards::Keyword::Haste],
        ping_mana: None,
    },
    Card {
        id: "anaba_shaman",
        name: "Anaba Shaman",
        mana: "{3}{R}",
        power: 2,
        toughness: 2,
        keywords: &[],
        ping_mana: Some("{R}"),
    },
    Card {
        id: "jeska,_warrior_adept",
        name: "Jeska, Warrior Adept",
        mana: "{2}{R}{R}",
        power: 3,
        toughness: 1,
        keywords: &[
            tricerules_cards::Keyword::FirstStrike,
            tricerules_cards::Keyword::Haste,
        ],
        ping_mana: None,
    },
];

#[test]
fn q39_tap_ping_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for card in CARDS {
        let definition = registry
            .get(card.id)
            .unwrap_or_else(|| panic!("missing {}", card.id));
        assert_eq!(definition.layout, Layout::Normal, "{}", card.id);
        assert_eq!(registry.id_for_name(card.name), Some(card.id));
        let f = definition.primary_face();
        let face_id = card.id.replace(',', "").replace(' ', "_");
        assert_eq!(f.face_id.as_str(), face_id);
        assert_eq!(f.name, card.name);
        assert_eq!(f.mana_cost.to_string(), card.mana);
        assert_eq!(f.types.first().map(String::as_str), Some("Creature"));
        assert_eq!(
            (f.power, f.toughness),
            (Some(card.power), Some(card.toughness))
        );
        assert_eq!(f.keywords, card.keywords);
        assert_eq!(f.activated_abilities.len(), 1);
        let ping = &f.activated_abilities[0];
        let line = if card.keywords.is_empty() { 1 } else { 2 };
        assert_eq!(
            ping.presentation,
            AbilityPresentation::OracleLines(vec![line])
        );
        match card.ping_mana {
            Some(mana) => assert!(
                matches!(&ping.costs[..], [AbilityCost::Mana(cost), AbilityCost::Tap] if cost.to_string() == mana)
            ),
            None => assert!(matches!(&ping.costs[..], [AbilityCost::Tap])),
        }
        assert!(matches!(
            ping.effect.as_slice(),
            [SpellEffectKind::DamageTarget { amount: Amount::Fixed(1), target }]
                if target.kind == TargetKind::AnyTarget
        ));
    }
}
