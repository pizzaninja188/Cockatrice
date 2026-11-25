//! Registry conformance for the queue-v80 02-protection-keywords cohort.
//!
//! Each card is a normal-layout creature whose complete rules text is a printed keyword list plus
//! color protections. Fingerprints are checked after the presentation metadata refresh.

use tricerules_cards::primitives::ProtectionQuality;
use tricerules_cards::{Color, Keyword, Layout};

struct Card {
    id: &'static str,
    face_id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    types: &'static [&'static str],
    supertypes: &'static [&'static str],
    power: u32,
    toughness: u32,
    keywords: &'static [Keyword],
    protections: &'static [Color],
    fingerprint: &'static str,
}

fn cases() -> Vec<Card> {
    vec![
        Card {
            id: "iridescent_angel",
            face_id: "iridescent_angel",
            name: "Iridescent Angel",
            mana_cost: "{5}{W}{U}",
            types: &["Creature", "Angel"],
            supertypes: &[],
            power: 4,
            toughness: 4,
            keywords: &[Keyword::Flying],
            protections: &[
                Color::White,
                Color::Blue,
                Color::Black,
                Color::Red,
                Color::Green,
            ],
            fingerprint: "930777381e1d1bb9ed17f96c62bad998d26c36ff6fd8a78c147a57c7a1e470db",
        },
        Card {
            id: "oversoul_of_dusk",
            face_id: "oversoul_of_dusk",
            name: "Oversoul of Dusk",
            mana_cost: "{G/W}{G/W}{G/W}{G/W}{G/W}",
            types: &["Creature", "Spirit", "Avatar"],
            supertypes: &[],
            power: 5,
            toughness: 5,
            keywords: &[],
            protections: &[Color::Blue, Color::Black, Color::Red],
            fingerprint: "6bcae98d70ab3cca459e92b6cb78574690fe8e849d395fb28e62ed8f6b59225d",
        },
        Card {
            id: "sabertooth_nishoba",
            face_id: "sabertooth_nishoba",
            name: "Sabertooth Nishoba",
            mana_cost: "{4}{G}{W}",
            types: &["Creature", "Cat", "Beast", "Warrior"],
            supertypes: &[],
            power: 5,
            toughness: 5,
            keywords: &[Keyword::Trample],
            protections: &[Color::Blue, Color::Red],
            fingerprint: "628062a49835e744b37573637191ee7407e3db306091e1d7e9241f91d8ac0a59",
        },
        Card {
            id: "sphinx_of_the_steel_wind",
            face_id: "sphinx_of_the_steel_wind",
            name: "Sphinx of the Steel Wind",
            mana_cost: "{5}{W}{U}{B}",
            types: &["Artifact", "Creature", "Sphinx"],
            supertypes: &[],
            power: 6,
            toughness: 6,
            keywords: &[
                Keyword::Flying,
                Keyword::FirstStrike,
                Keyword::Vigilance,
                Keyword::Lifelink,
            ],
            protections: &[Color::Red, Color::Green],
            fingerprint: "c9e410178aceaaf6cad1b2b87d8cec58dd4e8304246e67a50ef27fc9ef5ea018",
        },
        Card {
            id: "akroma,_angel_of_wrath",
            face_id: "akroma_angel_of_wrath",
            name: "Akroma, Angel of Wrath",
            mana_cost: "{5}{W}{W}{W}",
            types: &["Creature", "Angel"],
            supertypes: &["Legendary"],
            power: 6,
            toughness: 6,
            keywords: &[
                Keyword::Flying,
                Keyword::FirstStrike,
                Keyword::Vigilance,
                Keyword::Trample,
                Keyword::Haste,
            ],
            protections: &[Color::Black, Color::Red],
            fingerprint: "649f63f3751e3e803d81bbed52a0d94e07f3f59acf025a1aa2dcac4f8b86a4f5",
        },
    ]
}

#[test]
fn q80_protection_map_definitions() {
    let registry = tricerules_cards::registry::global();
    for card in cases() {
        let definition = registry
            .get(card.id)
            .unwrap_or_else(|| panic!("missing {}", card.id));
        assert_eq!(definition.layout, Layout::Normal, "{}", card.id);
        assert_eq!(
            registry.id_for_name(card.name),
            Some(card.id),
            "{}",
            card.id
        );
        let f = definition.primary_face();
        assert_eq!(f.face_id.as_str(), card.face_id, "{} face id", card.id);
        assert_eq!(f.name, card.name, "{} name", card.id);
        assert_eq!(f.mana_cost.to_string(), card.mana_cost, "{} mana", card.id);
        assert_eq!(f.types, card.types, "{} types", card.id);
        assert_eq!(f.supertypes, card.supertypes, "{} supertypes", card.id);
        assert_eq!(f.power, Some(card.power), "{} power", card.id);
        assert_eq!(f.toughness, Some(card.toughness), "{} toughness", card.id);
        assert_eq!(f.keywords, card.keywords, "{} keywords", card.id);
        let expected: Vec<ProtectionQuality> = card
            .protections
            .iter()
            .copied()
            .map(ProtectionQuality::Color)
            .collect();
        assert_eq!(f.protections, expected, "{} protections", card.id);
        assert!(f.static_abilities.is_empty(), "{}", card.id);
        assert!(f.activated_abilities.is_empty(), "{}", card.id);
        assert!(f.triggered_abilities.is_empty(), "{}", card.id);
        assert!(f.spell_effect.is_empty(), "{}", card.id);
    }
}

#[test]
fn q80_protection_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for card in cases() {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{}\t", card.id)))
            .unwrap_or_else(|| panic!("missing fingerprint row for {}", card.id));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[0], card.id);
        assert_eq!(fields[1], card.name);
        assert_eq!(fields[2], card.face_id, "face id drift for {}", card.id);
        assert_eq!(
            fields[4], card.fingerprint,
            "fingerprint drift for {}",
            card.id
        );
    }
}
