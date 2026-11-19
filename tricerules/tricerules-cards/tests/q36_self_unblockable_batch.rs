//! Registry conformance for the queue-v36 self-unblockable cohort.
use tricerules_cards::mana::ManaCost;
use tricerules_cards::primitives::{AbilityCost, CombatRestrictionScope, SpellEffectKind};
use tricerules_cards::Layout;

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
        id: "agent_of_horizons",
        name: "Agent of Horizons",
        mana_cost: "{2}{G}",
        fingerprint: "e20581e2e092a89b553f16c9acf29b6586fb728baa660aa0e7aad69913cfb332",
        types: &["Creature", "Human", "Rogue"],
        power: 3,
        toughness: 2,
        activation: "{2}{U}",
    },
    Card {
        id: "ashioks_skulker",
        name: "Ashiok's Skulker",
        mana_cost: "{4}{U}",
        fingerprint: "3492c03b23db7555d93ab6fe64a5c206341273e6aee6e503e54eb99def3cadea",
        types: &["Creature", "Nightmare"],
        power: 3,
        toughness: 5,
        activation: "{3}{U}",
    },
    Card {
        id: "blockade_runner",
        name: "Blockade Runner",
        mana_cost: "{3}{U}",
        fingerprint: "3efab26b813d4bfcde89a338d7e2424ccb92e37445656d28e7affbd94879b2f2",
        types: &["Creature", "Merfolk"],
        power: 2,
        toughness: 2,
        activation: "{U}",
    },
];

#[test]
fn q36_self_unblockable_batch_maps_definitions() {
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
                SpellEffectKind::ApplyCombatRestriction { scope, restriction }
                    if *scope == CombatRestrictionScope::Source
                        && restriction.cant_be_blocked
            ),
            "{}: source-only unblockable restriction",
            card.id
        );
    }
}

#[test]
fn q36_self_unblockable_batch_fingerprints_match_the_pinned_oracle_text() {
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
