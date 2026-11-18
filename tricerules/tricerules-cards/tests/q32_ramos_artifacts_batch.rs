//! Registry conformance for the queue-v32 Ramos artifact cycle.
use tricerules_cards::primitives::{AbilityCost, SpellEffectKind};
use tricerules_cards::{AbilityPresentation, Layout};

struct Card {
    id: &'static str,
    name: &'static str,
    color: char,
    fingerprint: &'static str,
}

const CARDS: &[Card] = &[
    Card {
        id: "eye_of_ramos",
        name: "Eye of Ramos",
        color: 'u',
        fingerprint: "d6767096e8c02a9ad5f86b8910afbc7f1f47f56272f7c47586f114b2f44c11ec",
    },
    Card {
        id: "heart_of_ramos",
        name: "Heart of Ramos",
        color: 'r',
        fingerprint: "2824f9043b4f181d315f453413bc0268938711945352eeeaf9e99e1e69508abd",
    },
    Card {
        id: "horn_of_ramos",
        name: "Horn of Ramos",
        color: 'g',
        fingerprint: "a17cffe392e3711ab1f4e7f73ea840d544ca967f0bfe01353efcc9fae90ca712",
    },
    Card {
        id: "skull_of_ramos",
        name: "Skull of Ramos",
        color: 'b',
        fingerprint: "6777d03311409bed4220f4987b6ea96690b3ae4bc5b1f4fff48b046afc4b1958",
    },
    Card {
        id: "tooth_of_ramos",
        name: "Tooth of Ramos",
        color: 'w',
        fingerprint: "4669cd494f26b502c5f6f2ac34cfa1e9ba5c627a1f90e042910371e628b4a75c",
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
fn q32_ramos_artifacts_batch_maps_definitions() {
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
        assert_eq!(f.mana_cost.to_string(), "{3}");
        assert_eq!(f.types, ["Artifact"]);
        assert_eq!((f.power, f.toughness), (None, None));
        assert!(f.keywords.is_empty());
        assert!(f.triggered_abilities.is_empty());
        assert!(f.spell_effect.is_empty());
        assert!(f.static_abilities.is_empty());
        assert!(f.targeting.is_none());
        let [tap_ability, sacrifice_ability] = f.activated_abilities.as_slice() else {
            panic!("{}: two activated abilities", card.id)
        };
        assert_eq!(
            tap_ability.presentation,
            AbilityPresentation::OracleLines(vec![1])
        );
        assert_eq!(tap_ability.costs, [AbilityCost::Tap]);
        assert!(tap_ability.is_mana_ability());
        assert_eq!(
            sacrifice_ability.presentation,
            AbilityPresentation::OracleLines(vec![2])
        );
        assert_eq!(sacrifice_ability.costs, [AbilityCost::SacrificeSelf]);
        assert!(sacrifice_ability.is_mana_ability());
        for ability in [tap_ability, sacrifice_ability] {
            assert!(
                matches!(
                    ability.effect.as_slice(),
                    [SpellEffectKind::ProduceMana { options, .. }]
                        if options.len() == 1 && amounts(&options[0]) == mask(card.color)
                ),
                "{}: printed mana option",
                card.id
            );
        }
    }
}

#[test]
fn q32_ramos_artifacts_batch_fingerprints_match_the_pinned_oracle_text() {
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
