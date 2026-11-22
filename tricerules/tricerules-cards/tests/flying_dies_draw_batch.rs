//! Registry conformance for the reviewed Flying / dies-draw cohort.
//!
//! Every included card is a normal-layout creature with Flying and a dies trigger
//! that draws exactly one card.
//! Each fingerprint is SHA-256 of the exact pinned normalized Oracle text.

use tricerules_cards::primitives::SpellEffectKind;
use tricerules_cards::{AbilityPresentation, Keyword, Layout};

type Card = (
    &'static str,
    &'static str,
    &'static str,
    &'static [&'static str],
    &'static [&'static str],
    Option<(u32, u32)>,
    &'static str,
);

const CARDS: &[Card] = &[
    (
        "darkslick_drake",
        "Darkslick Drake",
        "{2}{U}{U}",
        &["Creature", "Phyrexian", "Drake"],
        &[],
        Some((2, 4)),
        "fe6fe0f2750e7cd7920a0a9f37c704eb64efe15512f4f18383a8c92b44f814aa",
    ),
    (
        "ithilien_kingfisher",
        "Ithilien Kingfisher",
        "{2}{U}",
        &["Creature", "Bird"],
        &[],
        Some((2, 1)),
        "fe6fe0f2750e7cd7920a0a9f37c704eb64efe15512f4f18383a8c92b44f814aa",
    ),
    (
        "kingfisher",
        "Kingfisher",
        "{3}{U}",
        &["Creature", "Bird"],
        &[],
        Some((2, 2)),
        "fe6fe0f2750e7cd7920a0a9f37c704eb64efe15512f4f18383a8c92b44f814aa",
    ),
    (
        "messenger_drake",
        "Messenger Drake",
        "{3}{U}{U}",
        &["Creature", "Drake"],
        &[],
        Some((3, 3)),
        "fe6fe0f2750e7cd7920a0a9f37c704eb64efe15512f4f18383a8c92b44f814aa",
    ),
    (
        "palace_familiar",
        "Palace Familiar",
        "{1}{U}",
        &["Creature", "Bird"],
        &[],
        Some((1, 1)),
        "fe6fe0f2750e7cd7920a0a9f37c704eb64efe15512f4f18383a8c92b44f814aa",
    ),
    (
        "runewing",
        "Runewing",
        "{3}{U}",
        &["Creature", "Bird"],
        &[],
        Some((2, 2)),
        "fe6fe0f2750e7cd7920a0a9f37c704eb64efe15512f4f18383a8c92b44f814aa",
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
fn flying_dies_draw_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, mana, types, supertypes, body, _) in CARDS {
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
            f.supertypes.iter().map(String::as_str).collect::<Vec<_>>(),
            supertypes,
            "{id} supertypes"
        );
        assert_eq!(
            (f.power, f.toughness),
            body.map_or((None, None), |(p, t)| (Some(p), Some(t))),
            "{id} body"
        );
        assert!(f.spell_effect.is_empty(), "{id}");
        assert!(f.targeting.is_none(), "{id}");
        assert_eq!(f.keywords, [Keyword::Flying], "{id} keywords");
        assert!(f.activated_abilities.is_empty(), "{id}");
        assert!(f.static_abilities.is_empty(), "{id}");
        let [ability] = f.triggered_abilities.as_slice() else {
            panic!("{id}: exactly one triggered ability");
        };
        assert_eq!(ability.ability_id.as_str(), "triggered_01", "{id}");
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![2]),
            "{id} presentation"
        );
        assert_eq!(
            ability.trigger,
            tricerules_cards::primitives::TriggerCondition::WhenSelfDies,
            "{id}"
        );
        assert!(
            matches!(
                ability.effect.as_slice(),
                [SpellEffectKind::Draw {
                    count: tricerules_cards::Amount::Fixed(1),
                    ..
                }]
            ),
            "{id}: draw one"
        );
        assert!(ability.targeting.is_none(), "{id}");
    }
}

#[test]
fn flying_dies_draw_batch_fingerprints_match_the_pinned_oracle_text() {
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
