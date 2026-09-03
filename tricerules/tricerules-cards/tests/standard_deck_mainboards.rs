use tricerules_cards::primitives::{
    CardTypeFilter, CombatRestrictionScope, EffectSubject, GameCondition, ObjectCastCostKind,
    SpellEffectKind, StaticAbilityDef, TargetKind,
};
use tricerules_cards::{
    Amount, CardRegistry, CastCostOptionDef, CounterKind, Keyword, SearchDestination,
    TriggerCondition,
};

struct ExpectedCard {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    supertypes: &'static [&'static str],
    types: &'static [&'static str],
    stats: (Option<u32>, Option<u32>),
    keywords: &'static [Keyword],
}

const COHORT: &[ExpectedCard] = &[
    ExpectedCard {
        id: "dream_beavers",
        name: "Dream Beavers",
        mana_cost: "{B}",
        supertypes: &[],
        types: &["Creature", "Beaver", "Nightmare"],
        stats: (Some(1), Some(1)),
        keywords: &[Keyword::Flying],
    },
    ExpectedCard {
        id: "gloomlake_verge",
        name: "Gloomlake Verge",
        mana_cost: "",
        supertypes: &[],
        types: &["Land"],
        stats: (None, None),
        keywords: &[],
    },
    ExpectedCard {
        id: "requiting_hex",
        name: "Requiting Hex",
        mana_cost: "{B}",
        supertypes: &[],
        types: &["Instant"],
        stats: (None, None),
        keywords: &[],
    },
    ExpectedCard {
        id: "shoot_the_sheriff",
        name: "Shoot the Sheriff",
        mana_cost: "{1}{B}",
        supertypes: &[],
        types: &["Instant"],
        stats: (None, None),
        keywords: &[],
    },
    ExpectedCard {
        id: "spell_pierce",
        name: "Spell Pierce",
        mana_cost: "{U}",
        supertypes: &[],
        types: &["Instant"],
        stats: (None, None),
        keywords: &[],
    },
    ExpectedCard {
        id: "we_say_thee_nay!",
        name: "We Say Thee Nay!",
        mana_cost: "{1}{U}",
        supertypes: &[],
        types: &["Instant", "Arcane"],
        stats: (None, None),
        keywords: &[],
    },
    ExpectedCard {
        id: "ba_sing_se",
        name: "Ba Sing Se",
        mana_cost: "",
        supertypes: &[],
        types: &["Land"],
        stats: (None, None),
        keywords: &[],
    },
    ExpectedCard {
        id: "escape_tunnel",
        name: "Escape Tunnel",
        mana_cost: "",
        supertypes: &[],
        types: &["Land"],
        stats: (None, None),
        keywords: &[],
    },
    ExpectedCard {
        id: "glimpse_the_core",
        name: "Glimpse the Core",
        mana_cost: "{1}{G}",
        supertypes: &[],
        types: &["Sorcery"],
        stats: (None, None),
        keywords: &[],
    },
    ExpectedCard {
        id: "promising_vein",
        name: "Promising Vein",
        mana_cost: "",
        supertypes: &[],
        types: &["Land", "Cave"],
        stats: (None, None),
        keywords: &[],
    },
    ExpectedCard {
        id: "sazhs_chocobo",
        name: "Sazh's Chocobo",
        mana_cost: "{G}",
        supertypes: &[],
        types: &["Creature", "Bird"],
        stats: (Some(0), Some(1)),
        keywords: &[],
    },
];

#[test]
fn current_standard_mainboard_cohort_has_exact_oracle_characteristics() {
    let registry = CardRegistry::global();
    for expected in COHORT {
        let definition = registry
            .get(expected.id)
            .unwrap_or_else(|| panic!("missing current Standard mainboard card {}", expected.id));
        assert_eq!(definition.name, expected.name, "{}", expected.id);
        assert_eq!(registry.id_for_name(expected.name), Some(expected.id));
        let face = definition.primary_face();
        let expected_face_id = expected.id.trim_end_matches('!');
        assert_eq!(
            face.face_id.as_str(),
            expected_face_id,
            "{} face id",
            expected.id
        );
        assert_eq!(
            face.mana_cost.to_string(),
            expected.mana_cost,
            "{}",
            expected.id
        );
        assert_eq!(
            face.supertypes.as_slice(),
            expected.supertypes,
            "{}",
            expected.id
        );
        assert_eq!(
            face.types.iter().map(String::as_str).collect::<Vec<_>>(),
            expected.types,
            "{}",
            expected.id
        );
        assert_eq!(
            (face.power, face.toughness),
            expected.stats,
            "{}",
            expected.id
        );
        assert_eq!(
            face.keywords.as_slice(),
            expected.keywords,
            "{}",
            expected.id
        );
    }
}

