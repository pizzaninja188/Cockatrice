//! Registry conformance for the reviewed power-two unblockable cohort.
//!
//! Every included card is a normal-layout creature with a `{T}` ability that
//! grants one target creature with power 2 or less the can't-be-blocked
//! restriction for the turn. Each fingerprint is SHA-256 of the pinned lines.

use tricerules_cards::primitives::{
    AbilityCost, CombatRestriction, CombatRestrictionScope, PowerComparison, SpellEffectKind,
    TargetFilter, TargetKind,
};
use tricerules_cards::{AbilityPresentation, CardRegistry, Layout};

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
        id: "crafty_pathmage",
        name: "Crafty Pathmage",
        mana_cost: "{2}{U}",
        types: &["Creature", "Human", "Wizard"],
        power: 1,
        toughness: 1,
        fingerprint: "8ed53caf8521ebbb89fcacb97c1c4a21b772e22c0529b69095d4b6ccd05ca0c6",
    },
    Card {
        id: "dwarven_nomad",
        name: "Dwarven Nomad",
        mana_cost: "{2}{R}",
        types: &["Creature", "Dwarf", "Nomad"],
        power: 1,
        toughness: 1,
        fingerprint: "8ed53caf8521ebbb89fcacb97c1c4a21b772e22c0529b69095d4b6ccd05ca0c6",
    },
    Card {
        id: "dwarven_warriors",
        name: "Dwarven Warriors",
        mana_cost: "{2}{R}",
        types: &["Creature", "Dwarf", "Warrior"],
        power: 1,
        toughness: 1,
        fingerprint: "8ed53caf8521ebbb89fcacb97c1c4a21b772e22c0529b69095d4b6ccd05ca0c6",
    },
    Card {
        id: "goblin_tunneler",
        name: "Goblin Tunneler",
        mana_cost: "{1}{R}",
        types: &["Creature", "Goblin", "Rogue"],
        power: 1,
        toughness: 1,
        fingerprint: "8ed53caf8521ebbb89fcacb97c1c4a21b772e22c0529b69095d4b6ccd05ca0c6",
    },
    Card {
        id: "pathmaker_initiate",
        name: "Pathmaker Initiate",
        mana_cost: "{1}{R}",
        types: &["Creature", "Human", "Wizard"],
        power: 2,
        toughness: 1,
        fingerprint: "8ed53caf8521ebbb89fcacb97c1c4a21b772e22c0529b69095d4b6ccd05ca0c6",
    },
];

fn face<'a>(registry: &'a CardRegistry, id: &str) -> &'a tricerules_cards::CardFace {
    registry
        .get(id)
        .unwrap_or_else(|| panic!("missing {id}"))
        .primary_face()
}

#[test]
fn unblockable_power_two_batch_maps_definitions() {
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
        assert_eq!(f.power, Some(card.power), "{} power", card.id);
        assert_eq!(f.toughness, Some(card.toughness), "{} toughness", card.id);
        assert!(f.keywords.is_empty(), "{} no keyword", card.id);
        assert!(
            f.static_abilities.is_empty(),
            "{} no static ability",
            card.id
        );
        assert!(f.triggered_abilities.is_empty(), "{} no trigger", card.id);
        assert!(f.spell_effect.is_empty(), "{} no spell effect", card.id);

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
        assert_eq!(
            ability.costs,
            [AbilityCost::Tap],
            "{} tap-only cost",
            card.id
        );
        assert_eq!(
            ability.effect,
            [SpellEffectKind::ApplyCombatRestriction {
                scope: CombatRestrictionScope::Chosen(TargetFilter {
                    kind: TargetKind::Creature,
                    power: Some(PowerComparison::AtMost(2)),
                    ..TargetFilter::default()
                }),
                restriction: CombatRestriction {
                    cant_be_blocked: true,
                    ..CombatRestriction::default()
                },
            }],
            "{}: power-two-or-less unblockable grant",
            card.id
        );
        let targeting = ability.targeting.as_ref().expect("one target group");
        let [group] = targeting.groups.as_slice() else {
            panic!("{}: exactly one target group", card.id);
        };
        assert_eq!(group.min, 1, "{}", card.id);
        assert_eq!(group.max, 1, "{}", card.id);
        assert_eq!(group.effect_indices, [0], "{}", card.id);
    }
}

#[test]
fn unblockable_power_two_batch_fingerprints_match_the_pinned_oracle_text() {
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
