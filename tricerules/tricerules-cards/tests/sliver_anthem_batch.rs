//! Registry conformance for the reviewed all-Sliver anthem family.
//!
//! Every card is a normal-layout Sliver whose only rules text is a symmetric anthem for all Sliver creatures.
//! Each fingerprint is SHA-256 of the exact pinned normalized Oracle text.

use tricerules_cards::primitives::StaticAbilityDef;
use tricerules_cards::{AbilityPresentation, Layout};

type Card = (
    &'static str,
    &'static str,
    &'static str,
    u32,
    u32,
    i32,
    i32,
    &'static str,
);

const CARDS: &[Card] = &[
    (
        "blade_sliver",
        "Blade Sliver",
        "{2}{R}",
        2,
        2,
        1,
        0,
        "0763cc6e711cb5f14c43978e080265e44d727c5c3ef055d0f986e99473c5259d",
    ),
    (
        "bonesplitter_sliver",
        "Bonesplitter Sliver",
        "{3}{R}",
        2,
        2,
        2,
        0,
        "82139a785c53e8a2aed9cb3d31ae39db7f1d208184c0adafc47ab6a21f47cb9d",
    ),
    (
        "might_sliver",
        "Might Sliver",
        "{4}{G}",
        2,
        2,
        2,
        2,
        "31cbc7bc2ccf18e1f92b681e9afe8e4751fb79177a1961d5059056ff9ea0eb2d",
    ),
    (
        "muscle_sliver",
        "Muscle Sliver",
        "{1}{G}",
        1,
        1,
        1,
        1,
        "dcb262072deb36ce2460d4217dbb479ccab11bbfa050afc8b5a21ba75838c7f2",
    ),
    (
        "plated_sliver",
        "Plated Sliver",
        "{W}",
        1,
        1,
        0,
        1,
        "eb6ebaff1307f7bf94ccf111f13c7d1d3149edf7619ca7ebf716aa624958aa96",
    ),
    (
        "sinew_sliver",
        "Sinew Sliver",
        "{1}{W}",
        1,
        1,
        1,
        1,
        "dcb262072deb36ce2460d4217dbb479ccab11bbfa050afc8b5a21ba75838c7f2",
    ),
    (
        "watcher_sliver",
        "Watcher Sliver",
        "{3}{W}",
        2,
        2,
        0,
        2,
        "54d24978e00b6df1ef9069470488bab70575428f5f9aa251bf0866bc8210cc5d",
    ),
];

#[test]
fn sliver_anthem_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, mana, power, toughness, dp, dt, _) in CARDS {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        assert_eq!(registry.id_for_name(name), Some(id), "{id}");
        let f = face(registry, id);
        assert_eq!(f.name, name, "{id} name");
        assert_eq!(f.mana_cost.to_string(), mana, "{id} mana cost");
        assert_eq!(f.types, ["Creature", "Sliver"], "{id} types");
        assert_eq!(
            (f.power, f.toughness),
            (Some(power), Some(toughness)),
            "{id} body"
        );
        assert!(f.keywords.is_empty(), "{id}");
        assert!(f.activated_abilities.is_empty(), "{id}");
        assert!(f.triggered_abilities.is_empty(), "{id}");

        let [ability] = f.static_abilities.as_slice() else {
            panic!("{id}: exactly one anthem");
        };
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{id}"
        );
        assert!(
            matches!(
                &ability.definition,
                StaticAbilityDef::AnthemPt { filter, delta_power: p, delta_toughness: t, .. }
                    if filter.subtype.as_deref() == Some("Sliver")
                        && filter.controller.is_none()
                        && !filter.exclude_self
                        && *p == dp
                        && *t == dt
            ),
            "{id}: all-Sliver anthem"
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
fn sliver_anthem_batch_fingerprints_match_the_pinned_oracle_text() {
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
