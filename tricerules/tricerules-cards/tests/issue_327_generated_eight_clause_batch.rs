//! Registry and presentation conformance for issue #327.
//!
//! The eight exact generator templates must register the reviewed Standard identities with their
//! complete typed payloads: CR 122 counter placement on a chosen target; CR 602/611.2a the
//! parameterized {R} self-pump; CR 611.2a the -4/-4 pump; CR 611.3/613.4c the live
//! attacking-creature anthem (attacking per CR 508.1k); CR 603.2b/513.2/701.21 the each-end-step
//! sacrifice; CR 107.5/701.9 the tap-plus-discard activation cost; CR 122 the ETB mass counter
//! excluding the source; and CR 603.6c/700.4 the another-creature-dies counter trigger.

use tricerules_cards::primitives::{
    CastTriggerPlayer, CreatureScopeController, CreatureScopeFilter, EffectSubject,
    PermanentEventFilter, PermanentTypeFilter, PlayerRecipient, SpellEffectKind, StaticAbilityDef,
    TargetFilter, TargetGroupDef, TargetingDef,
};
use tricerules_cards::{
    AbilityCost, AbilityPresentation, Amount, CardRegistry, Color, CounterKind, Keyword, Layout,
    ManaCost, TriggerCondition,
};

fn single_group(targeting: &TargetingDef) -> &TargetGroupDef {
    let [group] = targeting.groups.as_slice() else {
        panic!("expected exactly one authored target group");
    };
    group
}

#[test]
fn issue_327_registers_the_eight_reviewed_identities() {
    let registry = CardRegistry::global();
    for (id, name, face_count, layout) in [
        ("honor", "Honor", 1, Layout::Normal),
        ("shivan_dragon", "Shivan Dragon", 1, Layout::Normal),
        ("dark_deed", "Dark Deed", 1, Layout::Normal),
        ("goblin_oriflamme", "Goblin Oriflamme", 1, Layout::Normal),
        ("ball_lightning", "Ball Lightning", 1, Layout::Normal),
        (
            "charging_strifeknight",
            "Charging Strifeknight",
            1,
            Layout::Normal,
        ),
        ("web-warriors", "Web-Warriors", 1, Layout::Normal),
        ("voracious_vermin", "Voracious Vermin", 1, Layout::Normal),
    ] {
        let definition = registry
            .get(id)
            .unwrap_or_else(|| panic!("missing reviewed card {id}"));
        assert_eq!(definition.name, name);
        assert_eq!(registry.id_for_name(name), Some(id));
        assert_eq!(definition.layout, layout);
        assert_eq!(definition.face_count(), face_count);
    }
}

#[test]
fn issue_327_honor_counters_one_target_and_draws() {
    let definition = CardRegistry::global().get("honor").expect("Honor");
    let face = definition.primary_face();
    assert_eq!(face.face_id.as_str(), "honor");
    assert_eq!(face.mana_cost.to_string(), "{W}");
    assert_eq!(face.types, ["Sorcery"]);
    assert_eq!(face.colors(), vec![Color::White]);
    assert_eq!(
        face.spell_effect,
        [
            SpellEffectKind::PutCounters {
                counter: CounterKind::PlusOnePlusOne,
                count: Amount::Fixed(1),
                subject: EffectSubject::Chosen(Box::new(TargetFilter::default_creature())),
            },
            SpellEffectKind::Draw {
                who: PlayerRecipient::Controller,
                count: Amount::Fixed(1),
            },
        ]
    );
    let targeting = face.targeting.as_ref().expect("Honor targets a creature");
    let group = single_group(targeting);
    assert_eq!((group.min, group.max), (1, 1));
    assert_eq!(group.prompt, "Choose target creature");
    assert_eq!(group.effect_indices, [0]);
    assert!(tricerules_cards::primitives::TargetSchema::compile(
        &face.spell_effect,
        Some(targeting)
    )
    .is_ok());
}

#[test]
fn issue_327_shivan_dragon_keeps_flying_and_single_red_self_pump() {
    let definition = CardRegistry::global()
        .get("shivan_dragon")
        .expect("Shivan Dragon");
    let face = definition.primary_face();
    assert_eq!(face.face_id.as_str(), "shivan_dragon");
    assert_eq!(face.mana_cost.to_string(), "{4}{R}{R}");
    assert_eq!(face.types, ["Creature", "Dragon"]);
    assert_eq!((face.power, face.toughness), (Some(5), Some(5)));
    assert_eq!(face.colors(), vec![Color::Red]);
    assert_eq!(face.keywords, [Keyword::Flying]);
    assert!(face.spell_effect.is_empty());
    assert!(face.triggered_abilities.is_empty());

    let [ability] = face.activated_abilities.as_slice() else {
        panic!("Shivan Dragon must have exactly one activated ability");
    };
    assert_eq!(ability.ability_id.as_str(), "activated_01");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(
        ability.costs,
        vec![AbilityCost::Mana(ManaCost::parse("{R}").unwrap())]
    );
    assert_eq!(
        ability.effect,
        [SpellEffectKind::PumpTarget {
            power: 1,
            toughness: 0,
            scale: None,
            subject: EffectSubject::Source,
        }]
    );
    assert!(ability.targeting.is_none());
}

