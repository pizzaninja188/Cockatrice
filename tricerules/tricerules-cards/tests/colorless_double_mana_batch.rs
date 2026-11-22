//! Registry conformance for the reviewed `{T}: Add {C}{C}` family.
//!
//! Every included card has exactly one tap mana ability producing two
//! colorless mana. Card names, costs, types and bodies are printed data; the
//! shared scenario executes every card. Each fingerprint is SHA-256 of the
//! exact pinned normalized Oracle text.

use tricerules_cards::primitives::{AbilityCost, SpellEffectKind};
use tricerules_cards::{AbilityPresentation, Layout};

type Card = (
    &'static str,
    &'static str,
    &'static str,
    &'static [&'static str],
    bool,
    Option<(u32, u32)>,
    &'static str,
);

const CARDS: &[Card] = &[
    (
        "sunastian_falconer",
        "Sunastian Falconer",
        "{3}{R}{G}",
        &["Creature", "Human", "Shaman"],
        true,
        Some((4, 4)),
        "8310b06afdea748af91bd3f1f23206140bba9309ef34fbd71e3211a8c3542d58",
    ),
    (
        "weaver_of_currents",
        "Weaver of Currents",
        "{1}{G}{U}",
        &["Creature", "Snake", "Druid"],
        false,
        Some((2, 2)),
        "8310b06afdea748af91bd3f1f23206140bba9309ef34fbd71e3211a8c3542d58",
    ),
    (
        "ur-golems_eye",
        "Ur-Golem's Eye",
        "{4}",
        &["Artifact"],
        false,
        None,
        "8310b06afdea748af91bd3f1f23206140bba9309ef34fbd71e3211a8c3542d58",
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
fn colorless_double_mana_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, mana, types, legendary, body, _) in CARDS {
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
        assert_eq!(
            f.supertypes,
            if legendary {
                vec!["Legendary".to_string()]
            } else {
                Vec::<String>::new()
            },
            "{id} supertypes"
        );
        assert_eq!(
            (f.power, f.toughness),
            body.map_or((None, None), |(p, t)| (Some(p), Some(t))),
            "{id} body"
        );
        assert!(f.keywords.is_empty(), "{id}");
        assert!(f.triggered_abilities.is_empty(), "{id}");
        assert!(f.static_abilities.is_empty(), "{id}");
        assert!(f.spell_effect.is_empty(), "{id}");
        assert!(f.targeting.is_none(), "{id}");

        let [ability] = f.activated_abilities.as_slice() else {
            panic!("{id}: exactly one activated ability");
        };
        assert_eq!(ability.ability_id.as_str(), "activated_01", "{id}");
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{id} presentation"
        );
        assert_eq!(ability.costs, [AbilityCost::Tap], "{id} tap cost");
        assert!(ability.is_mana_ability(), "{id}: mana ability");
        assert!(
            matches!(
                ability.effect.as_slice(),
                [SpellEffectKind::ProduceMana { options, .. }]
                    if options.len() == 1
                        && options[0].c == 2
                        && options[0].w == 0
                        && options[0].u == 0
                        && options[0].b == 0
                        && options[0].r == 0
                        && options[0].g == 0
            ),
            "{id}: exactly two colorless mana"
        );
    }
}

#[test]
fn colorless_double_mana_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for &(id, name, _, _, _, _, expected) in CARDS {
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
