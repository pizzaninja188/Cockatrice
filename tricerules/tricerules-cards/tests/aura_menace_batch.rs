//! Registry conformance for the reviewed menace Aura cohort.
//!
//! Every included card is a normal-layout Aura that enchants any creature and
//! grants the printed P/T pump plus menace through one attached modifier.
use tricerules_cards::primitives::{SpellEffectKind, StaticAbilityDef};
use tricerules_cards::{AbilityPresentation, Keyword, Layout};

struct ExpectedCard {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    delta_power: i32,
    delta_toughness: i32,
    oracle_text_sha256: &'static str,
}

const CARDS: &[ExpectedCard] = &[
    ExpectedCard {
        id: "frenzied_rage",
        name: "Frenzied Rage",
        mana_cost: "{1}{R}",
        delta_power: 2,
        delta_toughness: 1,
        oracle_text_sha256: "1d7fc2a1ce6272e7b3bfde263a3bcda425ec0321eb9dfb4fce5f9e281fc38226",
    },
    ExpectedCard {
        id: "furious_strength",
        name: "Furious Strength",
        mana_cost: "{2}{R}",
        delta_power: 2,
        delta_toughness: 2,
        oracle_text_sha256: "6975399c8f6c45bc74ab6d120e530b429d8ef3d228ec2000e3499630bec19087",
    },
    ExpectedCard {
        id: "giant_spectacle",
        name: "Giant Spectacle",
        mana_cost: "{1}{R}",
        delta_power: 2,
        delta_toughness: 1,
        oracle_text_sha256: "81ddc804fdc87683a5be1419f962402819e17ffeb800bdc2dca5d686c5f04c0d",
    },
    ExpectedCard {
        id: "madcap_skills",
        name: "Madcap Skills",
        mana_cost: "{1}{R}",
        delta_power: 3,
        delta_toughness: 0,
        oracle_text_sha256: "6e7bb298f7c7116a13ccf551619246079599b57be2774051442ce57ce1bea624",
    },
];

#[test]
fn aura_menace_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for expected in CARDS {
        let id = expected.id;
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        assert_eq!(definition.name, expected.name);
        assert_eq!(registry.id_for_name(expected.name), Some(id));
        assert_eq!(definition.face_count(), 1);
        let face = definition.primary_face();
        assert_eq!(face.face_id.as_str(), id.replace('-', "_"));
        assert_eq!(face.name, expected.name);
        assert_eq!(face.mana_cost.to_string(), expected.mana_cost, "{id}");
        assert_eq!(face.types, ["Enchantment", "Aura"], "{id}");
        assert!(face.keywords.is_empty(), "{id}");
        assert!(face.activated_abilities.is_empty(), "{id}");
        assert!(face.triggered_abilities.is_empty(), "{id}");
        assert!(face.targeting.is_none(), "{id}");
        assert_eq!(face.spell_effect.len(), 1, "{id}");
        assert!(
            matches!(
                &face.spell_effect[0],
                SpellEffectKind::AuraAttach { target }
                    if target.kind == tricerules_cards::primitives::TargetKind::Creature
                        && target.controller
                            == tricerules_cards::primitives::TargetController::Any
            ),
            "{id}: enchants any creature"
        );
        let [static_ability] = face.static_abilities.as_slice() else {
            panic!("{id}: exactly one attached static modifier");
        };
        assert_eq!(static_ability.ability_id.as_str(), "static_01", "{id}");
        assert_eq!(
            static_ability.presentation,
            AbilityPresentation::OracleLines(vec![2]),
            "{id}"
        );
        assert!(
            matches!(
                &static_ability.definition,
                StaticAbilityDef::AttachedModifier {
                    delta_power,
                    delta_toughness,
                    keywords,
                    ..
                } if *delta_power == expected.delta_power
                    && *delta_toughness == expected.delta_toughness
                    && keywords.as_slice() == [Keyword::Menace]
            ),
            "{id}: printed pump plus menace"
        );
    }
}

#[test]
fn aura_menace_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for expected in CARDS {
        let id = expected.id;
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")))
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}"));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[0], id);
        assert_eq!(fields[1], expected.name);
        assert_eq!(fields[2], id.replace('-', "_"));
        assert_eq!(
            fields[4], expected.oracle_text_sha256,
            "fingerprint drift for {id}"
        );
    }
}
