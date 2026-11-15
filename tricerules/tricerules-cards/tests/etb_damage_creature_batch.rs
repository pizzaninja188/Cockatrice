//! Registry conformance for the reviewed enters-damage creature cohort.
//!
//! Every included card is a normal-layout creature with one enters trigger that
//! deals exactly two damage to a creature target and no other rules text.
use tricerules_cards::primitives::{SpellEffectKind, TargetKind, TriggerCondition};
use tricerules_cards::{AbilityPresentation, Amount, Layout};

struct ExpectedCard {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    types: &'static [&'static str],
    body: (u32, u32),
    oracle_text_sha256: &'static str,
}

const CARDS: &[ExpectedCard] = &[
    ExpectedCard {
        id: "corrupt_eunuchs",
        name: "Corrupt Eunuchs",
        mana_cost: "{3}{R}",
        types: &["Creature", "Human", "Advisor"],
        body: (2, 2),
        oracle_text_sha256: "78dca8b4c14edb9a5d6ca2a922e36ee706ee567966674855cbd76d27b59af258",
    },
    ExpectedCard {
        id: "fire_imp",
        name: "Fire Imp",
        mana_cost: "{2}{R}",
        types: &["Creature", "Imp"],
        body: (2, 1),
        oracle_text_sha256: "78dca8b4c14edb9a5d6ca2a922e36ee706ee567966674855cbd76d27b59af258",
    },
    ExpectedCard {
        id: "goblin_commando",
        name: "Goblin Commando",
        mana_cost: "{4}{R}",
        types: &["Creature", "Goblin"],
        body: (2, 2),
        oracle_text_sha256: "78dca8b4c14edb9a5d6ca2a922e36ee706ee567966674855cbd76d27b59af258",
    },
];

#[test]
fn etb_damage_creature_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for expected in CARDS {
        let id = expected.id;
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        assert_eq!(definition.name, expected.name);
        assert_eq!(registry.id_for_name(expected.name), Some(id));
        let face = definition.primary_face();
        assert_eq!(face.face_id.as_str(), id.replace('-', "_"));
        assert_eq!(face.mana_cost.to_string(), expected.mana_cost, "{id}");
        assert_eq!(face.types, expected.types, "{id}");
        assert!(face.supertypes.is_empty(), "{id}");
        assert_eq!(
            (face.power, face.toughness),
            (Some(expected.body.0), Some(expected.body.1)),
            "{id}"
        );
        assert!(face.keywords.is_empty(), "{id}");
        assert!(face.activated_abilities.is_empty(), "{id}");
        assert!(face.static_abilities.is_empty(), "{id}");
        assert!(face.spell_effect.is_empty(), "{id}");
        assert!(face.targeting.is_none(), "{id}");
        let [ability] = face.triggered_abilities.as_slice() else {
            panic!("{id}: exactly one triggered ability");
        };
        assert_eq!(ability.ability_id.as_str(), "triggered_01", "{id}");
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{id} presentation"
        );
        assert_eq!(
            ability.trigger,
            TriggerCondition::WhenSelfEntersBattlefield,
            "{id}"
        );
        assert!(
            matches!(
                ability.effect.as_slice(),
                [SpellEffectKind::DamageTarget {
                    amount: Amount::Fixed(2),
                    target,
                    ..
                }] if target.kind == TargetKind::Creature
            ),
            "{id}: exactly two damage to a creature"
        );
        assert!(
            ability.targeting.is_none(),
            "{id}: the effect owns its target"
        );
    }
}

#[test]
fn etb_damage_creature_batch_fingerprints_match_the_pinned_oracle_text() {
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
