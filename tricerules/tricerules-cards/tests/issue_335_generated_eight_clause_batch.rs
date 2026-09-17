//! Registry and presentation conformance for issue #335.
//!
//! The eight generated Standard identities must register with their complete typed payloads:
//! CR 508/119.3 the attack trigger whose each-opponent life loss excludes its controller;
//! CR 603.6c/119.3 the dies gain-two-life trigger; CR 603.6a the self-or-other creature entry
//! trigger with the source included; CR 701.13/115 the mandatory creature-or-enchantment exile;
//! CR 603.6a the land's own entry gain-two-life trigger; CR 604.1/611.3/613.1f layer 6 the
//! counter-filtered trample anthem; CR 603.6a/611.2a the Landfall self pump; and CR 602.2/701.9
//! the {1}{U}, {T} draw-then-discard loot activation.

use tricerules_cards::primitives::{
    CreatureScopeController, CreatureScopeFilter, DrawDiscardOrder, EffectSubject, LifeAmount,
    PermanentEventFilter, PermanentTypeFilter, PlayerRecipient, StaticAbilityDef, TargetFilter,
    TargetKind,
};
use tricerules_cards::{
    AbilityCost, AbilityPresentation, Amount, CardRegistry, CastTriggerPlayer, Color, CounterKind,
    Keyword, Layout, ManaAmount, ManaCost, SpellEffectKind, TriggerCondition,
};

const ISSUE_335_CARDS: &[(&str, &str, &str)] = &[
    ("pulse_tracker", "Pulse Tracker", "pulse_tracker"),
    (
        "grasping_longneck",
        "Grasping Longneck",
        "grasping_longneck",
    ),
    ("bogwater_lumaret", "Bogwater Lumaret", "bogwater_lumaret"),
    ("angelic_edict", "Angelic Edict", "angelic_edict"),
    ("adventurers_inn", "Adventurer's Inn", "adventurer_s_inn"),
    ("drix_fatemaker", "Drix Fatemaker", "drix_fatemaker"),
    ("attercop", "Attercop", "attercop"),
    ("strix_lookout", "Strix Lookout", "strix_lookout"),
];

#[test]
fn issue_335_registers_the_eight_reviewed_identities() {
    let registry = CardRegistry::global();
    for (id, name, face_id) in ISSUE_335_CARDS {
        let definition = registry
            .get(id)
            .unwrap_or_else(|| panic!("missing reviewed card {id}"));
        assert_eq!(definition.name, *name);
        assert_eq!(registry.id_for_name(name), Some(*id));
        assert_eq!(definition.layout, Layout::Normal);
        assert_eq!(definition.face_count(), 1);
        assert_eq!(definition.primary_face().face_id.as_str(), *face_id);
    }
}

#[test]
fn issue_335_pulse_tracker_drains_each_opponent_on_attack() {
    let definition = CardRegistry::global()
        .get("pulse_tracker")
        .expect("Pulse Tracker");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{B}");
    assert_eq!(face.types, ["Creature", "Vampire", "Rogue"]);
    assert_eq!((face.power, face.toughness), (Some(1), Some(1)));
    assert_eq!(face.colors(), vec![Color::Black]);
    assert!(face.keywords.is_empty());
    assert!(face.activated_abilities.is_empty());
    assert!(face.static_abilities.is_empty());

    let [ability] = face.triggered_abilities.as_slice() else {
        panic!("Pulse Tracker must have exactly one triggered ability");
    };
    assert_eq!(
        ability.trigger,
        TriggerCondition::WheneverSelfAttacks {
            minimum_other_attackers: 0,
        }
    );
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(
        ability.effect,
        [SpellEffectKind::LoseLife {
            amount: LifeAmount::Fixed(1),
            who: PlayerRecipient::EachOpponent,
        }]
    );
    assert!(ability.targeting.is_none());
    assert!(!ability.may);
    assert!(ability.intervening_if.is_none());
}

#[test]
fn issue_335_grasping_longneck_gains_two_life_when_it_dies() {
    let definition = CardRegistry::global()
        .get("grasping_longneck")
        .expect("Grasping Longneck");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{2}{G}");
    assert_eq!(face.types, ["Enchantment", "Creature", "Horror"]);
    assert_eq!((face.power, face.toughness), (Some(4), Some(2)));
    assert_eq!(face.colors(), vec![Color::Green]);
    assert_eq!(face.keywords, [Keyword::Reach]);
    assert!(face.activated_abilities.is_empty());
    assert!(face.static_abilities.is_empty());

    let [ability] = face.triggered_abilities.as_slice() else {
        panic!("Grasping Longneck must have exactly one triggered ability");
    };
    assert_eq!(ability.trigger, TriggerCondition::WhenSelfDies);
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(
        ability.effect,
        [SpellEffectKind::GainLife {
            amount: Amount::Fixed(2),
        }]
    );
    assert!(ability.targeting.is_none());
    assert!(!ability.may);
    assert!(ability.intervening_if.is_none());
}

