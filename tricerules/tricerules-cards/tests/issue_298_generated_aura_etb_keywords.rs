use tricerules_cards::primitives::{
    EffectSubject, SpellEffectKind, StaticAbilityDef, TargetController, TargetKind,
};
use tricerules_cards::{AbilityPresentation, CardRegistry, Keyword, TriggerCondition};

struct ExpectedCard {
    id: &'static str,
    name: &'static str,
    face_id: &'static str,
    mana_cost: &'static str,
    controller: TargetController,
    keyword: Keyword,
    delta_power: i32,
    delta_toughness: i32,
    static_keyword: Option<Keyword>,
    oracle_text_sha256: &'static str,
}

const COHORT: &[ExpectedCard] = &[
    ExpectedCard {
        id: "super_speed",
        name: "Super Speed",
        face_id: "super_speed",
        mana_cost: "{R}",
        controller: TargetController::Any,
        keyword: Keyword::FirstStrike,
        delta_power: 1,
        delta_toughness: 0,
        static_keyword: Some(Keyword::Haste),
        oracle_text_sha256: "5dc07dbebfddfed5d6b6d9f7f8f933e123e1e22126f9e33053718c26e201cb26",
    },
    ExpectedCard {
        id: "fire-rim_form",
        name: "Fire-Rim Form",
        face_id: "fire_rim_form",
        mana_cost: "{1}{R}",
        controller: TargetController::Any,
        keyword: Keyword::FirstStrike,
        delta_power: 2,
        delta_toughness: 0,
        static_keyword: None,
        oracle_text_sha256: "c796d239581c336ae6df3d682cec57e925be7d4d68258a793a8c0815383ec12e",
    },
    ExpectedCard {
        id: "aquitects_defenses",
        name: "Aquitect's Defenses",
        face_id: "aquitect_s_defenses",
        mana_cost: "{1}{U}",
        controller: TargetController::You,
        keyword: Keyword::Hexproof,
        delta_power: 1,
        delta_toughness: 2,
        static_keyword: None,
        oracle_text_sha256: "3250bd3d5eb212dc9028cf5ed689baec9de006769242a42583b158a43eadcda6",
    },
    ExpectedCard {
        id: "fae_flight",
        name: "Fae Flight",
        face_id: "fae_flight",
        mana_cost: "{1}{U}",
        controller: TargetController::Any,
        keyword: Keyword::Hexproof,
        delta_power: 1,
        delta_toughness: 0,
        static_keyword: Some(Keyword::Flying),
        oracle_text_sha256: "658a716a559c053839e7813117b03f31bf99ad1f0d4ea5732c233a3804b549f6",
    },
];

#[test]
fn issue_298_generated_auras_have_exact_flash_attach_etb_and_static_shapes() {
    let registry = CardRegistry::global();
    for expected in COHORT {
        let id = expected.id;
        let definition = registry
            .get(id)
            .unwrap_or_else(|| panic!("missing issue #298 card {id}"));
        assert_eq!(definition.name, expected.name);
        assert_eq!(registry.id_for_name(expected.name), Some(id));
        assert_eq!(definition.face_count(), 1);
        let face = definition.primary_face();
        assert_eq!(face.face_id.as_str(), expected.face_id);
        assert_eq!(face.name, expected.name);
        assert_eq!(face.mana_cost.to_string(), expected.mana_cost);
        let presentation = registry
            .presentation_face(id, face.face_id.as_str())
            .unwrap_or_else(|| panic!("missing {id} presentation metadata"));
        assert_eq!(presentation.card_name, expected.name);
        assert_eq!(presentation.face_name, expected.name);
        assert_eq!(presentation.oracle_text_sha256, expected.oracle_text_sha256);
        assert_eq!(face.types, ["Enchantment", "Aura"]);
        assert_eq!(face.keywords, [Keyword::Flash]);
        assert!(face.activated_abilities.is_empty());
        assert!(face.characteristic_defining_abilities.is_empty());
        assert!(face.custom_effect.is_none());
        assert!(face.modal_spell.is_none());
        assert!(face.targeting.is_none());

        let [SpellEffectKind::AuraAttach { target }] = face.spell_effect.as_slice() else {
            panic!("{id} should have exactly one AuraAttach spell effect");
        };
        assert_eq!(target.kind, TargetKind::Creature);
        assert_eq!(target.controller, expected.controller);

        let [trigger] = face.triggered_abilities.as_slice() else {
            panic!("{id} should have exactly one ETB trigger");
        };
        assert_eq!(trigger.ability_id.as_str(), "triggered_01");
        assert_eq!(
            trigger.presentation,
            AbilityPresentation::OracleLines(vec![3])
        );
        assert_eq!(trigger.trigger, TriggerCondition::WhenSelfEntersBattlefield);
        assert!(!trigger.may);
        assert!(trigger.targeting.is_none());
        assert!(trigger.modal.is_none());
        assert!(trigger.intervening_if.is_none());
        assert!(!trigger.triggers_only_once);
        assert_eq!(
            trigger.effect,
            [SpellEffectKind::GrantKeywords {
                subject: EffectSubject::AttachedObject,
                keywords: vec![expected.keyword],
            }]
        );

        let [static_ability] = face.static_abilities.as_slice() else {
            panic!("{id} should have exactly one attached static modifier");
        };
        assert_eq!(
            static_ability.presentation,
            AbilityPresentation::OracleLines(vec![4])
        );
        assert!(matches!(
            &static_ability.definition,
            StaticAbilityDef::AttachedModifier {
                delta_power,
                delta_toughness,
                keywords,
                ..
            } if *delta_power == expected.delta_power
                && *delta_toughness == expected.delta_toughness
                && keywords.as_slice()
                    == expected.static_keyword.into_iter().collect::<Vec<_>>().as_slice()
        ));
    }
}
