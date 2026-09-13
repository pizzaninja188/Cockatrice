use tricerules_cards::primitives::{
    EffectSubject, HandCardAction, LifeAmount, PlayerRecipient, SpellEffectKind, TargetController,
    TargetKind, TargetObjectExclusion,
};
use tricerules_cards::{
    AbilityPresentation, Amount, CardRegistry, CastTriggerPlayer, CounterKind,
    LibraryPartitionKind, TriggerCondition,
};

#[test]
fn issue_266_registers_the_reviewed_twenty_one_card_cohort() {
    let registry = CardRegistry::global();
    for id in [
        "ajanis_pridemate",
        "pest_mascot",
        "marauding_blight-priest",
        "lys_alana_informant",
        "thawbringer",
        "wary_thespian",
        "wary_watchdog",
        "cogwork_wrestler",
        "humbling_elder",
        "corrupt_court_official",
        "forecasting_fortune_teller",
        "novice_inspector",
        "feather_of_flight",
        "lofty_dreams",
        "merrow_skyswimmer",
        "resolute_reinforcements",
        "s.h.i.e.l.d._deployment_drone",
        "mongoose_lizard",
        "skeleton_archer",
        "sterling_supplier",
        "rimekin_recluse",
    ] {
        registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
    }
}

#[test]
fn issue_266_emits_exact_life_and_combined_event_triggers() {
    let registry = CardRegistry::global();
    for id in ["ajanis_pridemate", "pest_mascot"] {
        let ability = first_ability(registry, id);
        assert_eq!(
            ability.trigger,
            TriggerCondition::WheneverPlayerGainsLife {
                player: CastTriggerPlayer::Controller,
            }
        );
        assert_eq!(
            ability.effect,
            [SpellEffectKind::PutCounters {
                counter: CounterKind::PlusOnePlusOne,
                count: Amount::Fixed(1),
                subject: EffectSubject::Source,
            }]
        );
    }

    let drain = first_ability(registry, "marauding_blight-priest");
    assert_eq!(
        drain.effect,
        [SpellEffectKind::LoseLife {
            amount: LifeAmount::Fixed(1),
            who: PlayerRecipient::EachOpponent,
        }]
    );

    for id in [
        "lys_alana_informant",
        "thawbringer",
        "wary_thespian",
        "wary_watchdog",
    ] {
        let abilities = &registry.get(id).unwrap().primary_face().triggered_abilities;
        assert_eq!(abilities.len(), 2, "{id}");
        assert_eq!(abilities[0].ability_id.as_str(), "triggered_01");
        assert_eq!(abilities[1].ability_id.as_str(), "triggered_02");
        assert_eq!(
            abilities[0].trigger,
            TriggerCondition::WhenSelfEntersBattlefield
        );
        assert_eq!(abilities[1].trigger, TriggerCondition::WhenSelfDies);
        for ability in abilities {
            assert_eq!(
                ability.effect,
                [SpellEffectKind::LibraryPartition {
                    count: 1,
                    top_min: 0,
                    top_max: None,
                    kind: LibraryPartitionKind::Surveil,
                }]
            );
        }
    }
}

#[test]
fn issue_266_emits_exact_target_cardinality_and_filters() {
    let registry = CardRegistry::global();

    for id in ["cogwork_wrestler", "humbling_elder"] {
        let ability = first_ability(registry, id);
        let SpellEffectKind::PumpTarget { subject, .. } = &ability.effect[0] else {
            panic!("{id} must emit PumpTarget");
        };
        let EffectSubject::Chosen(filter) = subject else {
            panic!("{id} must choose a target");
        };
        assert_eq!(filter.controller, TargetController::Opponent);
        assert_group(ability, 1, 1);
    }

    let discard = first_ability(registry, "corrupt_court_official");
    let SpellEffectKind::ChooseHandCards { action, target, .. } = &discard.effect[0] else {
        panic!("Corrupt Court Official must emit ChooseHandCards");
    };
    assert_eq!(*action, HandCardAction::Discard);
    assert_eq!(target.kind, TargetKind::OpponentPlayer);
    assert_group(discard, 1, 1);

    for id in ["mongoose_lizard", "skeleton_archer"] {
        let ability = first_ability(registry, id);
        let SpellEffectKind::DamageTarget { target, .. } = &ability.effect[0] else {
            panic!("{id} must emit DamageTarget");
        };
        assert_eq!(target.kind, TargetKind::AnyTarget);
        assert_group(ability, 1, 1);
    }

    let counter = first_ability(registry, "sterling_supplier");
    let SpellEffectKind::PutCounters { subject, .. } = &counter.effect[0] else {
        panic!("Sterling Supplier must emit PutCounters");
    };
    let EffectSubject::Chosen(filter) = subject else {
        panic!("Sterling Supplier must choose a target");
    };
    assert_eq!(filter.controller, TargetController::You);
    assert_eq!(filter.excluded_objects, [TargetObjectExclusion::Source]);
    assert_group(counter, 1, 1);

    let bounce = first_ability(registry, "rimekin_recluse");
    assert_group(bounce, 0, 1);
    let SpellEffectKind::ReturnToOwnersHand { subject } = &bounce.effect[0] else {
        panic!("Rimekin Recluse must emit ReturnToOwnersHand");
    };
    let EffectSubject::Chosen(filter) = subject else {
        panic!("Rimekin Recluse must choose a target");
    };
    assert_eq!(filter.kind, TargetKind::Creature);
    assert_eq!(filter.excluded_objects, [TargetObjectExclusion::Source]);
}

#[test]
fn issue_266_emits_exact_draw_and_token_triggers() {
    let registry = CardRegistry::global();
    for id in ["feather_of_flight", "lofty_dreams"] {
        assert_eq!(
            first_ability(registry, id).effect,
            [SpellEffectKind::Draw {
                who: PlayerRecipient::Controller,
                count: Amount::Fixed(1),
            }]
        );
    }

    for (id, token) in [
        ("forecasting_fortune_teller", "clue"),
        ("novice_inspector", "clue"),
        ("merrow_skyswimmer", "merfolk_wu_1_1"),
        ("resolute_reinforcements", "soldier_w_1_1"),
        ("s.h.i.e.l.d._deployment_drone", "soldier_w_1_1"),
    ] {
        let SpellEffectKind::CreateTokens {
            token: actual,
            count,
            who,
            tapped,
            ..
        } = &first_ability(registry, id).effect[0]
        else {
            panic!("{id} must emit CreateTokens");
        };
        assert_eq!(actual, token);
        assert_eq!(*count, Amount::Fixed(1));
        assert_eq!(*who, PlayerRecipient::Controller);
        assert!(!*tapped);
    }
}

fn first_ability(
    registry: &'static CardRegistry,
    id: &str,
) -> &'static tricerules_cards::TriggeredAbilityDef {
    let ability = &registry.get(id).unwrap().primary_face().triggered_abilities[0];
    assert_eq!(ability.ability_id.as_str(), "triggered_01");
    assert!(matches!(
        ability.presentation,
        AbilityPresentation::OracleLines(_)
    ));
    ability
}

fn assert_group(ability: &tricerules_cards::TriggeredAbilityDef, min: u32, max: u32) {
    let groups = &ability.targeting.as_ref().expect("targeting").groups;
    assert_eq!(groups.len(), 1);
    assert_eq!((groups[0].min, groups[0].max), (min, max));
    assert_eq!(groups[0].effect_indices, [0]);
}
