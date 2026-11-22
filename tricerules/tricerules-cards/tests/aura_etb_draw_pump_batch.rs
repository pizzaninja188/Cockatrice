//! Registry conformance for the queue-v18 Aura enters-draw pump cohort.
//!
//! Every included card is a normal-layout Aura that enchants a creature, draws a
//! card when it enters, and gives the enchanted creature a printed P/T bonus.
use tricerules_cards::primitives::{Amount, PlayerRecipient, SpellEffectKind, StaticAbilityDef};
use tricerules_cards::{AbilityPresentation, Layout, TriggerCondition};

struct ExpectedCard {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    pump: (i32, i32),
    oracle_text_sha256: &'static str,
}

const CARDS: &[ExpectedCard] = &[
    ExpectedCard {
        id: "chosen_by_heliod",
        name: "Chosen by Heliod",
        mana_cost: "{1}{W}",
        pump: (0, 2),
        oracle_text_sha256: "9fc843254d5f599e8d88bc23725fbca5c8ed7de7581ff85e91f49ed6a437f5de",
    },
    ExpectedCard {
        id: "scavenged_weaponry",
        name: "Scavenged Weaponry",
        mana_cost: "{2}{B}",
        pump: (1, 1),
        oracle_text_sha256: "93db7246cb555d1c79ba9e743542a938ab872764fc87636887e5fdfac40af414",
    },
    ExpectedCard {
        id: "scourgemark",
        name: "Scourgemark",
        mana_cost: "{1}{B}",
        pump: (1, 0),
        oracle_text_sha256: "980c3d8766939d22251d6377de45c332c061e0f9ff41a9613b1748d77d73f79d",
    },
    ExpectedCard {
        id: "sheltering_boughs",
        name: "Sheltering Boughs",
        mana_cost: "{2}{G}",
        pump: (1, 3),
        oracle_text_sha256: "04b1a38f4b11d16ceae2dbe2b4353bdc1d9b149ad8976189340cf38584e91ec7",
    },
];

#[test]
fn aura_etb_draw_pump_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for expected in CARDS {
        let id = expected.id;
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        assert_eq!(definition.name, expected.name);
        assert_eq!(registry.id_for_name(expected.name), Some(id));
        let face = definition.primary_face();
        assert_eq!(face.face_id.as_str(), id.replace('-', "_"), "{id}");
        assert_eq!(face.mana_cost.to_string(), expected.mana_cost, "{id}");
        assert_eq!(face.types, ["Enchantment", "Aura"], "{id}");
        assert!(face.keywords.is_empty(), "{id}");
        assert!(face.activated_abilities.is_empty(), "{id}");
        assert!(face.targeting.is_none(), "{id}");
        let [SpellEffectKind::AuraAttach { target }] = face.spell_effect.as_slice() else {
            panic!("{id}: exactly one AuraAttach");
        };
        assert_eq!(
            target.kind,
            tricerules_cards::primitives::TargetKind::Creature,
            "{id}: enchant creature"
        );
        let [trigger] = face.triggered_abilities.as_slice() else {
            panic!("{id}: exactly one triggered ability");
        };
        assert_eq!(trigger.ability_id.as_str(), "triggered_01", "{id}");
        assert_eq!(
            trigger.presentation,
            AbilityPresentation::OracleLines(vec![2]),
            "{id}"
        );
        assert_eq!(
            trigger.trigger,
            TriggerCondition::WhenSelfEntersBattlefield,
            "{id}"
        );
        assert_eq!(
            trigger.effect,
            [SpellEffectKind::Draw {
                who: PlayerRecipient::Controller,
                count: Amount::Fixed(1),
            }],
            "{id}"
        );
        assert!(trigger.targeting.is_none(), "{id}");
        let [static_ability] = face.static_abilities.as_slice() else {
            panic!("{id}: exactly one static ability");
        };
        assert_eq!(
            static_ability.presentation,
            AbilityPresentation::OracleLines(vec![3]),
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
                } if *delta_power == expected.pump.0
                    && *delta_toughness == expected.pump.1
                    && keywords.is_empty()
            ),
            "{id}: attached printed pump"
        );
    }
}

#[test]
fn aura_etb_draw_pump_batch_fingerprints_match_the_pinned_oracle_text() {
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
