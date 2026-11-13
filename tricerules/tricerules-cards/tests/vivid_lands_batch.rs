//! Registry conformance for the reviewed Vivid land cycle.
//!
//! Every card is a normal-layout land that enters tapped with two Charge
//! counters, taps for its printed color, and can spend a Charge counter with its
//! tap to add one mana of any color. Each fingerprint is SHA-256 of the exact
//! pinned normalized Oracle text.

use tricerules_cards::primitives::{
    AbilityCost, Amount, CounterKind, EntersTappedAffected, EntersWithCountersAffected,
    SpellEffectKind, StaticAbilityDef,
};
use tricerules_cards::{AbilityPresentation, Color, Layout};

type Card = (&'static str, &'static str, [u32; 6], Color, &'static str);

const CARDS: &[Card] = &[
    (
        "vivid_marsh",
        "Vivid Marsh",
        [0, 0, 1, 0, 0, 0],
        Color::Black,
        "2e1a395b21ec336e2f6a0f5425673f34d64c5cd3b8334e52d63bb6bcc2806e31",
    ),
    (
        "vivid_creek",
        "Vivid Creek",
        [0, 1, 0, 0, 0, 0],
        Color::Blue,
        "6833fd9beffc844c40349b30477dd3e875afb6d5bb6e06d9da6bd0dd9ab62c97",
    ),
    (
        "vivid_crag",
        "Vivid Crag",
        [0, 0, 0, 1, 0, 0],
        Color::Red,
        "3b80c6d24a34c7ad44f97976bbeec0970a12bace13b0b859674bf2d444dad020",
    ),
    (
        "vivid_meadow",
        "Vivid Meadow",
        [1, 0, 0, 0, 0, 0],
        Color::White,
        "086f41710d8a82b6b8f5f88e8dde67083982baf9c93060d6bd7528c8659e8993",
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
fn vivid_lands_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, color, identity, _) in CARDS {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        assert_eq!(registry.id_for_name(name), Some(id), "{id}");
        let f = face(registry, id);
        assert_eq!(f.face_id.as_str(), id, "{id} face id");
        assert_eq!(f.name, name, "{id} name");
        assert_eq!(f.mana_cost.to_string(), "", "{id} mana cost");
        assert_eq!(f.types, ["Land"], "{id} types");
        assert_eq!(
            definition.color_identity(),
            vec![identity],
            "{id} color identity"
        );
        assert_eq!((f.power, f.toughness), (None, None), "{id} body");
        assert!(f.keywords.is_empty(), "{id}");
        assert!(f.triggered_abilities.is_empty(), "{id}");
        assert!(f.spell_effect.is_empty(), "{id}");

        let [entry, counters] = f.static_abilities.as_slice() else {
            panic!("{id}: two entry replacements");
        };
        assert_eq!(entry.ability_id.as_str(), "static_01", "{id}");
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
        assert_eq!(counters.ability_id.as_str(), "static_02", "{id}");
        assert_eq!(
            counters.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{id}"
        );
        assert_eq!(
            counters.definition,
            StaticAbilityDef::EntersWithCounters {
                affected: EntersWithCountersAffected::Self_,
                counter: CounterKind::Charge,
                amount: Amount::Fixed(2),
                cast_cost_condition: None,
            },
            "{id}"
        );

        let [mana, any_color] = f.activated_abilities.as_slice() else {
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

        assert_eq!(any_color.ability_id.as_str(), "activated_02", "{id}");
        assert_eq!(
            any_color.presentation,
            AbilityPresentation::OracleLines(vec![3]),
            "{id}"
        );
        assert_eq!(any_color.costs.len(), 2, "{id}");
        assert_eq!(any_color.costs[0], AbilityCost::Tap, "{id}");
        assert!(
            matches!(
                &any_color.costs[1],
                AbilityCost::RemoveCounters {
                    counter: Some(CounterKind::Charge),
                    count: 1,
                    ..
                }
            ),
            "{id}: exactly one Charge counter"
        );
        let expected = [
            [1, 0, 0, 0, 0, 0],
            [0, 1, 0, 0, 0, 0],
            [0, 0, 1, 0, 0, 0],
            [0, 0, 0, 1, 0, 0],
            [0, 0, 0, 0, 1, 0],
        ];
        assert!(
            matches!(
                any_color.effect.as_slice(),
                [SpellEffectKind::ProduceMana { options, .. }]
                    if options.iter().map(mask).collect::<Vec<_>>() == expected
            ),
            "{id}: exactly the five distinct single-color options"
        );
    }
}

#[test]
fn vivid_lands_batch_fingerprints_match_the_pinned_oracle_text() {
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
