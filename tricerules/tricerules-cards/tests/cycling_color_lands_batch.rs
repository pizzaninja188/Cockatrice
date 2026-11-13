//! Registry conformance for the reviewed colored-cost Cycling land cycle.
//!
//! Every card is a normal-layout land that enters tapped, has one tap mana
//! ability for its printed color, and has the hand-only Cycling ability paid with
//! its own color that discards itself and draws a card. Each fingerprint is
//! SHA-256 of the exact pinned normalized Oracle text.

use tricerules_cards::primitives::{
    AbilityCost, EntersTappedAffected, SpellEffectKind, StaticAbilityDef,
};
use tricerules_cards::{AbilityPresentation, AbilitySourceZone, Layout};

type Card = (
    &'static str,
    &'static str,
    [u32; 6],
    &'static str,
    &'static str,
);

const CARDS: &[Card] = &[
    (
        "barren_moor",
        "Barren Moor",
        [0, 0, 1, 0, 0, 0],
        "{B}",
        "06f7c6dd8381b4f07fcaed5b589309696d939844a82db1cb16dc7b50c245e79a",
    ),
    (
        "lonely_sandbar",
        "Lonely Sandbar",
        [0, 1, 0, 0, 0, 0],
        "{U}",
        "9ce151180c9ca462e3b5facfcbf84fd141700fa389d9c01b70b538576b67f26f",
    ),
    (
        "secluded_steppe",
        "Secluded Steppe",
        [1, 0, 0, 0, 0, 0],
        "{W}",
        "97d470c81ccdbf653bc99632ed0112a7d4a3a9294a46deb462939153d714911a",
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

fn mask(amount: &tricerules_cards::ManaAmount) -> [u32; 6] {
    [amount.w, amount.u, amount.b, amount.r, amount.g, amount.c]
}

#[test]
fn cycling_color_lands_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, color, cycling_cost, _) in CARDS {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        assert_eq!(registry.id_for_name(name), Some(id), "{id}");
        let f = face(registry, id);
        assert_eq!(f.face_id.as_str(), id, "{id} face id");
        assert_eq!(f.name, name, "{id} name");
        assert_eq!(f.mana_cost.to_string(), "", "{id} mana cost");
        assert_eq!(f.types, ["Land"], "{id} types");
        assert_eq!((f.power, f.toughness), (None, None), "{id} body");
        assert!(f.keywords.is_empty(), "{id}");
        assert!(f.triggered_abilities.is_empty(), "{id}");
        assert!(f.spell_effect.is_empty(), "{id}");

        let [entry] = f.static_abilities.as_slice() else {
            panic!("{id}: exactly one static ability");
        };
        assert_eq!(entry.ability_id.as_str(), "enters_tapped", "{id}");
        assert_eq!(
            entry.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{id}"
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

        let [mana, cycling] = f.activated_abilities.as_slice() else {
            panic!("{id}: exactly two activated abilities");
        };
        assert_eq!(mana.ability_id.as_str(), "activated_01", "{id}");
        assert_eq!(
            mana.presentation,
            AbilityPresentation::OracleLines(vec![2]),
            "{id}"
        );
        assert_eq!(mana.costs, [AbilityCost::Tap], "{id}");
        assert!(mana.is_mana_ability(), "{id}: mana ability");
        assert!(
            matches!(
                mana.effect.as_slice(),
                [SpellEffectKind::ProduceMana { options, .. }]
                    if options.len() == 1 && mask(&options[0]) == color
            ),
            "{id}: exact printed color"
        );

        assert_eq!(cycling.ability_id.as_str(), "activated_02", "{id}");
        assert_eq!(
            cycling.presentation,
            AbilityPresentation::OracleLines(vec![3]),
            "{id}"
        );
        assert_eq!(cycling.source_zone, AbilitySourceZone::Hand, "{id}");
        assert!(
            matches!(
                cycling.costs.as_slice(),
                [AbilityCost::Mana(cost), AbilityCost::DiscardSelf]
                    if cost.to_string() == cycling_cost
            ),
            "{id}: printed colored cycling cost"
        );
        assert!(
            matches!(
                cycling.effect.as_slice(),
                [SpellEffectKind::Draw {
                    count: tricerules_cards::Amount::Fixed(1),
                    ..
                }]
            ),
            "{id}: draw one"
        );
        assert!(cycling.targeting.is_none(), "{id}: no target");
    }
}

#[test]
fn cycling_color_lands_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for &(id, name, _, _, expected) in CARDS {
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
