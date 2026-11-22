//! Registry conformance for the reviewed "This creature enters tapped." family.
//!
//! Every included card is a normal-layout creature whose only ability is the
//! entry replacement. Card names, costs, types and bodies are printed data;
//! the shared scenario executes every card. Each fingerprint is SHA-256 of the
//! exact pinned normalized Oracle text.

use tricerules_cards::primitives::{EntersTappedAffected, StaticAbilityDef};
use tricerules_cards::{AbilityPresentation, Layout};

type Card = (
    &'static str,
    &'static str,
    &'static str,
    &'static [&'static str],
    (u32, u32),
    &'static str,
);

const CARDS: &[Card] = &[
    (
        "shambling_ghoul",
        "Shambling Ghoul",
        "{1}{B}",
        &["Creature", "Zombie"],
        (2, 3),
        "2b8c812669c41674de17a0ada66e3df7b972cf4fd5400727367761c46a458468",
    ),
    (
        "wolf_cove_villager",
        "Wolf Cove Villager",
        "{W}",
        &["Creature", "Human", "Peasant"],
        (2, 2),
        "2b8c812669c41674de17a0ada66e3df7b972cf4fd5400727367761c46a458468",
    ),
    (
        "rotting_legion",
        "Rotting Legion",
        "{4}{B}",
        &["Creature", "Zombie"],
        (4, 5),
        "2b8c812669c41674de17a0ada66e3df7b972cf4fd5400727367761c46a458468",
    ),
    (
        "scarwood_treefolk",
        "Scarwood Treefolk",
        "{3}{G}",
        &["Creature", "Treefolk"],
        (3, 5),
        "2b8c812669c41674de17a0ada66e3df7b972cf4fd5400727367761c46a458468",
    ),
    (
        "unhallowed_phalanx",
        "Unhallowed Phalanx",
        "{4}{B}",
        &["Creature", "Zombie", "Soldier"],
        (1, 13),
        "2b8c812669c41674de17a0ada66e3df7b972cf4fd5400727367761c46a458468",
    ),
    (
        "forgotten_sentinel",
        "Forgotten Sentinel",
        "{4}",
        &["Artifact", "Creature", "Golem"],
        (4, 3),
        "2b8c812669c41674de17a0ada66e3df7b972cf4fd5400727367761c46a458468",
    ),
    (
        "crooked_custodian",
        "Crooked Custodian",
        "{1}{B}",
        &["Creature", "Ogre", "Rogue"],
        (3, 2),
        "2b8c812669c41674de17a0ada66e3df7b972cf4fd5400727367761c46a458468",
    ),
    (
        "rusted_sentinel",
        "Rusted Sentinel",
        "{4}",
        &["Artifact", "Creature", "Golem"],
        (3, 4),
        "2b8c812669c41674de17a0ada66e3df7b972cf4fd5400727367761c46a458468",
    ),
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
fn enters_tapped_creatures_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, mana, types, body, _) in CARDS {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        assert_eq!(registry.id_for_name(name), Some(id), "{id}");
        let f = face(registry, id);
        assert_eq!(f.face_id.as_str(), id, "{id} face id");
        assert_eq!(f.name, name, "{id} name");
        assert_eq!(f.mana_cost.to_string(), mana, "{id} mana cost");
        assert_eq!(
            f.types.iter().map(String::as_str).collect::<Vec<_>>(),
            types,
            "{id} types"
        );
        assert!(f.supertypes.is_empty(), "{id} supertypes");
        assert_eq!(
            (f.power, f.toughness),
            (Some(body.0), Some(body.1)),
            "{id} body"
        );
        assert!(f.keywords.is_empty(), "{id}");
        assert!(f.activated_abilities.is_empty(), "{id}");
        assert!(f.triggered_abilities.is_empty(), "{id}");
        assert!(f.spell_effect.is_empty(), "{id}");
        assert!(f.targeting.is_none(), "{id}");

        let [entry] = f.static_abilities.as_slice() else {
            panic!("{id}: exactly one static ability");
        };
        assert_eq!(entry.ability_id.as_str(), "static_01", "{id}");
        assert_eq!(
            entry.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{id} entry presentation"
        );
        assert_eq!(
            entry.definition,
            StaticAbilityDef::EntersTapped {
                affected: EntersTappedAffected::Self_,
                condition: None,
                unless_cost: None,
            },
            "{id}"
        );
    }
}

#[test]
fn enters_tapped_creatures_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for &(id, name, _, _, _, expected) in CARDS {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")))
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}"));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[0], id);
        assert_eq!(fields[1], name);
        assert_eq!(fields[2], id);
        assert_eq!(fields[4], expected, "fingerprint drift for {id}");
    }
}