#[test]
fn issue_335_bogwater_lumaret_gains_one_life_for_inclusive_creature_entries() {
    let definition = CardRegistry::global()
        .get("bogwater_lumaret")
        .expect("Bogwater Lumaret");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{B}{G}");
    assert_eq!(face.types, ["Creature", "Spirit", "Frog"]);
    assert_eq!((face.power, face.toughness), (Some(2), Some(2)));
    assert_eq!(face.colors(), vec![Color::Black, Color::Green]);
    assert!(face.keywords.is_empty());

    let [ability] = face.triggered_abilities.as_slice() else {
        panic!("Bogwater Lumaret must have exactly one triggered ability");
    };
    assert_eq!(
        ability.trigger,
        TriggerCondition::WheneverPermanentEntersBattlefield {
            controller: CastTriggerPlayer::Controller,
            filter: PermanentEventFilter {
                permanent_type: Some(PermanentTypeFilter::Creature),
                ..PermanentEventFilter::default()
            },
            creature_filter: None,
        }
    );
    let TriggerCondition::WheneverPermanentEntersBattlefield { filter, .. } = &ability.trigger
    else {
        unreachable!("the trigger was just asserted as a creature entry event");
    };
    assert!(
        !filter.exclude_source,
        "CR 603.6a: \"this creature or another\" must include the source's own entry"
    );
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(
        ability.effect,
        [SpellEffectKind::GainLife {
            amount: Amount::Fixed(1),
        }]
    );
    assert!(ability.targeting.is_none());
}

#[test]
fn issue_335_angelic_edict_exiles_a_creature_or_enchantment_union() {
    let definition = CardRegistry::global()
        .get("angelic_edict")
        .expect("Angelic Edict");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{4}{W}");
    assert_eq!(face.types, ["Sorcery"]);
    assert_eq!(face.colors(), vec![Color::White]);
    assert!(face.triggered_abilities.is_empty());
    assert!(face.activated_abilities.is_empty());

    assert_eq!(
        face.spell_effect,
        [SpellEffectKind::Exile {
            subject: EffectSubject::Chosen(Box::new(TargetFilter {
                kind: TargetKind::AnyPermanent,
                permanent_types: vec![
                    PermanentTypeFilter::Creature,
                    PermanentTypeFilter::Enchantment,
                ],
                ..TargetFilter::default()
            })),
        }]
    );
    assert!(
        face.targeting.is_none(),
        "the single targeted instruction uses the implicit one-target contract"
    );
}

