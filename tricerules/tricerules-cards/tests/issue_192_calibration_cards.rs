use tricerules_cards::primitives::{
    CombatRestrictionScope, DrawDiscardOrder, EffectSubject, PlayerRecipient, ResolutionCost,
    SpellEffectKind, StaticAbilityDef, TargetController, TargetKind,
};
use tricerules_cards::{
    AbilityCost, AbilitySourceZone, ActivationTiming, CardRegistry, Color, CounterKind, Keyword,
    SearchDestination, TriggerCondition,
};

struct ExpectedCard {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    types: &'static [&'static str],
    stats: (Option<u32>, Option<u32>),
    keywords: &'static [Keyword],
}

const COHORT: &[ExpectedCard] = &[
    ExpectedCard {
        id: "zog,_triceraton_castaway",
        name: "Zog, Triceraton Castaway",
        mana_cost: "{4}{R}",
        types: &["Creature", "Dinosaur", "Soldier"],
        stats: (Some(5), Some(4)),
        keywords: &[Keyword::Reach, Keyword::Trample],
    },
    ExpectedCard {
        id: "donatello,_turtle_techie",
        name: "Donatello, Turtle Techie",
        mana_cost: "{3}{U}",
        types: &["Creature", "Mutant", "Ninja", "Turtle"],
        stats: (Some(3), Some(4)),
        keywords: &[],
    },
    ExpectedCard {
        id: "mutant_town_musicians",
        name: "Mutant Town Musicians",
        mana_cost: "{2}{R}",
        types: &["Creature", "Mutant", "Bard", "Performer"],
        stats: (Some(2), Some(4)),
        keywords: &[Keyword::Trample],
    },
    ExpectedCard {
        id: "punk_frogs",
        name: "Punk Frogs",
        mana_cost: "{3}{G/U}{G/U}",
        types: &["Creature", "Frog", "Mutant", "Rebel"],
        stats: (Some(4), Some(5)),
        keywords: &[],
    },
    ExpectedCard {
        id: "dimension_x",
        name: "Dimension X",
        mana_cost: "",
        types: &["Land"],
        stats: (None, None),
        keywords: &[],
    },
    ExpectedCard {
        id: "return_to_the_sewers",
        name: "Return to the Sewers",
        mana_cost: "{3}{U}",
        types: &["Instant"],
        stats: (None, None),
        keywords: &[],
    },
    ExpectedCard {
        id: "april_oneil,_kunoichi_trainee",
        name: "April O'Neil, Kunoichi Trainee",
        mana_cost: "{1}{W}",
        types: &["Creature", "Human", "Ninja"],
        stats: (Some(2), Some(2)),
        keywords: &[],
    },
    ExpectedCard {
        id: "hamato_guardian_stance",
        name: "Hamato Guardian Stance",
        mana_cost: "{W}",
        types: &["Instant"],
        stats: (None, None),
        keywords: &[],
    },
    ExpectedCard {
        id: "epf_point_squad",
        name: "EPF Point Squad",
        mana_cost: "{1}{R/W}{R/W}",
        types: &["Creature", "Human", "Soldier"],
        stats: (Some(2), Some(1)),
        keywords: &[],
    },
    ExpectedCard {
        id: "tenderize",
        name: "Tenderize",
        mana_cost: "{1}{G}",
        types: &["Instant"],
        stats: (None, None),
        keywords: &[],
    },
    ExpectedCard {
        id: "jennika,_bad_apple_big_sister",
        name: "Jennika, Bad Apple Big Sister",
        mana_cost: "{4}{W}",
        types: &["Creature", "Mutant", "Ninja", "Turtle"],
        stats: (Some(3), Some(3)),
        keywords: &[],
    },
    ExpectedCard {
        id: "bot_bashing_time",
        name: "Bot Bashing Time",
        mana_cost: "{3}{R}",
        types: &["Sorcery"],
        stats: (None, None),
        keywords: &[],
    },
    ExpectedCard {
        id: "null_group_biological_assets",
        name: "Null Group Biological Assets",
        mana_cost: "{2}{R}",
        types: &["Creature", "Mutant", "Mercenary"],
        stats: (Some(3), Some(1)),
        keywords: &[],
    },
    ExpectedCard {
        id: "featherbrained_filcher",
        name: "Featherbrained Filcher",
        mana_cost: "{W}",
        types: &["Creature", "Bird", "Mutant"],
        stats: (Some(0), Some(2)),
        keywords: &[Keyword::Flying],
    },
    ExpectedCard {
        id: "spicy_oatmeal_pizza",
        name: "Spicy Oatmeal Pizza",
        mana_cost: "{2}{R}",
        types: &["Artifact", "Food"],
        stats: (None, None),
        keywords: &[],
    },
    ExpectedCard {
        id: "genghis_frog",
        name: "Genghis Frog",
        mana_cost: "{G}{U}",
        types: &["Creature", "Frog", "Mutant", "Rogue"],
        stats: (Some(1), Some(3)),
        keywords: &[Keyword::Trample],
    },
    ExpectedCard {
        id: "ooze_spill",
        name: "Ooze Spill",
        mana_cost: "{1}{U}{U}",
        types: &["Instant"],
        stats: (None, None),
        keywords: &[],
    },
    ExpectedCard {
        id: "skateboard",
        name: "Skateboard",
        mana_cost: "{1}",
        types: &["Artifact", "Equipment"],
        stats: (None, None),
        keywords: &[],
    },
];

