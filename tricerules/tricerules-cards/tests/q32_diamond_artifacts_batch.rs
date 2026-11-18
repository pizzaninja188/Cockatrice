//! Registry conformance for the queue-v32 Diamond artifact cycle.
use tricerules_cards::primitives::{AbilityCost, SpellEffectKind, StaticAbilityDef};
use tricerules_cards::{AbilityPresentation, Layout};

struct Card {
    id: &'static str,
    name: &'static str,
    color: char,
    fingerprint: &'static str,
}

const CARDS: &[Card] = &[
    Card {
        id: "charcoal_diamond",
        name: "Charcoal Diamond",
        color: 'b',
        fingerprint: "80510197bd1121569653ae9d9423e97ba123833f5662f00eb81abf83f1d618bc",
    },
    Card {
        id: "fire_diamond",
        name: "Fire Diamond",
        color: 'r',
        fingerprint: "e7c7c8141de31ef6214a638c702a84bb89c6a5ba778caa76e00c4571110281eb",
    },
    Card {
        id: "marble_diamond",
        name: "Marble Diamond",
        color: 'w',
        fingerprint: "e09f394c4a44d0b341f30372fcabdbe6b277778251f036e9f233b1bea547b7f6",
    },
    Card {
        id: "moss_diamond",
        name: "Moss Diamond",
        color: 'g',
        fingerprint: "94e689caaf474125872a4d20011684af9925178c606b21891be1564da9202eb5",
    },
    Card {
        id: "sky_diamond",
        name: "Sky Diamond",
        color: 'u',
        fingerprint: "41d5e5ca38db6b70895be1d98602e2979a91f6e61ad7ab509685c4336b916a6a",
    },
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
fn q32_diamond_artifacts_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for card in CARDS {
        let definition = registry
            .get(card.id)
            .unwrap_or_else(|| panic!("missing {}", card.id));
        assert_eq!(definition.layout, Layout::Normal, "{}", card.id);
        assert_eq!(registry.id_for_name(card.name), Some(card.id));
        let f = definition.primary_face();
        assert_eq!(f.face_id.as_str(), card.id.replace('-', "_"));
        assert_eq!(f.name, card.name);
        assert_eq!(f.mana_cost.to_string(), "{2}");
        assert_eq!(f.types, ["Artifact"]);
        assert_eq!((f.power, f.toughness), (None, None));
        assert!(f.keywords.is_empty());
        assert!(f.triggered_abilities.is_empty());
        assert!(f.spell_effect.is_empty());
        assert!(f.targeting.is_none());
        let [ability] = f.activated_abilities.as_slice() else {
            panic!("{}: one activated ability", card.id)
        };
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![2])
        );
        assert_eq!(ability.costs, [AbilityCost::Tap]);
        assert!(ability.is_mana_ability());
        assert!(
            matches!(
                ability.effect.as_slice(),
                [SpellEffectKind::ProduceMana { options, .. }]
                    if options.len() == 1 && amounts(&options[0]) == mask(card.color)
            ),
            "{}: printed mana option",
            card.id
        );
        let [static_ability] = f.static_abilities.as_slice() else {
            panic!("{}: one static ability", card.id)
        };
        assert_eq!(
            static_ability.presentation,
            AbilityPresentation::OracleLines(vec![1])
        );
        assert!(matches!(
            static_ability.definition,
            StaticAbilityDef::EntersTapped { .. }
        ));
    }
}

#[test]
fn q32_diamond_artifacts_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for card in CARDS {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{}\t", card.id)))
            .unwrap_or_else(|| panic!("missing fingerprint row for {}", card.id));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "{}", card.id);
        assert_eq!(fields[0], card.id);
        assert_eq!(fields[1], card.name);
        assert_eq!(
            fields[4], card.fingerprint,
            "fingerprint drift for {}",
            card.id
        );
    }
}
