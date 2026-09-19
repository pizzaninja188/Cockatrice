//! Issue #374 registry conformance for the five retained graveyard-condition trigger identities.
//!
//! Every retained card is keyed on public graveyard state (CR 404.2) through the shipped
//! `GraveyardAggregate` condition, rechecked at trigger creation and resolution (CR 603.4). The
//! Elf/Lesson predicates use `required_subtypes`, the Descend-4 permanent predicate uses the
//! shipped instant/sorcery exclusion, and the Delirium gate counts distinct card types. Creakwood
//! Safewright's unconditional entry replacement is the generic three -1/-1 counter template. The
//! six blocked cohort identities stay unregistered until their narrower non-graveyard primitives
//! land.

use tricerules_cards::primitives::{
    CardTypeFilter, EffectSubject, EntersWithCountersAffected, LifeAmount, PlayerRecipient,
    ResolutionBranchDef, ResolutionBranchRequirement, ResolutionBranchSelection, ResolutionCost,
    StaticAbilityDef, ZoneCardFilter,
};
use tricerules_cards::{
    AbilityPresentation, Amount, CardRegistry, CastTriggerPlayer, ChoiceId, CounterKind,
    GameCondition, GraveyardAggregate, Keyword, Layout, RelativePlayerSet, SpellEffectKind,
    TriggerCondition,
};

fn graveyard_gate(
    aggregate: GraveyardAggregate,
    filter: Option<ZoneCardFilter>,
    min: u32,
) -> GameCondition {
    GameCondition::GraveyardAggregate {
        owners: RelativePlayerSet::Controller,
        aggregate,
        filter,
        min: Some(min),
        max: None,
    }
}

fn elf_card_filter() -> ZoneCardFilter {
    ZoneCardFilter {
        required_subtypes: vec!["Elf".into()],
        ..ZoneCardFilter::default()
    }
}

fn permanent_card_filter() -> ZoneCardFilter {
    ZoneCardFilter {
        excluded_card_types: vec![CardTypeFilter::Instant, CardTypeFilter::Sorcery],
        ..ZoneCardFilter::default()
    }
}

#[test]
fn issue_374_registers_the_five_retained_identities() {
    let registry = CardRegistry::global();
    for (id, name, face_id, mana_cost, types, power, toughness, keywords) in [
        (
            "creakwood_safewright",
            "Creakwood Safewright",
            "creakwood_safewright",
            "{1}{B}",
            vec!["Creature", "Elf", "Warrior"],
            5,
            5,
            vec![],
        ),
        (
            "dawnhand_eulogist",
            "Dawnhand Eulogist",
            "dawnhand_eulogist",
            "{3}{B}",
            vec!["Creature", "Elf", "Warlock"],
            3,
            3,
            vec![Keyword::Menace],
        ),
        (
            "hand_that_feeds",
            "Hand That Feeds",
            "hand_that_feeds",
            "{1}{R}",
            vec!["Creature", "Mutant"],
            2,
            2,
            vec![],
        ),
        (
            "stinging_cave_crawler",
            "Stinging Cave Crawler",
            "stinging_cave_crawler",
            "{2}{B}",
            vec!["Creature", "Insect", "Horror"],
            1,
            3,
            vec![Keyword::Deathtouch],
        ),
        (
            "walltop_sentries",
            "Walltop Sentries",
            "walltop_sentries",
            "{2}{G}",
            vec!["Creature", "Human", "Soldier", "Ally"],
            2,
            3,
            vec![Keyword::Reach, Keyword::Deathtouch],
        ),
    ] {
        let definition = registry
            .get(id)
            .unwrap_or_else(|| panic!("missing retained card {id}"));
        assert_eq!(definition.name, name);
        assert_eq!(registry.id_for_name(name), Some(id));
        assert_eq!(definition.layout, Layout::Normal);
        assert_eq!(definition.face_count(), 1);
        let face = definition.primary_face();
        assert_eq!(face.face_id.as_str(), face_id);
        assert_eq!(face.mana_cost.to_string(), mana_cost);
        assert_eq!(face.types, types);
        assert_eq!(face.keywords, keywords);
        assert_eq!((face.power, face.toughness), (Some(power), Some(toughness)));
    }
}

