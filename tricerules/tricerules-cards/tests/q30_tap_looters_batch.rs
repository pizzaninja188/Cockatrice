//! Registry conformance for the queue-v30 tap-looter cohort.
use tricerules_cards::primitives::{
    AbilityCost, Amount, DiscardQuantity, InstructionKind, SpellEffectKind,
};
use tricerules_cards::Layout;

struct Card {
    id: &'static str,
    name: &'static str,
    mana: &'static str,
    types: &'static [&'static str],
    power: Option<u32>,
    toughness: Option<u32>,
    draw: u32,
    discard: u32,
    fingerprint: &'static str,
}

const CARDS: &[Card] = &[
    Card {
        id: "thought_courier",
        name: "Thought Courier",
        mana: "{1}{U}",
        types: &["Creature", "Human", "Wizard"],
        power: Some(1),
        toughness: Some(1),
        draw: 1,
        discard: 1,
        fingerprint: "8637575a3fa5971c99f20f79d93d4f3c1248248df8a78d5c943f11f8c54f639f",
    },
    Card {
        id: "merfolk_looter",
        name: "Merfolk Looter",
        mana: "{1}{U}",
        types: &["Creature", "Merfolk", "Rogue"],
        power: Some(1),
        toughness: Some(1),
        draw: 1,
        discard: 1,
        fingerprint: "8637575a3fa5971c99f20f79d93d4f3c1248248df8a78d5c943f11f8c54f639f",
    },
    Card {
        id: "bazaar_of_baghdad",
        name: "Bazaar of Baghdad",
        mana: "",
        types: &["Land"],
        power: None,
        toughness: None,
        draw: 2,
        discard: 3,
        fingerprint: "776e19e4b887a7a885d417afb1e9ebaa6259ff4e747949b6c664cbeba13fea01",
    },
    Card {
        id: "magus_of_the_bazaar",
        name: "Magus of the Bazaar",
        mana: "{1}{U}",
        types: &["Creature", "Human", "Wizard"],
        power: Some(0),
        toughness: Some(1),
        draw: 2,
        discard: 3,
        fingerprint: "776e19e4b887a7a885d417afb1e9ebaa6259ff4e747949b6c664cbeba13fea01",
    },
];

#[test]
fn q30_tap_looters_batch_maps_definitions() {
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
            (card.power, card.toughness),
            "{}",
            card.id
        );
        assert_eq!(definition.layout, Layout::Normal, "{}", card.id);
        assert!(f.keywords.is_empty(), "{}", card.id);
        assert!(f.triggered_abilities.is_empty(), "{}", card.id);
        assert!(f.static_abilities.is_empty(), "{}", card.id);
        assert!(f.spell_effect.is_empty(), "{}", card.id);
        let [ability] = f.activated_abilities.as_slice() else {
            panic!("{}: exactly one activated ability", card.id);
        };
        assert_eq!(ability.costs, [AbilityCost::Tap], "{}", card.id);
        assert!(ability.effect.is_empty(), "{}", card.id);
        let program = ability.program.as_ref().expect("draw/discard program");
        let [draw, discard] = program.body.steps.as_slice() else {
            panic!("{}: exactly two program steps", card.id);
        };
        assert!(
            matches!(
                &draw.kind,
                InstructionKind::Perform(op)
                    if matches!(**op, SpellEffectKind::Draw { count: Amount::Fixed(n), .. } if n == card.draw)
            ),
            "{}: draw count",
            card.id
        );
        assert!(
            matches!(
                &discard.kind,
                InstructionKind::Perform(op)
                    if matches!(
                        **op,
                        SpellEffectKind::Discard { quantity: DiscardQuantity::Exact(n), .. } if n == card.discard
                    )
            ),
            "{}: discard count",
            card.id
        );
        assert!(f.targeting.is_none(), "{}", card.id);
    }
}

#[test]
fn q30_tap_looters_batch_fingerprints_match_the_pinned_oracle_text() {
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
