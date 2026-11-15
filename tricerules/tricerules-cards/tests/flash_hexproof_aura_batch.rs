//! Registry conformance for the reviewed flash / hexproof Aura cohort.
//!
//! Every included card is a normal-layout Aura with Flash, "Enchant creature you
//! control", an enters trigger granting hexproof, and a static attached pump.
use tricerules_cards::primitives::{
    EffectSubject, SpellEffectKind, StaticAbilityDef, TargetController, TargetKind,
};
use tricerules_cards::{AbilityPresentation, Keyword, Layout, TriggerCondition};

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
        id: "accelerated_evolution",
        name: "Accelerated Evolution",
        mana_cost: "{2}{G}",
        delta_power: 2,
        delta_toughness: 2,
        oracle_text_sha256: "420033e8b9b723aec53a9343fcdc9f198a2565babd71a55bfdf997bc505e449c",
    },
    ExpectedCard {
        id: "cradle_of_safety",
        name: "Cradle of Safety",
        mana_cost: "{1}{U}",
        delta_power: 1,
        delta_toughness: 1,
        oracle_text_sha256: "d94db9faa59cf339145d8fe0cce48012fddba4e430bbd7e20560d665e55dcfde",
    },
    ExpectedCard {
        id: "starlit_mantle",
        name: "Starlit Mantle",
        mana_cost: "{1}{U}",
        delta_power: 1,
        delta_toughness: 1,
        oracle_text_sha256: "7481bce6a5b115d421c9b8df0e15ba281fd207d56a9cf8e4e2f984ebd3bd8c4c",
    },
];

#[test]
fn flash_hexproof_aura_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for expected in CARDS {
        let id = expected.id;
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        assert_eq!(definition.name, expected.name);
        assert_eq!(registry.id_for_name(expected.name), Some(id));
        assert_eq!(definition.face_count(), 1);
        let face = definition.primary_face();
        assert_eq!(face.face_id.as_str(), id.replace('-', "_"), "{id} face id");
        assert_eq!(face.name, expected.name);
        assert_eq!(face.mana_cost.to_string(), expected.mana_cost, "{id}");
        let presentation = registry
            .presentation_face(id, face.face_id.as_str())
            .unwrap_or_else(|| panic!("missing {id} presentation metadata"));
        assert_eq!(presentation.card_name, expected.name);
        assert_eq!(presentation.face_name, expected.name);
        assert_eq!(
            presentation.oracle_text_sha256, expected.oracle_text_sha256,
            "{id} fingerprint"
        );
        assert_eq!(face.types, ["Enchantment", "Aura"], "{id}");
        assert_eq!(face.keywords, [Keyword::Flash], "{id}");
        assert!(face.activated_abilities.is_empty(), "{id}");
        assert!(face.characteristic_defining_abilities.is_empty(), "{id}");
        assert!(face.custom_effect.is_none(), "{id}");
        assert!(face.modal_spell.is_none(), "{id}");
        assert!(face.targeting.is_none(), "{id}");

        let [SpellEffectKind::AuraAttach { target }] = face.spell_effect.as_slice() else {
            panic!("{id} should have exactly one AuraAttach spell effect");
        };
        assert_eq!(target.kind, TargetKind::Creature, "{id}");
        assert_eq!(target.controller, TargetController::You, "{id}");

        let [trigger] = face.triggered_abilities.as_slice() else {
            panic!("{id} should have exactly one ETB trigger");
        };
        assert_eq!(trigger.ability_id.as_str(), "triggered_01", "{id}");
        assert_eq!(
            trigger.presentation,
            AbilityPresentation::OracleLines(vec![3]),
            "{id}"
        );
        assert_eq!(
            trigger.trigger,
            TriggerCondition::WhenSelfEntersBattlefield,
            "{id}"
        );
        assert!(!trigger.may, "{id}");
        assert!(trigger.targeting.is_none(), "{id}");
        assert!(trigger.modal.is_none(), "{id}");
        assert!(trigger.intervening_if.is_none(), "{id}");
        assert!(!trigger.triggers_only_once, "{id}");
        assert_eq!(
            trigger.effect,
            [SpellEffectKind::GrantKeywords {
                subject: EffectSubject::AttachedObject,
                keywords: vec![Keyword::Hexproof],
            }],
            "{id}"
        );

        let [static_ability] = face.static_abilities.as_slice() else {
            panic!("{id} should have exactly one attached static modifier");
        };
        assert_eq!(
            static_ability.presentation,
            AbilityPresentation::OracleLines(vec![4]),
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
                    && keywords.is_empty()
            ),
            "{id}: attached pump"
        );
    }
}

#[test]
fn flash_hexproof_aura_batch_fingerprints_match_the_pinned_oracle_text() {
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
