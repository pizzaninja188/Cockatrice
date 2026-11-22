//! Registry conformance for the queue-v28 keyword-only creature cohort.
//!
//! Each card is a normal-layout printed creature with only printed keyword abilities.
//! Expected values are copied from the pinned Oracle text; fingerprints are SHA-256 of
//! the exact normalized Oracle lines.

use tricerules_cards::Layout;

struct Card {
    id: &'static str,
    name: &'static str,
    mana: &'static str,
    supertypes: &'static [&'static str],
    types: &'static [&'static str],
    power: u32,
    toughness: u32,
    keywords: &'static [&'static str],
    fingerprint: &'static str,
}

const CARDS: &[Card] = &[
    Card {
        id: "thunder_spirit",
        name: "Thunder Spirit",
        mana: "{1}{W}{W}",
        supertypes: &[],
        types: &["Creature", "Elemental", "Spirit"],
        power: 2,
        toughness: 2,
        keywords: &["Flying", "FirstStrike"],
        fingerprint: "2a9c4dc21c8d50d6a00f89af1771a356fbbc0e16380fd1e772207f3986278a6c",
    },
    Card {
        id: "sabertooth_wyvern",
        name: "Sabertooth Wyvern",
        mana: "{4}{R}",
        supertypes: &[],
        types: &["Creature", "Drake"],
        power: 3,
        toughness: 2,
        keywords: &["Flying", "FirstStrike"],
        fingerprint: "2a9c4dc21c8d50d6a00f89af1771a356fbbc0e16380fd1e772207f3986278a6c",
    },
    Card {
        id: "carnivorous_plant",
        name: "Carnivorous Plant",
        mana: "{3}{G}",
        supertypes: &[],
        types: &["Creature", "Plant", "Wall"],
        power: 4,
        toughness: 5,
        keywords: &["Defender"],
        fingerprint: "157ddc59b5e4e7ca9a05ffa09e4fa7cdefe073d8a3ebdf56b70a8446b64ae83f",
    },
    Card {
        id: "ambush_party",
        name: "Ambush Party",
        mana: "{4}{R}",
        supertypes: &[],
        types: &["Creature", "Human", "Rogue"],
        power: 3,
        toughness: 1,
        keywords: &["FirstStrike", "Haste"],
        fingerprint: "be7356e100ced8aba533139a8fb0f8faea26775ded869ffb740f2657e63af7b1",
    },
    Card {
        id: "raging_minotaur",
        name: "Raging Minotaur",
        mana: "{2}{R}{R}",
        supertypes: &[],
        types: &["Creature", "Minotaur", "Berserker"],
        power: 3,
        toughness: 3,
        keywords: &["Haste"],
        fingerprint: "de3b46da4da8a98c381a34e6ef1326411f01962e4bb094b007055b41e1d37ce4",
    },
    Card {
        id: "brimstone_dragon",
        name: "Brimstone Dragon",
        mana: "{6}{R}{R}",
        supertypes: &[],
        types: &["Creature", "Dragon"],
        power: 6,
        toughness: 6,
        keywords: &["Flying", "Haste"],
        fingerprint: "9e9e7cdf08e4e4260bc39b2f0dc6996ba88b5582f79e3b0540c724daac45fb80",
    },
    Card {
        id: "windseeker_centaur",
        name: "Windseeker Centaur",
        mana: "{1}{R}{R}",
        supertypes: &[],
        types: &["Creature", "Centaur"],
        power: 2,
        toughness: 2,
        keywords: &["Vigilance"],
        fingerprint: "7b186aaf815451ab3e721bb76dd8520e11c31a6f4b35688c8ffdf1a560625a06",
    },
    Card {
        id: "angel_of_light",
        name: "Angel of Light",
        mana: "{4}{W}",
        supertypes: &[],
        types: &["Creature", "Angel"],
        power: 3,
        toughness: 3,
        keywords: &["Flying", "Vigilance"],
        fingerprint: "771dc95bfdda131d16a82140c13461c4c334ac5742ce78e278c4943d23b5db37",
    },
    Card {
        id: "mugato",
        name: "Mugato",
        mana: "{3}{G}{G}",
        supertypes: &[],
        types: &["Creature", "Ape", "Beast"],
        power: 6,
        toughness: 6,
        keywords: &["Reach", "Vigilance", "Deathtouch"],
        fingerprint: "61631989f8f476fd0c64956407358c2ecf1ad66e53b7210b589df35169b15f36",
    },
    Card {
        id: "blor_the_impervious",
        name: "Blor the Impervious",
        mana: "{3}{G}{G}",
        supertypes: &["Legendary"],
        types: &["Creature", "Frog", "Warrior"],
        power: 3,
        toughness: 3,
        keywords: &["Trample", "Hexproof", "Indestructible"],
        fingerprint: "788c371be7f8a391e546c5ddad65849bb61cb5718fe53ad28024f9c27be1e6cc",
    },
];

