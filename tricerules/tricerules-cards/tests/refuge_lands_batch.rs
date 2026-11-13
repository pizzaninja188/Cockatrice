//! Registry conformance for the reviewed Zendikar refuge land cycle.
//!
//! Every card is a normal-layout land that enters tapped, gains one life on entry, and taps for one of its two printed colors.
//! Each fingerprint is SHA-256 of the exact pinned normalized Oracle text.

use tricerules_cards::primitives::{
    AbilityCost, Amount, EntersTappedAffected, PlayerRecipient, SpellEffectKind, StaticAbilityDef,
    TriggerCondition,
};
use tricerules_cards::{AbilityPresentation, Layout};

type Card = (&'static str, &'static str, &'static str);

const CARDS: &[Card] = &[
    (
        "akoum_refuge",
        "Akoum Refuge",
        "65355c8593f50e489d5854db246a42df2dcf8303d23907e4b8fe4634b2fb48d3",
    ),
    (
        "graypelt_refuge",
        "Graypelt Refuge",
        "e1e5ffb5492349ccf51aeff240bce97e2394327430ace8f9ed3b6542b2aba7d7",
    ),
    (
        "jwar_isle_refuge",
        "Jwar Isle Refuge",
        "40ce155786d374fbfcba12b09354ce2195c5b221fabaf77189a8669128301542",
    ),
    (
        "kazandu_refuge",
        "Kazandu Refuge",
        "297bc9080795f4c61217d465fa4455faf56dc50c9287f405e653e0ab39d36c67",
    ),
    (
        "sejiri_refuge",
        "Sejiri Refuge",
        "084b0dfe00f7740663f01f2305d1f366c0f241b00560a45d53d8ecd38e5eb818",
    ),
];

fn mask(color: char) -> [u32; 6] {
    match color {
        'w' => [1, 0, 0, 0, 0, 0],
        'u' => [0, 1, 0, 0, 0, 0],
        'b' => [0, 0, 1, 0, 0, 0],
        'r' => [0, 0, 0, 1, 0, 0],
        'g' => [0, 0, 0, 0, 1, 0],
        _ => unreachable!("printed color"),
    }
}

fn amounts(amount: &tricerules_cards::ManaAmount) -> [u32; 6] {
    [amount.w, amount.u, amount.b, amount.r, amount.g, amount.c]
}

#[test]
fn refuge_lands_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, _) in CARDS {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        assert_eq!(registry.id_for_name(name), Some(id), "{id}");
        let f = face(registry, id);
        assert_eq!(f.face_id.as_str(), id.replace('-', "_"), "{id} face id");
        assert_eq!(f.name, name, "{id} name");
        assert_eq!(f.mana_cost.to_string(), "", "{id} mana cost");
        assert_eq!(f.types, ["Land"], "{id} types");
        assert_eq!((f.power, f.toughness), (None, None), "{id} body");
        assert!(f.keywords.is_empty(), "{id}");
        assert!(f.spell_effect.is_empty(), "{id}");
        assert!(f.targeting.is_none(), "{id}");

        let [static_ability] = f.static_abilities.as_slice() else {
            panic!("{id}: exactly one static ability");
        };
        assert_eq!(static_ability.ability_id.as_str(), "static_01", "{id}");
        assert_eq!(
            static_ability.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{id}"
        );
        assert!(
            matches!(
                static_ability.definition,
                StaticAbilityDef::EntersTapped {
                    affected: EntersTappedAffected::Self_,
                    ..
                }
            ),
            "{id}: enters tapped"
        );

        let [triggered] = f.triggered_abilities.as_slice() else {
            panic!("{id}: exactly one entry trigger");
        };
        assert_eq!(triggered.ability_id.as_str(), "triggered_01", "{id}");
        assert_eq!(
            triggered.presentation,
            AbilityPresentation::OracleLines(vec![2]),
            "{id}"
        );
        assert_eq!(
            triggered.trigger,
            TriggerCondition::WhenSelfEntersBattlefield,
            "{id}"
        );
        assert_eq!(
            triggered.effect,
            [SpellEffectKind::GainLife {
                amount: Amount::Fixed(1),
                who: PlayerRecipient::Controller,
            }],
            "{id}: gains exactly one life"
        );

        let [activated] = f.activated_abilities.as_slice() else {
            panic!("{id}: exactly one mana ability");
        };
        assert_eq!(activated.ability_id.as_str(), "activated_01", "{id}");
        assert_eq!(
            activated.presentation,
            AbilityPresentation::OracleLines(vec![3]),
            "{id}"
        );
        assert_eq!(activated.costs, [AbilityCost::Tap], "{id}");
        assert!(activated.is_mana_ability(), "{id}");
        assert!(activated.targeting.is_none(), "{id}");
        let [SpellEffectKind::ProduceMana { options, .. }] = activated.effect.as_slice() else {
            panic!("{id}: one mana production");
        };
        assert_eq!(options.len(), 2, "{id}");
        let printed = match id {
            "akoum_refuge" => ['b', 'r'],
            "graypelt_refuge" => ['g', 'w'],
            "jwar_isle_refuge" => ['u', 'b'],
            "kazandu_refuge" => ['r', 'g'],
            "sejiri_refuge" => ['w', 'u'],
            _ => unreachable!("printed card"),
        };
        assert_eq!(amounts(&options[0]), mask(printed[0]), "{id} first color");
        assert_eq!(amounts(&options[1]), mask(printed[1]), "{id} second color");
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
fn refuge_lands_batch_fingerprints_match_the_pinned_oracle_text() {
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
