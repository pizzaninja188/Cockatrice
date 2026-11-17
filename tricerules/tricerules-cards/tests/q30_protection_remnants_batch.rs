//! Registry conformance for the queue-v30 protection remnants.
use tricerules_cards::primitives::{
    AbilityCost, Amount, Color, Keyword, ProtectionQuality, SpellEffectKind,
};
use tricerules_cards::Layout;

struct Card {
    id: &'static str,
    name: &'static str,
    mana: &'static str,
    types: &'static [&'static str],
    body: (u32, u32),
    protection: Color,
    keywords: &'static [Keyword],
    cycling: bool,
    fingerprint: &'static str,
}

const CARDS: &[Card] = &[
    Card {
        id: "cemetery_gate",
        name: "Cemetery Gate",
        mana: "{2}{B}",
        types: &["Creature", "Wall"],
        body: (0, 5),
        protection: Color::Black,
        keywords: &[Keyword::Defender],
        cycling: false,
        fingerprint: "e12c7f16b0df76dbb307215ebee223d10c5ae552684503e1f64255b7bbd2e828",
    },
    Card {
        id: "wall_of_light",
        name: "Wall of Light",
        mana: "{2}{W}",
        types: &["Creature", "Wall"],
        body: (1, 5),
        protection: Color::Black,
        keywords: &[Keyword::Defender],
        cycling: false,
        fingerprint: "e12c7f16b0df76dbb307215ebee223d10c5ae552684503e1f64255b7bbd2e828",
    },
    Card {
        id: "vodalian_zombie",
        name: "Vodalian Zombie",
        mana: "{U}{B}",
        types: &["Creature", "Merfolk", "Zombie"],
        body: (2, 2),
        protection: Color::Green,
        keywords: &[],
        cycling: false,
        fingerprint: "8bfac064ac724bf8731c621a14bc72d33741119079031d61424e5ebe6f3be7b2",
    },
    Card {
        id: "zombie_outlander",
        name: "Zombie Outlander",
        mana: "{U}{B}",
        types: &["Creature", "Zombie", "Scout"],
        body: (2, 2),
        protection: Color::Green,
        keywords: &[],
        cycling: false,
        fingerprint: "8bfac064ac724bf8731c621a14bc72d33741119079031d61424e5ebe6f3be7b2",
    },
    Card {
        id: "voice_of_reason",
        name: "Voice of Reason",
        mana: "{3}{W}",
        types: &["Creature", "Angel"],
        body: (2, 2),
        protection: Color::Blue,
        keywords: &[Keyword::Flying],
        cycling: false,
        fingerprint: "611928e93658a7877576af0a0d6ea0a250c3f8ee1fe83381c3f707b9f399de49",
    },
    Card {
        id: "windreaper_falcon",
        name: "Windreaper Falcon",
        mana: "{1}{R}{G}",
        types: &["Creature", "Bird"],
        body: (1, 1),
        protection: Color::Blue,
        keywords: &[Keyword::Flying],
        cycling: false,
        fingerprint: "611928e93658a7877576af0a0d6ea0a250c3f8ee1fe83381c3f707b9f399de49",
    },
    Card {
        id: "darkwatch_elves",
        name: "Darkwatch Elves",
        mana: "{2}{G}",
        types: &["Creature", "Elf"],
        body: (2, 2),
        protection: Color::Black,
        keywords: &[],
        cycling: true,
        fingerprint: "73702a8fa84d2064c843b0bf870c9db62c769a84906cb53994d426bd72fc9c73",
    },
    Card {
        id: "disciple_of_grace",
        name: "Disciple of Grace",
        mana: "{1}{W}",
        types: &["Creature", "Human", "Cleric"],
        body: (1, 2),
        protection: Color::Black,
        keywords: &[],
        cycling: true,
        fingerprint: "73702a8fa84d2064c843b0bf870c9db62c769a84906cb53994d426bd72fc9c73",
    },
];

#[test]
fn q30_protection_remnants_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for card in CARDS {
        let definition = registry
            .get(card.id)
            .unwrap_or_else(|| panic!("missing {}", card.id));
        let f = definition.primary_face();
        assert_eq!(registry.id_for_name(card.name), Some(card.id));
        assert_eq!(f.face_id.as_str(), card.id, "{}", card.id);
        assert_eq!(f.name, card.name, "{}", card.id);
        assert_eq!(f.mana_cost.to_string(), card.mana, "{}", card.id);
        assert_eq!(f.types, card.types, "{}", card.id);
        assert_eq!(
            (f.power, f.toughness),
            (Some(card.body.0), Some(card.body.1)),
            "{}",
            card.id
        );
        assert_eq!(definition.layout, Layout::Normal, "{}", card.id);
        assert_eq!(
            f.protections,
            [ProtectionQuality::Color(card.protection)],
            "{}",
            card.id
        );
        assert_eq!(f.keywords, card.keywords, "{}", card.id);
        assert!(f.triggered_abilities.is_empty(), "{}", card.id);
        assert!(f.static_abilities.is_empty(), "{}", card.id);
        if card.cycling {
            let [ability] = f.activated_abilities.as_slice() else {
                panic!("{}: exactly one cycling ability", card.id);
            };
            assert_eq!(
                ability.costs,
                [
                    AbilityCost::Mana(tricerules_cards::mana::ManaCost::parse("{2}").unwrap()),
                    AbilityCost::DiscardSelf
                ],
                "{}",
                card.id
            );
            assert_eq!(
                ability.source_zone,
                tricerules_cards::primitives::AbilitySourceZone::Hand,
                "{}",
                card.id
            );
            assert!(
                matches!(
                    ability.effect.as_slice(),
                    [SpellEffectKind::Draw {
                        count: Amount::Fixed(1),
                        ..
                    }]
                ),
                "{}: cycling draws exactly one card",
                card.id
            );
        } else {
            assert!(f.activated_abilities.is_empty(), "{}", card.id);
        }
    }
}

#[test]
fn q30_protection_remnants_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for card in CARDS {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{}\t", card.id)))
            .unwrap_or_else(|| panic!("missing fingerprint row for {}", card.id));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[0], card.id);
        assert_eq!(fields[1], card.name);
        assert_eq!(fields[2], card.id.replace('-', "_"));
        assert_eq!(
            fields[4], card.fingerprint,
            "fingerprint drift for {}",
            card.id
        );
    }
}
