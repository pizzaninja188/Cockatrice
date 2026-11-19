//! Registry conformance for the queue-v36 self-deathtouch cohort.
use tricerules_cards::mana::ManaCost;
use tricerules_cards::primitives::{AbilityCost, EffectSubject, SpellEffectKind};
use tricerules_cards::{Keyword, Layout};

struct Card {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    fingerprint: &'static str,
    types: &'static [&'static str],
    power: u32,
    toughness: u32,
    activation: &'static str,
}

const CARDS: &[Card] = &[
    Card {
        id: "mardu_hateblade",
        name: "Mardu Hateblade",
        mana_cost: "{W}",
        fingerprint: "d368b9f6fa41796cd19ef4075cbceac9a6fedac8bb002d12efc1a603129e29a2",
        types: &["Creature", "Human", "Warrior"],
        power: 1,
        toughness: 1,
        activation: "{B}",
    },
    Card {
        id: "narnam_cobra",
        name: "Narnam Cobra",
        mana_cost: "{2}",
        fingerprint: "5ce9e7783b38db614bb3a117c132ecc1413c4a4f87a6d779f9a102227f51994d",
        types: &["Artifact", "Creature", "Snake"],
        power: 2,
        toughness: 1,
        activation: "{G}",
    },
    Card {
        id: "skittering_heartstopper",
        name: "Skittering Heartstopper",
        mana_cost: "{B}",
        fingerprint: "ed3f0b6963dccee138a64e3cf08e399cbac9073a4a6c90a7441e5dfa1e6625e6",
        types: &["Creature", "Insect"],
        power: 1,
        toughness: 2,
        activation: "{B}",
    },
    Card {
        id: "twilight_panther",
        name: "Twilight Panther",
        mana_cost: "{W}",
        fingerprint: "ed3f0b6963dccee138a64e3cf08e399cbac9073a4a6c90a7441e5dfa1e6625e6",
        types: &["Creature", "Cat", "Spirit"],
        power: 1,
        toughness: 2,
        activation: "{B}",
    },
];

#[test]
fn q36_self_deathtouch_batch_maps_definitions() {
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
        assert_eq!(f.mana_cost.to_string(), card.mana_cost);
        assert_eq!(
            f.types.iter().map(String::as_str).collect::<Vec<_>>(),
            card.types.to_vec()
        );
        assert_eq!(
            (f.power, f.toughness),
            (Some(card.power), Some(card.toughness)),
            "{}",
            card.id
        );
        assert!(f.keywords.is_empty(), "{}", card.id);
        assert!(f.triggered_abilities.is_empty(), "{}", card.id);
        assert!(f.static_abilities.is_empty(), "{}", card.id);
        assert_eq!(f.activated_abilities.len(), 1, "{}", card.id);
        let ability = &f.activated_abilities[0];
        assert_eq!(
            ability.costs,
            vec![AbilityCost::Mana(
                ManaCost::parse(card.activation).expect("mana cost")
            )],
            "{}: printed activation cost",
            card.id
        );
        assert_eq!(ability.effect.len(), 1, "{}", card.id);
        assert!(
            matches!(
                &ability.effect[0],
                SpellEffectKind::GrantKeywords { subject: EffectSubject::Source, keywords }
                    if keywords == &[Keyword::Deathtouch]
            ),
            "{}: source-only deathtouch grant",
            card.id
        );
    }
}

#[test]
fn q36_self_deathtouch_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for card in CARDS {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{}\t", card.id)))
            .unwrap_or_else(|| panic!("missing fingerprint row for {}", card.id));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "{}", card.id);
        assert_eq!(fields[0], card.id);
        assert_eq!(fields[1], card.name);
        assert_eq!(fields[2], card.id, "face id drift for {}", card.id);
        assert_eq!(
            fields[4], card.fingerprint,
            "fingerprint drift for {}",
            card.id
        );
    }
}
