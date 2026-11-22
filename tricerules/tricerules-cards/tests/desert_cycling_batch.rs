//! Registry conformance for the reviewed Desert cycling land cycle.
//!
//! Every included card is a normal-layout Desert that enters tapped, has one
//! `{T}` single-color mana ability and hand-only Cycling paid with `{1}` plus
//! its printed color that discards itself and draws one. Each fingerprint is
//! SHA-256 of the exact pinned Oracle lines.

use tricerules_cards::primitives::{
    AbilityCost, Amount, EntersTappedAffected, SpellEffectKind, StaticAbilityDef,
};
use tricerules_cards::{AbilityPresentation, AbilitySourceZone, Layout};

type Mana = (u32, u32, u32, u32, u32, u32);

struct Card {
    id: &'static str,
    name: &'static str,
    color: Mana,
    cycling: &'static str,
    fingerprint: &'static str,
}

const CARDS: &[Card] = &[
    Card {
        id: "desert_of_the_fervent",
        name: "Desert of the Fervent",
        color: (0, 0, 0, 1, 0, 0),
        cycling: "{1}{R}",
        fingerprint: "1418afd10c4180c067243d46c9a3dd7b4a3c56d52f40fba7207d8228534265f9",
    },
    Card {
        id: "desert_of_the_glorified",
        name: "Desert of the Glorified",
        color: (0, 0, 1, 0, 0, 0),
        cycling: "{1}{B}",
        fingerprint: "530e4662649055d45adcad5abacb6f7126bb14f8c7aca019b8dc862230638c55",
    },
    Card {
        id: "desert_of_the_indomitable",
        name: "Desert of the Indomitable",
        color: (0, 0, 0, 0, 1, 0),
        cycling: "{1}{G}",
        fingerprint: "431d409bd71fd372cb8d4142757d0477eafddd4bedafba7b452c3897fd195418",
    },
    Card {
        id: "desert_of_the_mindful",
        name: "Desert of the Mindful",
        color: (0, 1, 0, 0, 0, 0),
        cycling: "{1}{U}",
        fingerprint: "afa2769b991d9737e451dabe18ea0a3bcc4a78c02d7bb4ca21fc4e22fc017382",
    },
    Card {
        id: "desert_of_the_true",
        name: "Desert of the True",
        color: (1, 0, 0, 0, 0, 0),
        cycling: "{1}{W}",
        fingerprint: "0f55e4a933a66adc94c79dc1c6f8e76039deb238fbe1a929fc32a1bc8da73548",
    },
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
fn desert_cycling_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for card in CARDS {
        let definition = registry
            .get(card.id)
            .unwrap_or_else(|| panic!("missing {}", card.id));
        assert_eq!(definition.layout, Layout::Normal, "{}", card.id);
        assert_eq!(
            registry.id_for_name(card.name),
            Some(card.id),
            "{}",
            card.id
        );
        let f = face(registry, card.id);
        assert_eq!(f.face_id.as_str(), card.id, "{} face id", card.id);
        assert_eq!(f.name, card.name, "{} name", card.id);
        assert_eq!(f.mana_cost.to_string(), "", "{} mana cost", card.id);
        assert_eq!(f.types, ["Land", "Desert"], "{} types", card.id);
        assert!(f.supertypes.is_empty(), "{}", card.id);
        assert_eq!(f.power, None, "{}", card.id);
        assert_eq!(f.toughness, None, "{}", card.id);
        assert!(f.keywords.is_empty(), "{}", card.id);
        assert!(f.triggered_abilities.is_empty(), "{}", card.id);
        assert!(f.spell_effect.is_empty(), "{}", card.id);
        assert!(f.targeting.is_none(), "{}", card.id);

        let [entry] = f.static_abilities.as_slice() else {
            panic!("{}: exactly one static ability", card.id);
        };
        assert_eq!(entry.ability_id.as_str(), "static_01", "{}", card.id);
        assert_eq!(
            entry.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{} entry presentation",
            card.id
        );
        assert_eq!(
            entry.definition,
            StaticAbilityDef::EntersTapped {
                affected: EntersTappedAffected::Self_,
                condition: None,
                unless_cost: None,
            },
            "{}",
            card.id
        );

        let [mana, cycling] = f.activated_abilities.as_slice() else {
            panic!("{}: exactly two activated abilities", card.id);
        };
        assert_eq!(mana.ability_id.as_str(), "activated_01", "{}", card.id);
        assert_eq!(
            mana.presentation,
            AbilityPresentation::OracleLines(vec![2]),
            "{} mana presentation",
            card.id
        );
        assert_eq!(mana.costs, [AbilityCost::Tap], "{} mana cost", card.id);
        assert!(mana.is_mana_ability(), "{} mana ability", card.id);
        assert!(
            matches!(
                mana.effect.as_slice(),
                [SpellEffectKind::ProduceMana { options, .. }]
                    if options.len() == 1
                        && (options[0].w, options[0].u, options[0].b,
                            options[0].r, options[0].g, options[0].c) == card.color
            ),
            "{} mana option",
            card.id
        );

        assert_eq!(cycling.ability_id.as_str(), "activated_02", "{}", card.id);
        assert_eq!(
            cycling.presentation,
            AbilityPresentation::OracleLines(vec![3]),
            "{} cycling presentation",
            card.id
        );
        assert_eq!(cycling.source_zone, AbilitySourceZone::Hand, "{}", card.id);
        assert!(
            matches!(
                cycling.costs.as_slice(),
                [AbilityCost::Mana(cost), AbilityCost::DiscardSelf]
                    if cost.to_string() == card.cycling
            ),
            "{} cycling cost",
            card.id
        );
        assert!(
            matches!(
                cycling.effect.as_slice(),
                [SpellEffectKind::Draw {
                    count: Amount::Fixed(1),
                    ..
                }]
            ),
            "{} cycling draw",
            card.id
        );
        assert!(cycling.targeting.is_none(), "{}", card.id);
    }
}

#[test]
fn desert_cycling_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for card in CARDS {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{}\t", card.id)))
            .unwrap_or_else(|| panic!("missing fingerprint row for {}", card.id));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[0], card.id);
        assert_eq!(fields[1], card.name);
        assert_eq!(fields[2], card.id);
        assert_eq!(
            fields[4], card.fingerprint,
            "fingerprint drift for {}",
            card.id
        );
    }
}