#[test]
fn issue_192_cohort_has_exact_oracle_characteristics() {
    let registry = CardRegistry::global();
    for expected in COHORT {
        let definition = registry
            .get(expected.id)
            .unwrap_or_else(|| panic!("missing issue #192 card {}", expected.id));
        assert_eq!(definition.name, expected.name, "{}", expected.id);
        assert_eq!(registry.id_for_name(expected.name), Some(expected.id));
        let face = definition.primary_face();
        assert_eq!(
            face.mana_cost.to_string(),
            expected.mana_cost,
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
        assert_eq!(
            face.face_id.as_str(),
            expected.id.replace(",_", "_").as_str(),
            "{} stable face identity",
            expected.id
        );
        let legendary = matches!(
            expected.id,
            "zog,_triceraton_castaway"
                | "donatello,_turtle_techie"
                | "april_oneil,_kunoichi_trainee"
                | "jennika,_bad_apple_big_sister"
                | "genghis_frog"
        );
        let expected_supertypes: &[&str] = if legendary { &["Legendary"] } else { &[] };
        assert_eq!(
            face.supertypes.as_slice(),
            expected_supertypes,
            "{} supertypes",
            expected.id
        );
    }
}

#[test]
fn issue_192_tokens_have_exact_registry_definitions() {
    let registry = CardRegistry::global();
    let mutagen = registry
        .get("mutagen")
        .expect("Mutagen token")
        .primary_face();
    assert!(registry.is_token("mutagen"));
    assert_eq!(mutagen.types, ["Artifact", "Mutagen"]);
    let ability = &mutagen.activated_abilities[0];
    assert_eq!(ability.timing, ActivationTiming::SorcerySpeed);
    assert!(
        matches!(ability.costs.as_slice(), [AbilityCost::Mana(cost), AbilityCost::Tap, AbilityCost::SacrificeSelf] if cost.to_string() == "{1}")
    );
    assert!(matches!(
        ability.effect.as_slice(),
        [SpellEffectKind::PutCounters {
            counter: CounterKind::PlusOnePlusOne,
            count: tricerules_cards::Amount::Fixed(1),
            subject: EffectSubject::Chosen(target),
        }] if target.kind == TargetKind::Creature
    ));

    let mutant = registry
        .get("mutant_r_2_2")
        .expect("red 2/2 Mutant token")
        .primary_face();
    assert!(registry.is_token("mutant_r_2_2"));
    assert_eq!(mutant.types, ["Creature", "Mutant"]);
    assert_eq!(mutant.colors_override.as_ref(), Some(&vec![Color::Red]));
    assert_eq!((mutant.power, mutant.toughness), (Some(2), Some(2)));
}

#[test]
fn issue_192_uses_generic_trigger_combat_and_token_shapes() {
    let registry = CardRegistry::global();
    let zog = registry
        .get("zog,_triceraton_castaway")
        .unwrap()
        .primary_face();
    assert!(matches!(
        zog.triggered_abilities[0].effect.as_slice(),
        [SpellEffectKind::ApplyCombatRestriction {
            scope: CombatRestrictionScope::Chosen(target),
            restriction,
        }] if target.kind == TargetKind::Creature && restriction.cant_block
    ));
    let cycling = &zog.activated_abilities[0];
    assert_eq!(cycling.source_zone, AbilitySourceZone::Hand);
    assert!(
        matches!(cycling.costs.as_slice(), [AbilityCost::Mana(cost), AbilityCost::DiscardSelf] if cost.to_string() == "{2}")
    );
    assert!(matches!(
        cycling.effect.as_slice(),
        [SpellEffectKind::SearchLibrary {
            destination: SearchDestination::Hand,
            reveal: true,
            shuffle: true,
            ..
        }]
    ));

    for card_id in ["mutant_town_musicians", "epf_point_squad"] {
        let face = registry.get(card_id).unwrap().primary_face();
        assert!(matches!(
            &face.triggered_abilities[0].trigger,
            TriggerCondition::WheneverPermanentEntersBattlefield {
                controller: tricerules_cards::CastTriggerPlayer::Controller,
                filter,
                ..
            } if filter.exclude_source
        ));
    }

    let punk_frogs = registry.get("punk_frogs").unwrap().primary_face();
    assert!(matches!(
        punk_frogs.triggered_abilities[0].trigger,
        TriggerCondition::WheneverSelfBecomesTarget { .. }
    ));
    assert!(matches!(
        punk_frogs.triggered_abilities[0].effect.as_slice(),
        [SpellEffectKind::CounterTriggeringStackObjectUnlessPays { cost: ResolutionCost::Mana(cost) }]
            if cost.to_string() == "{3}"
    ));

    let feather = registry
        .get("featherbrained_filcher")
        .unwrap()
        .primary_face();
    assert_eq!(
        feather.triggered_abilities[0].trigger,
        TriggerCondition::WhenSelfLeavesBattlefield
    );
    assert!(matches!(
        feather.triggered_abilities[0].effect.as_slice(),
        [SpellEffectKind::CreateTokens { token, .. }] if token == "food"
    ));

    let genghis = registry.get("genghis_frog").unwrap().primary_face();
    assert!(matches!(
        &genghis.triggered_abilities[0].trigger,
        TriggerCondition::WheneverPermanentEntersBattlefield { filter, creature_filter: Some(creature_filter), .. }
            if !filter.exclude_source && creature_filter.required_subtypes == ["Mutant"]
    ));
}

#[test]
fn issue_192_uses_generic_spell_equipment_and_turn_shapes() {
    let registry = CardRegistry::global();
    let return_to_sewers = registry.get("return_to_the_sewers").unwrap().primary_face();
    assert!(matches!(
        return_to_sewers.spell_effect.as_slice(),
        [SpellEffectKind::PutInOwnersLibrary { .. }, SpellEffectKind::CreateTokens { token, .. }]
            if token == "mutagen"
    ));

    let tenderize = registry.get("tenderize").unwrap().primary_face();
    assert!(matches!(
        tenderize.spell_effect.as_slice(),
        [SpellEffectKind::CreatureDealsDamageEqualToPower { source, target }]
            if source.controller == TargetController::You && target.controller == TargetController::NotYou
    ));

    let bot_bashing = registry.get("bot_bashing_time").unwrap().primary_face();
    assert!(matches!(
        bot_bashing.spell_effect.as_slice(),
        [
            SpellEffectKind::DamageTarget {
                amount: tricerules_cards::Amount::Fixed(6),
                ..
            },
            SpellEffectKind::ExileIfWouldDieThisTurn { .. }
        ]
    ));

    let null_group = registry
        .get("null_group_biological_assets")
        .unwrap()
        .primary_face();
    assert!(matches!(
        &null_group.static_abilities[0].definition,
        StaticAbilityDef::ConditionalSelfModifier { keywords, .. } if keywords == &[Keyword::FirstStrike]
    ));
    assert!(matches!(
        null_group.triggered_abilities[0].effect.as_slice(),
        [SpellEffectKind::DrawDiscard {
            draw_count: 1,
            discard_count: 1,
            order: DrawDiscardOrder::DiscardThenDraw,
            optional: true,
            ..
        }]
    ));

    let pizza = registry.get("spicy_oatmeal_pizza").unwrap().primary_face();
    assert!(matches!(
        pizza.triggered_abilities[0].effect.as_slice(),
        [
            SpellEffectKind::DamageTarget {
                amount: tricerules_cards::Amount::Fixed(4),
                ..
            },
            SpellEffectKind::DamagePlayer {
                amount: tricerules_cards::Amount::Fixed(3),
                who: PlayerRecipient::Controller
            }
        ]
    ));

    let ooze_spill = registry.get("ooze_spill").unwrap().primary_face();
    assert!(matches!(
        ooze_spill.spell_effect.as_slice(),
        [SpellEffectKind::CounterTargetSpell { .. }, SpellEffectKind::CreateTokens { token, .. }]
            if token == "mutagen"
    ));

    let skateboard = registry.get("skateboard").unwrap().primary_face();
    assert!(matches!(
        &skateboard.static_abilities[0].definition,
        StaticAbilityDef::AttachedModifier { delta_power: 1, keywords, .. }
            if keywords == &[Keyword::Haste]
    ));
    assert!(matches!(
        skateboard.triggered_abilities[0].effect.as_slice(),
        [SpellEffectKind::Tap { subject: EffectSubject::Chosen(target) }]
            if target.kind == TargetKind::AnyPermanent
    ));
}
