//! Registry conformance for the reviewed `{T}: Add one mana of any color` family.
//!
//! Every included card has exactly one tap mana ability offering one mana of
//! each color. Card names, costs, types and bodies are printed data; the
//! shared scenario executes every card. Each fingerprint is SHA-256 of the
//! exact pinned normalized Oracle text.

use tricerules_cards::primitives::{AbilityCost, SpellEffectKind};
use tricerules_cards::{AbilityPresentation, Layout};

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
        "lifespring_druid",
        "Lifespring Druid",
        "{2}{G}",
        &["Creature", "Elf", "Druid"],
        Some((2, 1)),
        "8f936f93056579215d4375dd86511c58e9c0a1a079579b6cea430d5f87bfcec0",
    ),
    (
        "utopia_tree",
        "Utopia Tree",
        "{1}{G}",
        &["Creature", "Plant"],
        Some((0, 2)),
        "8f936f93056579215d4375dd86511c58e9c0a1a079579b6cea430d5f87bfcec0",
    ),
    (
        "xenobotanist",
        "Xenobotanist",
        "{1}{G}",
        &["Creature", "Vulcan", "Scientist"],
        Some((1, 3)),
        "8f936f93056579215d4375dd86511c58e9c0a1a079579b6cea430d5f87bfcec0",
    ),
    (
        "manalith",
        "Manalith",
        "{3}",
        &["Artifact"],
        None,
        "8f936f93056579215d4375dd86511c58e9c0a1a079579b6cea430d5f87bfcec0",
    ),
    (
        "opaline_unicorn",
        "Opaline Unicorn",
        "{3}",
        &["Artifact", "Creature", "Unicorn"],
        Some((1, 2)),
        "8f936f93056579215d4375dd86511c58e9c0a1a079579b6cea430d5f87bfcec0",
    ),
    (
        "alloy_myr",
        "Alloy Myr",
        "{3}",
        &["Artifact", "Creature", "Myr"],
        Some((2, 2)),
        "8f936f93056579215d4375dd86511c58e9c0a1a079579b6cea430d5f87bfcec0",
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
fn any_color_mana_batch_maps_definitions() {
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
                    if options.len() == 5
                        && options.iter().all(|amount| {
                            [amount.w, amount.u, amount.b, amount.r, amount.g]
                                .iter()
                                .filter(|color| **color == 1)
                                .count()
                                == 1
                        })
            ),
            "{id}: five single-color options"
        );
    }
}

#[test]
fn any_color_mana_batch_fingerprints_match_the_pinned_oracle_text() {
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
