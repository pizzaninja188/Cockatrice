//! Registry conformance for the reviewed `attacks each combat if able` family.
//!
//! Every included card is a normal-layout creature with no abilities other than
//! the CR 508.1d must-attack characteristic. Card names, costs, types and bodies
//! are printed data; the shared scenario executes every card. Each fingerprint is
//! SHA-256 of the exact pinned normalized Oracle text.

use tricerules_cards::Layout;

type Card = (
    &'static str,
    &'static str,
    &'static str,
    &'static [&'static str],
    Option<(u32, u32)>,
    &'static str,
);

const CARDS: &[Card] = &[
    (
        "tattermunge_maniac",
        "Tattermunge Maniac",
        "{R/G}",
        &["Creature", "Goblin", "Warrior"],
        Some((2, 1)),
        "74282207889eacf8b8ec8ad04f44b39ded6a5aab0f676352e0c7443f952d58c0",
    ),
    (
        "bloodrock_cyclops",
        "Bloodrock Cyclops",
        "{2}{R}",
        &["Creature", "Cyclops"],
        Some((3, 3)),
        "74282207889eacf8b8ec8ad04f44b39ded6a5aab0f676352e0c7443f952d58c0",
    ),
    (
        "frontline_rebel",
        "Frontline Rebel",
        "{2}{R}",
        &["Creature", "Human", "Warrior"],
        Some((3, 3)),
        "74282207889eacf8b8ec8ad04f44b39ded6a5aab0f676352e0c7443f952d58c0",
    ),
    (
        "berserkers_of_blood_ridge",
        "Berserkers of Blood Ridge",
        "{4}{R}",
        &["Creature", "Human", "Berserker"],
        Some((4, 4)),
        "74282207889eacf8b8ec8ad04f44b39ded6a5aab0f676352e0c7443f952d58c0",
    ),
    (
        "rubblebelt_recluse",
        "Rubblebelt Recluse",
        "{4}{R}",
        &["Creature", "Ogre", "Berserker"],
        Some((6, 5)),
        "74282207889eacf8b8ec8ad04f44b39ded6a5aab0f676352e0c7443f952d58c0",
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
fn attack_each_combat_batch_maps_definitions() {
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
            body.map_or((None, None), |(p, t)| (Some(p), Some(t))),
            "{id} body"
        );

        assert!(f.must_attack_if_able, "{id}");
        assert!(f.keywords.is_empty(), "{id}");
        assert!(f.activated_abilities.is_empty(), "{id}");
        assert!(f.triggered_abilities.is_empty(), "{id}");
        assert!(f.static_abilities.is_empty(), "{id}");
        assert!(f.spell_effect.is_empty(), "{id}");
        assert!(f.targeting.is_none(), "{id}");
    }
}

#[test]
fn attack_each_combat_batch_fingerprints_match_the_pinned_oracle_text() {
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
