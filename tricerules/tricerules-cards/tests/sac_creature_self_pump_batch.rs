//! Registry conformance for the reviewed sacrifice-a-creature self-pump cohort.
//!
//! Every included card is a normal-layout creature with a single activated
//! ability whose whole cost is `Sacrifice a creature` and whose effect pumps
//! the source by +2/+2 until end of turn. Each fingerprint is SHA-256 of the
//! exact pinned normalized Oracle text.

use tricerules_cards::primitives::{
    AbilityCost, EffectSubject, SpellEffectKind, TargetController, TargetKind,
};
use tricerules_cards::{AbilityPresentation, Layout};

struct Card {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    types: &'static [&'static str],
    power: u32,
    toughness: u32,
    fingerprint: &'static str,
}

const CARDS: &[Card] = &[
    Card {
        id: "bloodthrone_vampire",
        name: "Bloodthrone Vampire",
        mana_cost: "{1}{B}",
        types: &["Creature", "Vampire"],
        power: 1,
        toughness: 1,
        fingerprint: "3d37f4137798c0a8e165b75939eb7f2ad9db33dc3e82a8b725fbf866aaba4f30",
    },
    Card {
        id: "nantuko_husk",
        name: "Nantuko Husk",
        mana_cost: "{2}{B}",
        types: &["Creature", "Zombie", "Insect"],
        power: 2,
        toughness: 2,
        fingerprint: "3d37f4137798c0a8e165b75939eb7f2ad9db33dc3e82a8b725fbf866aaba4f30",
    },
    Card {
        id: "phyrexian_ghoul",
        name: "Phyrexian Ghoul",
        mana_cost: "{2}{B}",
        types: &["Creature", "Phyrexian", "Zombie"],
        power: 2,
        toughness: 2,
        fingerprint: "3d37f4137798c0a8e165b75939eb7f2ad9db33dc3e82a8b725fbf866aaba4f30",
    },
    Card {
        id: "vampire_aristocrat",
        name: "Vampire Aristocrat",
        mana_cost: "{2}{B}",
        types: &["Creature", "Vampire", "Rogue", "Noble"],
        power: 2,
        toughness: 2,
        fingerprint: "3d37f4137798c0a8e165b75939eb7f2ad9db33dc3e82a8b725fbf866aaba4f30",
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
fn sac_creature_self_pump_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for card in CARDS {
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
        let f = face(registry, card.id);
        assert_eq!(f.face_id.as_str(), card.id, "{} face id", card.id);
        assert_eq!(f.name, card.name, "{} name", card.id);
        assert_eq!(
            f.mana_cost.to_string(),
            card.mana_cost,
            "{} mana cost",
            card.id
        );
        assert_eq!(f.types, card.types, "{} types", card.id);
        assert!(f.supertypes.is_empty(), "{}", card.id);
        assert_eq!(f.power, Some(card.power), "{} power", card.id);
        assert_eq!(f.toughness, Some(card.toughness), "{} toughness", card.id);
        assert!(f.keywords.is_empty(), "{}", card.id);
        assert!(f.triggered_abilities.is_empty(), "{}", card.id);
        assert!(f.static_abilities.is_empty(), "{}", card.id);
        assert!(f.spell_effect.is_empty(), "{}", card.id);
        assert!(f.targeting.is_none(), "{}", card.id);

        let [ability] = f.activated_abilities.as_slice() else {
            panic!("{}: exactly one activated ability", card.id);
        };
        assert_eq!(ability.ability_id.as_str(), "activated_01", "{}", card.id);
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{} presentation",
            card.id
        );
        let [AbilityCost::SacrificePermanent { filter, count }] = ability.costs.as_slice() else {
            panic!("{}: one sacrifice cost", card.id);
        };
        assert_eq!(*count, 1, "{}", card.id);
        assert_eq!(filter.kind, TargetKind::Creature, "{}", card.id);
        assert_eq!(filter.controller, TargetController::You, "{}", card.id);
        assert!(
            matches!(
                ability.effect.as_slice(),
                [SpellEffectKind::PumpTarget {
                    power: 2,
                    toughness: 2,
                    scale: None,
                    subject: EffectSubject::Source,
                }]
            ),
            "{} pump",
            card.id
        );
        assert!(ability.targeting.is_none(), "{} no targets", card.id);
    }
}

#[test]
fn sac_creature_self_pump_batch_fingerprints_match_the_pinned_oracle_text() {
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
