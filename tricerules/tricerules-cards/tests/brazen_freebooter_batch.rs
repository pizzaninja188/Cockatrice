//! Registry conformance for the reviewed Treasure-on-entry creature family.
//!
//! Every included card is a normal-layout creature whose only ability triggers when it
//! enters and creates one canonical Treasure token. Card names, costs, types and bodies
//! are printed data; the shared scenario executes every card. Each fingerprint is SHA-256
//! of the exact pinned Oracle line.

use tricerules_cards::primitives::SpellEffectKind;
use tricerules_cards::{AbilityPresentation, Layout, TriggerCondition};

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
        "brazen_freebooter",
        "Brazen Freebooter",
        "{3}{R}",
        &["Creature", "Human", "Pirate"],
        (3, 3),
        "8b1edeba2273cdc64fa012ad72831622661a29ead2a5bf806849a4553ba0cba1",
    ),
    (
        "sailor_of_means",
        "Sailor of Means",
        "{2}{U}",
        &["Creature", "Human", "Pirate"],
        (1, 4),
        "8b1edeba2273cdc64fa012ad72831622661a29ead2a5bf806849a4553ba0cba1",
    ),
    (
        "wily_goblin",
        "Wily Goblin",
        "{R}{R}",
        &["Creature", "Goblin", "Pirate"],
        (1, 1),
        "8b1edeba2273cdc64fa012ad72831622661a29ead2a5bf806849a4553ba0cba1",
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
fn brazen_freebooter_batch_maps_definitions() {
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
        assert!(f.static_abilities.is_empty(), "{id}");
        assert!(f.spell_effect.is_empty(), "{id}");
        assert!(f.targeting.is_none(), "{id}");

        let [ability] = f.triggered_abilities.as_slice() else {
            panic!("{id}: exactly one triggered ability");
        };
        assert_eq!(ability.ability_id.as_str(), "triggered_01", "{id}");
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{id} presentation"
        );
        assert_eq!(
            ability.trigger,
            TriggerCondition::WhenSelfEntersBattlefield,
            "{id} entry trigger"
        );
        assert!(
            matches!(
                ability.effect.as_slice(),
                [SpellEffectKind::CreateTokens { token, count, .. }]
                    if token == "treasure"
                        && *count == tricerules_cards::primitives::Amount::Fixed(1)
            ),
            "{id}: one Treasure"
        );
    }
}

#[test]
fn brazen_freebooter_batch_fingerprints_match_the_pinned_oracle_text() {
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