#[test]
fn issue_335_adventurers_inn_gains_two_life_on_its_own_entry() {
    let definition = CardRegistry::global()
        .get("adventurers_inn")
        .expect("Adventurer's Inn");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "");
    assert_eq!(face.types, ["Land", "Town"]);
    assert!(face.colors().is_empty());
    assert!(face.keywords.is_empty());

    let [trigger] = face.triggered_abilities.as_slice() else {
        panic!("Adventurer's Inn must have exactly one triggered ability");
    };
    assert_eq!(trigger.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert_eq!(
        trigger.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(
        trigger.effect,
        [SpellEffectKind::GainLife {
            amount: Amount::Fixed(2),
        }]
    );
    assert!(trigger.targeting.is_none());

    let [ability] = face.activated_abilities.as_slice() else {
        panic!("Adventurer's Inn must keep its printed tap mana ability");
    };
    assert_eq!(ability.ability_id.as_str(), "activated_01");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(ability.costs, [AbilityCost::Tap]);
    assert_eq!(
        ability.effect,
        [SpellEffectKind::ProduceMana {
            options: vec![ManaAmount {
                c: 1,
                ..ManaAmount::default()
            }],
            restriction: None,
            conditional: None,
        }]
    );
    assert!(ability.targeting.is_none());
}

#[test]
fn issue_335_drix_fatemaker_keeps_its_warp_counter_and_counter_filtered_anthem() {
    let definition = CardRegistry::global()
        .get("drix_fatemaker")
        .expect("Drix Fatemaker");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{3}{G}");
    assert_eq!(
        face.warp_cost.as_ref().map(ToString::to_string),
        Some("{1}{G}".to_string())
    );
    assert_eq!(face.types, ["Creature", "Drix", "Wizard"]);
    assert_eq!((face.power, face.toughness), (Some(3), Some(2)));
    assert_eq!(face.colors(), vec![Color::Green]);
    assert!(
        face.keywords.is_empty(),
        "Warp is a face cast-method cost, not a battlefield keyword"
    );
    assert!(face.activated_abilities.is_empty());

    let [trigger] = face.triggered_abilities.as_slice() else {
        panic!("Drix Fatemaker must keep exactly one ETB trigger");
    };
    assert_eq!(trigger.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert_eq!(
        trigger.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(
        trigger.effect,
        [SpellEffectKind::PutCounters {
            counter: CounterKind::PlusOnePlusOne,
            count: Amount::Fixed(1),
            subject: EffectSubject::Chosen(Box::new(TargetFilter::default_creature())),
        }]
    );
    let targeting = trigger.targeting.as_ref().expect("Drix ETB target group");
    let [group] = targeting.groups.as_slice() else {
        panic!("Drix Fatemaker must own exactly one target group");
    };
    assert_eq!((group.min, group.max), (1, 1));
    assert_eq!(group.prompt, "Choose target creature");
    assert_eq!(group.effect_indices, [0]);

    let [anthem] = face.static_abilities.as_slice() else {
        panic!("Drix Fatemaker must own exactly one static ability");
    };
    assert_eq!(
        anthem.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(
        anthem.definition,
        StaticAbilityDef::AnthemKeyword {
            filter: CreatureScopeFilter {
                controller: Some(CreatureScopeController::YouControl),
                required_counter: Some(CounterKind::PlusOnePlusOne),
                ..CreatureScopeFilter::default()
            },
            condition: None,
            keyword: Keyword::Trample,
        }
    );
}

#[test]
fn issue_335_attercop_keeps_reach_deathtouch_and_landfall_pump() {
    let definition = CardRegistry::global().get("attercop").expect("Attercop");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{1}{G}");
    assert_eq!(face.types, ["Creature", "Spider"]);
    assert_eq!((face.power, face.toughness), (Some(2), Some(1)));
    assert_eq!(face.colors(), vec![Color::Green]);
    assert_eq!(face.keywords, [Keyword::Reach, Keyword::Deathtouch]);
    assert!(face.activated_abilities.is_empty());
    assert!(face.static_abilities.is_empty());

    let [ability] = face.triggered_abilities.as_slice() else {
        panic!("Attercop must have exactly one triggered ability");
    };
    assert_eq!(
        ability.trigger,
        TriggerCondition::WheneverPermanentEntersBattlefield {
            controller: CastTriggerPlayer::Controller,
            filter: PermanentEventFilter {
                permanent_type: Some(PermanentTypeFilter::Land),
                ..PermanentEventFilter::default()
            },
            creature_filter: None,
        }
    );
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(
        ability.effect,
        [SpellEffectKind::PumpTarget {
            power: 1,
            toughness: 1,
            scale: None,
            subject: EffectSubject::Source,
        }]
    );
    assert!(ability.targeting.is_none());
    assert!(!ability.may);
}

#[test]
fn issue_335_strix_lookout_keeps_flying_vigilance_and_tap_loot() {
    let definition = CardRegistry::global()
        .get("strix_lookout")
        .expect("Strix Lookout");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{1}{U}");
    assert_eq!(face.types, ["Creature", "Bird"]);
    assert_eq!((face.power, face.toughness), (Some(1), Some(2)));
    assert_eq!(face.colors(), vec![Color::Blue]);
    assert_eq!(face.keywords, [Keyword::Flying, Keyword::Vigilance]);
    assert!(face.triggered_abilities.is_empty());
    assert!(face.static_abilities.is_empty());

    let [ability] = face.activated_abilities.as_slice() else {
        panic!("Strix Lookout must have exactly one activated ability");
    };
    assert_eq!(ability.ability_id.as_str(), "activated_01");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(
        ability.costs,
        [
            AbilityCost::Mana(ManaCost::parse("{1}{U}").expect("fixed mana cost")),
            AbilityCost::Tap,
        ]
    );
    assert_eq!(
        ability.effect,
        [SpellEffectKind::DrawDiscard {
            who: PlayerRecipient::Controller,
            draw_count: 1,
            discard_count: 1,
            order: DrawDiscardOrder::DrawThenDiscard,
            optional: false,
        }]
    );
    assert!(ability.targeting.is_none());
}

#[test]
fn issue_335_fingerprint_rows_match_the_presentation_registry() {
    let registry = CardRegistry::global();
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, card_name, face_id) in ISSUE_335_CARDS {
        let presentation = registry
            .presentation_face(id, face_id)
            .unwrap_or_else(|| panic!("missing presentation metadata for {id}/{face_id}"));
        assert_eq!(presentation.card_name, *card_name);
        assert_eq!(presentation.face_name, *card_name);
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