fn face<'a>(
    registry: &'a tricerules_cards::CardRegistry,
    id: &str,
) -> &'a tricerules_cards::CardFace {
    registry
        .get(id)
        .unwrap_or_else(|| panic!("missing {id}"))
        .primary_face()
}

#[test]
fn keyword_only_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for card in CARDS {
        let definition = registry
            .get(card.id)
            .unwrap_or_else(|| panic!("missing {}", card.id));
        assert_eq!(
            registry.id_for_name(card.name),
            Some(card.id),
            "{}",
            card.id
        );
        let f = face(registry, card.id);
        assert_eq!(f.face_id.as_str(), card.id, "{} face id", card.id);
        assert_eq!(f.name, card.name, "{} name", card.id);
        assert_eq!(f.mana_cost.to_string(), card.mana, "{} mana", card.id);
        assert_eq!(f.supertypes, card.supertypes, "{} supertypes", card.id);
        assert_eq!(f.types, card.types, "{} types", card.id);
        assert_eq!(f.power, Some(card.power), "{} power", card.id);
        assert_eq!(f.toughness, Some(card.toughness), "{} toughness", card.id);
        assert_eq!(definition.layout, Layout::Normal, "{}", card.id);
        assert!(f.activated_abilities.is_empty(), "{}", card.id);
        assert!(f.triggered_abilities.is_empty(), "{}", card.id);
        assert!(f.static_abilities.is_empty(), "{}", card.id);
        assert!(f.spell_effect.is_empty(), "{}", card.id);
        assert!(f.targeting.is_none(), "{}", card.id);
        assert_eq!(f.keywords.len(), card.keywords.len(), "{}", card.id);
        for keyword in card.keywords {
            assert!(
                f.keywords
                    .iter()
                    .any(|value| format!("{value:?}") == *keyword),
                "{} keyword {keyword}",
                card.id
            );
        }
    }
}

#[test]
fn keyword_only_batch_fingerprints_match_the_pinned_oracle_text() {
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
        assert_eq!(fields[2], card.id);
        assert_eq!(
            fields[4], card.fingerprint,
            "fingerprint drift for {}",
            card.id
        );
    }
}

#[test]
fn keyword_only_batch_uses_only_the_printed_keyword_vocabulary() {
    for card in CARDS {
        for keyword in card.keywords {
            assert!(
                matches!(
                    *keyword,
                    "Flying"
                        | "Reach"
                        | "Vigilance"
                        | "Lifelink"
                        | "Haste"
                        | "Deathtouch"
                        | "Menace"
                        | "Trample"
                        | "FirstStrike"
                        | "DoubleStrike"
                        | "Indestructible"
                        | "Hexproof"
                        | "Shroud"
                        | "Defender"
                        | "Flash"
                ),
                "{}: unsupported keyword {keyword}",
                card.id
            );
        }
    }
}