#[test]
fn issue_327_dark_deed_pumps_minus_four_minus_four() {
    let definition = CardRegistry::global().get("dark_deed").expect("Dark Deed");
    let face = definition.primary_face();
    assert_eq!(face.face_id.as_str(), "dark_deed");
    assert_eq!(face.mana_cost.to_string(), "{1}{B}");
    assert_eq!(face.types, ["Instant"]);
    assert_eq!(face.colors(), vec![Color::Black]);
    assert_eq!(
        face.spell_effect,
        [SpellEffectKind::PumpTarget {
            power: -4,
            toughness: -4,
            scale: None,
            subject: EffectSubject::Chosen(Box::new(TargetFilter::default_creature())),
        }]
    );
    assert!(face.targeting.is_none());
}

#[test]
fn issue_327_goblin_oriflamme_anthems_attacking_creatures_only() {
    let definition = CardRegistry::global()
        .get("goblin_oriflamme")
        .expect("Goblin Oriflamme");
    let face = definition.primary_face();
    assert_eq!(face.face_id.as_str(), "goblin_oriflamme");
    assert_eq!(face.mana_cost.to_string(), "{1}{R}");
    assert_eq!(face.types, ["Enchantment"]);
    assert_eq!(face.colors(), vec![Color::Red]);

    let [ability] = face.static_abilities.as_slice() else {
        panic!("Goblin Oriflamme must have exactly one static ability");
    };
    assert_eq!(ability.ability_id.as_str(), "static_01");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(
        ability.definition,
        StaticAbilityDef::AnthemPt {
            filter: CreatureScopeFilter {
                controller: Some(CreatureScopeController::YouControl),
                attacking: true,
                ..CreatureScopeFilter::default()
            },
            condition: None,
            delta_power: 1,
            delta_toughness: 0,
        }
    );
}

#[test]
fn issue_327_ball_lightning_sacrifices_itself_at_the_end_step() {
    let definition = CardRegistry::global()
        .get("ball_lightning")
        .expect("Ball Lightning");
    let face = definition.primary_face();
    assert_eq!(face.face_id.as_str(), "ball_lightning");
    assert_eq!(face.mana_cost.to_string(), "{R}{R}{R}");
    assert_eq!(face.types, ["Creature", "Elemental"]);
    assert_eq!((face.power, face.toughness), (Some(6), Some(1)));
    assert_eq!(face.colors(), vec![Color::Red]);
    assert_eq!(face.keywords, [Keyword::Trample, Keyword::Haste]);

    let [ability] = face.triggered_abilities.as_slice() else {
        panic!("Ball Lightning must have exactly one triggered ability");
    };
    assert_eq!(ability.ability_id.as_str(), "triggered_01");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![3])
    );
    assert_eq!(
        ability.trigger,
        TriggerCondition::AtBeginningOfEndStep {
            player: CastTriggerPlayer::AnyPlayer,
        }
    );
    assert_eq!(
        ability.effect,
        [SpellEffectKind::Sacrifice {
            subject: EffectSubject::Source,
        }]
    );
    assert!(ability.targeting.is_none());
}

#[test]
fn issue_327_charging_strifeknight_taps_and_discards_for_a_card() {
    let definition = CardRegistry::global()
        .get("charging_strifeknight")
        .expect("Charging Strifeknight");
    let face = definition.primary_face();
    assert_eq!(face.face_id.as_str(), "charging_strifeknight");
    assert_eq!(face.mana_cost.to_string(), "{2}{R}");
    assert_eq!(face.types, ["Creature", "Spirit", "Knight"]);
    assert_eq!((face.power, face.toughness), (Some(3), Some(3)));
    assert_eq!(face.colors(), vec![Color::Red]);
    assert_eq!(face.keywords, [Keyword::Haste]);

    let [ability] = face.activated_abilities.as_slice() else {
        panic!("Charging Strifeknight must have exactly one activated ability");
    };
    assert_eq!(ability.ability_id.as_str(), "activated_01");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(ability.costs, vec![AbilityCost::Tap, AbilityCost::Discard]);
    assert_eq!(
        ability.effect,
        [SpellEffectKind::Draw {
            who: PlayerRecipient::Controller,
            count: Amount::Fixed(1),
        }]
    );
    assert!(ability.targeting.is_none());
}

