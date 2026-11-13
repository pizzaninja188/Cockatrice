//! Registry conformance for the reviewed Flying / can't block family.
//!
//! Every included card is a normal-layout card with the same flying creatures that cannot block
//! shape. Card names, costs, types and bodies are printed data; the shared
//! scenario executes every card. Each fingerprint is SHA-256 of the exact
//! pinned normalized Oracle text.

use tricerules_cards::primitives::StaticAbilityDef;
use tricerules_cards::{AbilityPresentation, Keyword, Layout};

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
        "aesthir_glider",
        "Aesthir Glider",
        "{3}",
        &["Artifact", "Creature", "Bird", "Construct"],
        Some((2, 1)),
        "eb15f687af89e0f5524e2e6907d8dc1055c4a4764dd27eca280563049849b622",
    ),
    (
        "daggerclaw_imp",
        "Daggerclaw Imp",
        "{2}{B}",
        &["Creature", "Imp"],
        Some((3, 1)),
        "eb15f687af89e0f5524e2e6907d8dc1055c4a4764dd27eca280563049849b622",
    ),
    (
        "goblin_glider",
        "Goblin Glider",
        "{1}{R}",
        &["Creature", "Goblin"],
        Some((1, 1)),
        "eb15f687af89e0f5524e2e6907d8dc1055c4a4764dd27eca280563049849b622",
    ),
    (
        "kyren_glider",
        "Kyren Glider",
        "{1}{R}",
        &["Creature", "Goblin"],
        Some((1, 1)),
        "eb15f687af89e0f5524e2e6907d8dc1055c4a4764dd27eca280563049849b622",
    ),
    (
        "nightshade_stinger",
        "Nightshade Stinger",
        "{B}",
        &["Creature", "Faerie", "Rogue"],
        Some((1, 1)),
        "eb15f687af89e0f5524e2e6907d8dc1055c4a4764dd27eca280563049849b622",
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
fn flying_cant_block_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, mana, types, body, _) in CARDS {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        assert_eq!(registry.id_for_name(name), Some(id), "{id}");
        let f = face(registry, id);
        assert_eq!(f.face_id.as_str(), id.replace('-', "_"), "{id} face id");
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

        assert_eq!(f.keywords, [Keyword::Flying], "{id} keywords");
        assert!(f.triggered_abilities.is_empty(), "{id}");
        assert!(f.activated_abilities.is_empty(), "{id}");
        assert!(f.spell_effect.is_empty(), "{id}");
        assert!(f.targeting.is_none(), "{id}");
        let [ability] = f.static_abilities.as_slice() else {
            panic!("{id}: exactly one static ability");
        };
        assert_eq!(ability.ability_id.as_str(), "static_01", "{id}");
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![2]),
            "{id} presentation"
        );
        assert!(
            matches!(
                &ability.definition,
                StaticAbilityDef::SelfCombatRestriction {
                    restriction,
                    condition: None,
                } if restriction.cant_block
            ),
            "{id}: must publish the can't-block restriction"
        );
    }
}

#[test]
fn flying_cant_block_batch_fingerprints_match_the_pinned_oracle_text() {
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
        assert_eq!(fields[2], id.replace('-', "_"));
        assert_eq!(fields[4], expected, "fingerprint drift for {id}");
    }
}
