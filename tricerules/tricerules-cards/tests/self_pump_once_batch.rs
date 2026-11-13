//! Registry conformance for the reviewed once-each-turn self-pump family.
//!
//! Every card is a normal-layout creature with one activated self-pump ability limited to one activation each turn.
//! Each fingerprint is SHA-256 of the exact pinned normalized Oracle text.

use tricerules_cards::primitives::{AbilityCost, ActivationLimit, EffectSubject, SpellEffectKind};
use tricerules_cards::{AbilityPresentation, Layout};

type Card = (
    &'static str,
    &'static str,
    &'static str,
    &'static [&'static str],
    u32,
    u32,
    &'static str,
    i32,
    i32,
    &'static str,
);

const CARDS: &[Card] = &[
    (
        "darkthicket_wolf",
        "Darkthicket Wolf",
        "{1}{G}",
        &["Creature", "Wolf"],
        2,
        2,
        "{2}{G}",
        2,
        2,
        "b13fd9c28eba5e2cb80229094f9b3d3d924ca0740b5c9aeecb281abb82c017bf",
    ),
    (
        "frilled_oculus",
        "Frilled Oculus",
        "{1}{U}",
        &["Creature", "Homunculus"],
        1,
        3,
        "{1}{G}",
        2,
        2,
        "8da6fd77d3afd42a75186e971d94d9447cab61b874f81e0e1bf4f79586a30707",
    ),
    (
        "frilled_sandwalla",
        "Frilled Sandwalla",
        "{G}",
        &["Creature", "Lizard"],
        1,
        1,
        "{1}{G}",
        2,
        2,
        "8da6fd77d3afd42a75186e971d94d9447cab61b874f81e0e1bf4f79586a30707",
    ),
    (
        "knight_of_the_skyward_eye",
        "Knight of the Skyward Eye",
        "{1}{W}",
        &["Creature", "Human", "Knight"],
        2,
        2,
        "{3}{G}",
        3,
        3,
        "a297a9019aa2b3d071528b9201965910e56e9a149548d1ed3ae695f65c85ddf3",
    ),
    (
        "plated_rootwalla",
        "Plated Rootwalla",
        "{4}{G}",
        &["Creature", "Lizard"],
        3,
        3,
        "{2}{G}",
        3,
        3,
        "f66e5c574c7efdcabbc6dd16e2307c31d2818edf0c0636b73daf65093d2439a4",
    ),
    (
        "rootwalla",
        "Rootwalla",
        "{2}{G}",
        &["Creature", "Lizard"],
        2,
        2,
        "{1}{G}",
        2,
        2,
        "8da6fd77d3afd42a75186e971d94d9447cab61b874f81e0e1bf4f79586a30707",
    ),
    (
        "snarling_wolf",
        "Snarling Wolf",
        "{G}",
        &["Creature", "Wolf"],
        1,
        1,
        "{1}{G}",
        2,
        2,
        "8da6fd77d3afd42a75186e971d94d9447cab61b874f81e0e1bf4f79586a30707",
    ),
];

#[test]
fn self_pump_once_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, mana, types, power, toughness, printed_cost, dp, dt, _) in CARDS {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        assert_eq!(registry.id_for_name(name), Some(id), "{id}");
        let f = face(registry, id);
        assert_eq!(f.name, name, "{id} name");
        assert_eq!(f.mana_cost.to_string(), mana, "{id} mana cost");
        assert_eq!(f.types, types, "{id} types");
        assert_eq!(
            (f.power, f.toughness),
            (Some(power), Some(toughness)),
            "{id} body"
        );
        assert!(f.keywords.is_empty(), "{id}");
        assert!(f.triggered_abilities.is_empty(), "{id}");
        assert!(f.static_abilities.is_empty(), "{id}");
        assert!(f.spell_effect.is_empty(), "{id}");
        assert!(f.targeting.is_none(), "{id}");

        let [activated] = f.activated_abilities.as_slice() else {
            panic!("{id}: exactly one activated ability");
        };
        assert_eq!(
            activated.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{id}"
        );
        assert!(
            matches!(
                activated.costs.as_slice(),
                [AbilityCost::Mana(cost)] if cost.to_string() == printed_cost
            ),
            "{id}: printed activation cost"
        );
        assert_eq!(
            activated.activation_limit,
            Some(ActivationLimit::PerTurn { max_activations: 1 }),
            "{id}: once each turn"
        );
        assert!(
            matches!(
                activated.effect.as_slice(),
                [SpellEffectKind::PumpTarget { power: p, toughness: t, subject: EffectSubject::Source, .. }]
                    if *p == dp && *t == dt
            ),
            "{id}: source-only pump"
        );
    }
}

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
fn self_pump_once_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for &(id, name, .., expected) in CARDS {
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