#[test]
fn issue_327_web_warriors_counters_each_other_controlled_creature() {
    let definition = CardRegistry::global()
        .get("web-warriors")
        .expect("Web-Warriors");
    let face = definition.primary_face();
    assert_eq!(face.face_id.as_str(), "web_warriors");
    assert_eq!(face.mana_cost.to_string(), "{4}{G/W}");
    assert_eq!(face.types, ["Creature", "Spider", "Hero"]);
    assert_eq!((face.power, face.toughness), (Some(4), Some(3)));
    let colors = face.colors();
    assert_eq!(colors.len(), 2);
    assert!(colors.contains(&Color::Green) && colors.contains(&Color::White));
    assert!(face.spell_effect.is_empty());

    let [ability] = face.triggered_abilities.as_slice() else {
        panic!("Web-Warriors must have exactly one ETB ability");
    };
    assert_eq!(ability.ability_id.as_str(), "triggered_01");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(ability.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert_eq!(
        ability.effect,
        [SpellEffectKind::PutCountersAll {
            counter: CounterKind::PlusOnePlusOne,
            count: Amount::Fixed(1),
            filter: CreatureScopeFilter {
                controller: Some(CreatureScopeController::YouControl),
                exclude_self: true,
                ..CreatureScopeFilter::default()
            },
        }]
    );
    assert!(ability.targeting.is_none());
    assert!(!ability.may);
}

#[test]
fn issue_327_voracious_vermin_keeps_the_rat_etb_and_dies_counter() {
    let definition = CardRegistry::global()
        .get("voracious_vermin")
        .expect("Voracious Vermin");
    let face = definition.primary_face();
    assert_eq!(face.face_id.as_str(), "voracious_vermin");
    assert_eq!(face.mana_cost.to_string(), "{2}{B}");
    assert_eq!(face.types, ["Creature", "Rat"]);
    assert_eq!((face.power, face.toughness), (Some(2), Some(1)));
    assert_eq!(face.colors(), vec![Color::Black]);
    assert!(face.spell_effect.is_empty());

    let [etb, dies] = face.triggered_abilities.as_slice() else {
        panic!("Voracious Vermin must keep exactly two triggered abilities");
    };
    assert_eq!(etb.ability_id.as_str(), "triggered_01");
    assert_eq!(etb.presentation, AbilityPresentation::OracleLines(vec![1]));
    assert_eq!(etb.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert_eq!(
        etb.effect,
        [SpellEffectKind::CreateTokens {
            token: "rat_b_1_1_cant_block".into(),
            count: Amount::Fixed(1),
            who: PlayerRecipient::Controller,
            tapped: false,
            sacrifice_timing: None,
        }]
    );

    assert_eq!(dies.ability_id.as_str(), "triggered_02");
    assert_eq!(dies.presentation, AbilityPresentation::OracleLines(vec![2]));
    assert_eq!(
        dies.trigger,
        TriggerCondition::WheneverCreatureDies {
            controller: CastTriggerPlayer::Controller,
            filter: PermanentEventFilter {
                permanent_type: Some(PermanentTypeFilter::Creature),
                exclude_source: true,
                ..PermanentEventFilter::default()
            },
        }
    );
    assert_eq!(
        dies.effect,
        [SpellEffectKind::PutCounters {
            counter: CounterKind::PlusOnePlusOne,
            count: Amount::Fixed(1),
            subject: EffectSubject::Source,
        }]
    );
    assert!(dies.targeting.is_none());
}

#[test]
fn issue_327_fingerprint_rows_match_the_presentation_registry() {
    let registry = CardRegistry::global();
    let cases: &[(&str, &str, &str, &str)] = &[
        ("honor", "honor", "Honor", "Honor"),
        (
            "shivan_dragon",
            "shivan_dragon",
            "Shivan Dragon",
            "Shivan Dragon",
        ),
        ("dark_deed", "dark_deed", "Dark Deed", "Dark Deed"),
        (
            "goblin_oriflamme",
            "goblin_oriflamme",
            "Goblin Oriflamme",
            "Goblin Oriflamme",
        ),
        (
            "ball_lightning",
            "ball_lightning",
            "Ball Lightning",
            "Ball Lightning",
        ),
        (
            "charging_strifeknight",
            "charging_strifeknight",
            "Charging Strifeknight",
            "Charging Strifeknight",
        ),
        (
            "web-warriors",
            "web_warriors",
            "Web-Warriors",
            "Web-Warriors",
        ),
        (
            "voracious_vermin",
            "voracious_vermin",
            "Voracious Vermin",
            "Voracious Vermin",
        ),
    ];
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, face_id, card_name, face_name) in cases {
        let presentation = registry
            .presentation_face(id, face_id)
            .unwrap_or_else(|| panic!("missing presentation metadata for {id}/{face_id}"));
        assert_eq!(presentation.card_name, *card_name);
        assert_eq!(presentation.face_name, *face_name);
        assert_eq!(presentation.oracle_text_sha256.len(), 64);

        let row = fingerprints
            .lines()
            .find(|line| {
                line.starts_with(&format!("{id}\t")) && line.contains(&format!("\t{face_id}\t"))
            })
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}/{face_id}"));
        assert!(
            row.ends_with(&presentation.oracle_text_sha256),
            "fingerprint drift for {id}/{face_id}: {row}"
        );
    }
}