#[test]
fn issue_374_trigger_payloads_are_exact() {
    let registry = CardRegistry::global();

    // Creakwood Safewright: the entry replacement starts it at three -1/-1 counters, and the
    // end-step trigger's conjunctive intervening-if rechecks the graveyard Elf and the self
    // counter count.
    let creakwood = registry
        .get("creakwood_safewright")
        .expect("Creakwood Safewright");
    let face = creakwood.primary_face();
    let [entry] = face.static_abilities.as_slice() else {
        panic!("Creakwood must author one entry replacement");
    };
    assert_eq!(
        entry.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(
        entry.definition,
        StaticAbilityDef::EntersWithCounters {
            affected: EntersWithCountersAffected::Self_,
            counter: CounterKind::MinusOneMinusOne,
            amount: Amount::Fixed(3),
            cast_cost_condition: None,
        }
    );
    let [end_step] = face.triggered_abilities.as_slice() else {
        panic!("Creakwood must author one end-step trigger");
    };
    assert_eq!(
        end_step.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(
        end_step.trigger,
        TriggerCondition::AtBeginningOfEndStep {
            player: CastTriggerPlayer::Controller,
        }
    );
    assert_eq!(
        end_step.intervening_if,
        Some(GameCondition::AllOf(vec![
            graveyard_gate(GraveyardAggregate::CardCount, Some(elf_card_filter()), 1),
            GameCondition::SourceCounterCount {
                counter: CounterKind::MinusOneMinusOne,
                min: Some(1),
                max: None,
            },
        ]))
    );
    assert_eq!(
        end_step.effect,
        [SpellEffectKind::RemoveCounters {
            counter: CounterKind::MinusOneMinusOne,
            count: 1,
            subject: EffectSubject::Source,
        }]
    );

    // Dawnhand Eulogist: the entry trigger mills exactly three, then the mandatory
    // `FirstApplicable` branch drains each opponent and gains life only with an Elf card in the
    // graveyard. The fallback branch is empty and unconditional.
    let dawnhand = registry
        .get("dawnhand_eulogist")
        .expect("Dawnhand Eulogist");
    let [etb] = dawnhand.primary_face().triggered_abilities.as_slice() else {
        panic!("Dawnhand must author one entry trigger");
    };
    assert_eq!(etb.presentation, AbilityPresentation::OracleLines(vec![2]));
    assert_eq!(etb.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert_eq!(etb.intervening_if, None);
    assert_eq!(
        etb.effect,
        [
            SpellEffectKind::Mill {
                count: Amount::Fixed(3),
                who: PlayerRecipient::Controller,
            },
            SpellEffectKind::ChooseResolutionBranch {
                chooser: PlayerRecipient::Controller,
                optional: false,
                selection: ResolutionBranchSelection::FirstApplicable,
                branches: vec![
                    ResolutionBranchDef {
                        branch_id: ChoiceId::new("elf_in_graveyard").unwrap(),
                        presentation: AbilityPresentation::Fallback,
                        runtime_fallback: None,
                        cost: ResolutionCost::None,
                        requirement: ResolutionBranchRequirement::GameCondition(graveyard_gate(
                            GraveyardAggregate::CardCount,
                            Some(elf_card_filter()),
                            1,
                        )),
                        effects: vec![
                            SpellEffectKind::LoseLife {
                                amount: LifeAmount::Fixed(2),
                                who: PlayerRecipient::EachOpponent,
                            },
                            SpellEffectKind::GainLife {
                                amount: Amount::Fixed(2),
                            },
                        ],
                    },
                    ResolutionBranchDef {
                        branch_id: ChoiceId::new("otherwise").unwrap(),
                        presentation: AbilityPresentation::Fallback,
                        runtime_fallback: None,
                        cost: ResolutionCost::None,
                        requirement: ResolutionBranchRequirement::Always,
                        effects: Vec::new(),
                    },
                ],
                otherwise: Vec::new(),
            },
        ]
    );

    // Hand That Feeds: the Delirium attack trigger pumps +2/+0 and grants menace in printed order.
    let hand = registry.get("hand_that_feeds").expect("Hand That Feeds");
    let [attack] = hand.primary_face().triggered_abilities.as_slice() else {
        panic!("Hand That Feeds must author one attack trigger");
    };
    assert_eq!(
        attack.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(
        attack.trigger,
        TriggerCondition::WheneverSelfAttacks {
            minimum_other_attackers: 0,
        }
    );
    assert_eq!(
        attack.intervening_if,
        Some(graveyard_gate(
            GraveyardAggregate::DistinctCardTypes,
            None,
            4,
        ))
    );
    assert_eq!(
        attack.effect,
        [
            SpellEffectKind::PumpTarget {
                power: 2,
                toughness: 0,
                scale: None,
                subject: EffectSubject::Source,
            },
            SpellEffectKind::GrantKeywords {
                subject: EffectSubject::Source,
                keywords: vec![Keyword::Menace],
            },
        ]
    );

    // Stinging Cave Crawler: the Descend-4 attack trigger draws then loses one life.
    let stinging = registry
        .get("stinging_cave_crawler")
        .expect("Stinging Cave Crawler");
    let [attack] = stinging.primary_face().triggered_abilities.as_slice() else {
        panic!("Stinging Cave Crawler must author one attack trigger");
    };
    assert_eq!(
        attack.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(
        attack.intervening_if,
        Some(graveyard_gate(
            GraveyardAggregate::CardCount,
            Some(permanent_card_filter()),
            4,
        ))
    );
    assert_eq!(
        attack.effect,
        [
            SpellEffectKind::Draw {
                who: PlayerRecipient::Controller,
                count: Amount::Fixed(1),
            },
            SpellEffectKind::LoseLife {
                amount: LifeAmount::Fixed(1),
                who: PlayerRecipient::Controller,
            },
        ]
    );

    // Walltop Sentries: the dies trigger gains two with a Lesson card in the graveyard.
    let walltop = registry.get("walltop_sentries").expect("Walltop Sentries");
    let [dies] = walltop.primary_face().triggered_abilities.as_slice() else {
        panic!("Walltop Sentries must author one dies trigger");
    };
    assert_eq!(dies.presentation, AbilityPresentation::OracleLines(vec![2]));
    assert_eq!(dies.trigger, TriggerCondition::WhenSelfDies);
    assert_eq!(
        dies.intervening_if,
        Some(graveyard_gate(
            GraveyardAggregate::CardCount,
            Some(ZoneCardFilter {
                required_subtypes: vec!["Lesson".into()],
                ..ZoneCardFilter::default()
            }),
            1,
        ))
    );
    assert_eq!(
        dies.effect,
        [SpellEffectKind::GainLife {
            amount: Amount::Fixed(2),
        }]
    );
}

#[test]
fn issue_374_blocked_identities_stay_unregistered() {
    let registry = CardRegistry::global();
    for name in [
        "Fear of Burning Alive",
        "Fear of Missing Out",
        "Osseous Sticktwister",
        "Starving Revenant",
        "Trystan, Callous Cultivator // Trystan, Penitent Culler",
        "Winter, Misanthropic Guide",
    ] {
        assert!(
            registry.id_for_name(name).is_none(),
            "{name} must stay fail-closed until its narrower blocker lands"
        );
    }
}