#[test]
fn dimir_cohort_uses_authoritative_cost_target_and_trigger_shapes() {
    let registry = CardRegistry::global();

    let dream = registry.get("dream_beavers").unwrap().primary_face();
    assert_eq!(
        dream.triggered_abilities[0].trigger,
        TriggerCondition::WhenSelfEntersBattlefield
    );
    assert!(matches!(
        dream.triggered_abilities[0].effect.as_slice(),
        [
            SpellEffectKind::EachOpponentLosesLifeYouGainEqual { amount: 1 },
            SpellEffectKind::Scry {
                count: Amount::Fixed(1)
            }
        ]
    ));

    let verge = registry.get("gloomlake_verge").unwrap().primary_face();
    assert!(matches!(
        verge.activated_abilities[1].conditions.as_slice(),
        [GameCondition::BattlefieldAggregate { min: Some(1), .. }]
    ));

    let hex = registry.get("requiting_hex").unwrap().primary_face();
    assert!(matches!(
        hex.cast_cost_groups[0].options.as_slice(),
        [CastCostOptionDef::Blight { count: 1, .. }]
    ));
    assert!(matches!(
        hex.spell_effect.as_slice(),
        [
            SpellEffectKind::Destroy {
                subject: EffectSubject::Chosen(target)
            },
            SpellEffectKind::ConditionalCastCost { effect, .. }
        ] if target.max_mana_value == Some(2)
            && matches!(effect.as_ref(), SpellEffectKind::GainLife { amount: Amount::Fixed(2) })
    ));

    let sheriff = registry.get("shoot_the_sheriff").unwrap().primary_face();
    assert!(matches!(
        sheriff.spell_effect.as_slice(),
        [SpellEffectKind::Destroy {
            subject: EffectSubject::Chosen(target)
        }] if target.excluded_subtypes
            == ["Assassin", "Mercenary", "Pirate", "Rogue", "Warlock"]
    ));

    let pierce = registry.get("spell_pierce").unwrap().primary_face();
    assert!(matches!(
        pierce.spell_effect.as_slice(),
        [SpellEffectKind::CounterTargetSpell {
            spell_filter: Some(CardTypeFilter::Noncreature),
            unless_controller_pays: Some(Amount::Fixed(2)),
            ..
        }]
    ));

    let nay = registry.get("we_say_thee_nay!").unwrap().primary_face();
    assert!(matches!(
        nay.cast_cost_groups[0].options.as_slice(),
        [CastCostOptionDef::TapPermanents {
            kind: ObjectCastCostKind::Teamwork,
            ..
        }]
    ));
    assert!(matches!(
        nay.spell_effect.as_slice(),
        [SpellEffectKind::CounterTargetSpell {
            unless_controller_pays_by_cast_cost: Some(amount),
            ..
        }] if amount.if_selected == 4 && amount.otherwise == 2
    ));
}

#[test]
fn landfall_cohort_uses_authoritative_entry_search_and_combat_shapes() {
    let registry = CardRegistry::global();

    let ba_sing_se = registry.get("ba_sing_se").unwrap().primary_face();
    assert!(matches!(
        &ba_sing_se.static_abilities[0].definition,
        StaticAbilityDef::EntersTapped {
            condition: Some(GameCondition::BattlefieldAggregate { max: Some(0), .. }),
            ..
        }
    ));
    assert!(matches!(
        ba_sing_se.activated_abilities[1].effect.as_slice(),
        [SpellEffectKind::Earthbend {
            count: Amount::Fixed(2)
        }]
    ));

    let tunnel = registry.get("escape_tunnel").unwrap().primary_face();
    assert!(matches!(
        tunnel.activated_abilities[0].effect.as_slice(),
        [SpellEffectKind::SearchLibrary {
            destination: SearchDestination::Battlefield { tapped: true },
            ..
        }]
    ));
    assert!(matches!(
        tunnel.activated_abilities[1].effect.as_slice(),
        [SpellEffectKind::ApplyCombatRestriction {
            scope: CombatRestrictionScope::Chosen(target),
            restriction,
        }] if target.kind == TargetKind::Creature
            && restriction.cant_be_blocked
    ));

    let glimpse = registry.get("glimpse_the_core").unwrap().primary_face();
    let modal = glimpse.modal_spell.as_ref().expect("Glimpse modes");
    assert_eq!(modal.modes.len(), 2);
    assert!(matches!(
        modal.modes[0].effects.as_slice(),
        [SpellEffectKind::SearchLibrary {
            destination: SearchDestination::Battlefield { tapped: true },
            ..
        }]
    ));
    assert!(matches!(
        modal.modes[1].effects.as_slice(),
        [SpellEffectKind::MoveGraveyardCards { .. }]
    ));

    let vein = registry.get("promising_vein").unwrap().primary_face();
    assert!(matches!(
        vein.activated_abilities[1].effect.as_slice(),
        [SpellEffectKind::SearchLibrary {
            destination: SearchDestination::Battlefield { tapped: true },
            ..
        }]
    ));

    let chocobo = registry.get("sazhs_chocobo").unwrap().primary_face();
    assert!(matches!(
        &chocobo.triggered_abilities[0].trigger,
        TriggerCondition::WheneverPermanentEntersBattlefield { filter, .. }
            if filter.permanent_type == Some(tricerules_cards::PermanentTypeFilter::Land)
    ));
    assert!(matches!(
        chocobo.triggered_abilities[0].effect.as_slice(),
        [SpellEffectKind::PutCounters {
            counter: CounterKind::PlusOnePlusOne,
            count: Amount::Fixed(1),
            subject: EffectSubject::Source,
        }]
    ));
}
