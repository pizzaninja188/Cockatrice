use super::helpers::*;
use crate::helpers::ScenarioCommandExt;
use tricerules_cards::{
    AbilityCost, AbilityPresentation, AbilitySourceZone, Amount, CounterKind, Layout, ManaCost,
    SpellEffectKind,
};
use tricerules_core::{GameEngine, Zone};
use tricerules_proto::ruled::v1::{
    cost_selection::Selection, dev_command, ruled_command::Cmd, ChooseTriggerTarget,
    CostChoiceKind, CostObjectRef, CostSelection, CounterRemovalSelection, DevCommand, DevMoveCard,
    DevZone, TargetRef, TargetRefKind,
};

const MAGISTRATES_SCEPTER: &str = "magistrates_scepter";

fn main_phase(seed: u64) -> GameEngine {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        Some(vec![deck_with("island", &[]), deck_with("forest", &[])]),
        true,
    )
    .expect("new Scepter scenario");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn scepter(engine: &mut GameEngine) -> u32 {
    inject_permanent_on_battlefield(engine, 0, MAGISTRATES_SCEPTER)
}

fn set_charge(engine: &mut GameEngine, source: u32, count: u32) {
    engine
        .state
        .objects
        .get_mut(&source)
        .expect("Scepter object")
        .set_counter(CounterKind::Charge, count);
    engine.initial_response_batch();
}

fn charge_removal_selection(
    engine: &mut GameEngine,
    source: u32,
    player: i32,
    expected_available: u32,
) -> Option<CostSelection> {
    let offer = engine.initial_response_batch();
    let ability_key = (u64::from(source) << 32) | 1;
    let costs = &offer.legal_by_player[&player].cost_choices_by_ability[&ability_key];
    assert_eq!(
        costs.non_mana_costs_payable,
        expected_available >= 3,
        "the tap-plus-three-Charge cost is payable exactly when all three counters exist"
    );
    let choice = costs
        .choices
        .iter()
        .find(|choice| choice.kind() == CostChoiceKind::RemoveCounters)
        .expect("Scepter offers its Charge-removal cost");
    let removal = choice
        .counter_removal
        .as_ref()
        .expect("counter removal includes the exact source offer");
    assert_eq!(removal.count, 3);
    if expected_available < 3 {
        assert!(
            removal.options.is_empty(),
            "fewer than three counters exposes no payable removal option"
        );
        return None;
    }
    assert_eq!(removal.options.len(), 1);
    assert_eq!(removal.options[0].label, "charge");
    assert_eq!(removal.options[0].available_count, expected_available);
    let offered_source = removal.source.as_ref().expect("source counter payment");
    assert_eq!(offered_source.object_id, source);
    assert_eq!(
        offered_source.zone_change_generation,
        semantic::generation(engine, source)
    );
    Some(CostSelection {
        cost_index: choice.cost_index,
        selection: Some(Selection::CounterRemoval(CounterRemovalSelection {
            source: Some(CostObjectRef {
                object_id: offered_source.object_id,
                zone_change_generation: offered_source.zone_change_generation,
            }),
            option_id: removal.options[0].option_id,
        })),
    })
}

fn activate_extra_turn(
    engine: &GameEngine,
    source: u32,
    selection: CostSelection,
) -> tricerules_proto::ruled::v1::RuledCommand {
    let mut command = activate_ability_for(engine, source, 1, vec![]);
    let Some(Cmd::ActivateAbility(ability)) = command.cmd.as_mut() else {
        unreachable!("helper builds an activated-ability command");
    };
    ability.cost_selections.push(selection);
    command
}

fn move_scepter_to_graveyard(engine: &mut GameEngine, owner: i32) {
    engine.enable_dev_commands();
    let actor = engine.state.priority_player_id();
    engine
        .apply_scenario_command(
            actor,
            &tricerules_proto::ruled::v1::RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: owner,
                    dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                        card_name: "Magistrate's Scepter".into(),
                        zone: DevZone::Graveyard as i32,
                        ready: false,
                    })),
                })),
            },
        )
        .expect("move the activated source through the engine");
}

#[test]
fn magistrates_scepter_complete_registry_identity_exists() {
    let registry = tricerules_cards::registry::global();
    assert_eq!(
        registry.id_for_name("Magistrate's Scepter"),
        Some(MAGISTRATES_SCEPTER)
    );
    let card = registry
        .get(MAGISTRATES_SCEPTER)
        .expect("complete Magistrate's Scepter definition");
    assert_eq!(card.id, MAGISTRATES_SCEPTER);
    assert_eq!(card.name, "Magistrate's Scepter");
    assert_eq!(card.layout, Layout::Normal);
    assert_eq!(card.face_count(), 1);
    assert!(!card.commander_setup_only);

    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), MAGISTRATES_SCEPTER);
    assert_eq!(face.mana_cost, ManaCost::parse("{3}").unwrap());
    assert_eq!(face.types, ["Artifact"]);
    assert!(face.supertypes.is_empty());
    assert!(face.colors().is_empty());
    assert!(face.power.is_none() && face.toughness.is_none());
    let [charge, extra_turn] = face.activated_abilities.as_slice() else {
        panic!("Scepter has exactly its charge-counter and extra-turn abilities");
    };

    assert_eq!(charge.ability_id.as_str(), "activated_01");
    assert_eq!(
        charge.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(charge.source_zone, AbilitySourceZone::Battlefield);
    assert!(matches!(
        charge.costs.as_slice(),
        [AbilityCost::Mana(cost), AbilityCost::Tap] if cost == &ManaCost::parse("{4}").unwrap()
    ));
    assert_eq!(
        charge.effect,
        [SpellEffectKind::PutCounters {
            counter: CounterKind::Charge,
            count: Amount::Fixed(1),
            subject: tricerules_cards::primitives::EffectSubject::Source,
        }]
    );
    assert!(charge.targeting.is_none());

    assert_eq!(extra_turn.ability_id.as_str(), "activated_02");
    assert_eq!(
        extra_turn.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(extra_turn.source_zone, AbilitySourceZone::Battlefield);
    assert_eq!(
        extra_turn.costs,
        [
            AbilityCost::Tap,
            AbilityCost::RemoveCounters {
                counter: Some(CounterKind::Charge),
                count: 3,
                payment_source: Default::default(),
            },
        ]
    );
    assert_eq!(
        extra_turn.effect,
        [SpellEffectKind::TakeExtraTurns { count: 1 }]
    );
    assert!(extra_turn.targeting.is_none());
}

#[test]
fn magistrates_scepter_first_ability_pays_four_taps_and_places_charge_after_resolution() {
    let mut engine = main_phase(202_610_280);
    let source = scepter(&mut engine);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 3,
            ..Default::default()
        },
    );
    let revision_before_rejection = engine.state.command_index;

    engine
        .apply_scenario_command(0, &activate_ability_for(&engine, source, 0, vec![]))
        .expect_err("the first ability requires all four generic mana");
    assert_eq!(engine.state.command_index, revision_before_rejection);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 3);
    assert!(!engine.state.objects[&source].tapped);
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Charge),
        0
    );
    assert!(engine.state.stack.is_empty());

    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    engine
        .apply_scenario_command(0, &activate_ability_for(&engine, source, 0, vec![]))
        .expect("pay four generic mana and tap Scepter");
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    assert!(engine.state.objects[&source].tapped);
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Charge),
        0
    );
    assert_eq!(engine.state.stack.len(), 1);
    assert_eq!(engine.state.turn_instance, 1);

    pass_both_players(&mut engine);
    assert!(engine.state.stack.is_empty());
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Charge),
        1
    );
    assert_eq!(engine.state.turn_instance, 1);

    end_active_turn(&mut engine, 0);
    assert_eq!(
        engine.state.active_player_id(),
        1,
        "the charge ability grants no turn"
    );
    assert_eq!(engine.state.turn_instance, 2);
}

#[test]
fn magistrates_scepter_charge_placement_uses_the_shared_counter_replacement_pipeline() {
    let mut engine = main_phase(202_610_281);
    let source = scepter(&mut engine);
    inject_permanent_on_battlefield(&mut engine, 0, "doubling_season");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 4,
            ..Default::default()
        },
    );

    engine
        .apply_scenario_command(0, &activate_ability_for(&engine, source, 0, vec![]))
        .expect("activate Scepter with Doubling Season present");
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Charge),
        0
    );
    pass_both_players(&mut engine);

    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Charge),
        2,
        "the shared counter-placement replacement doubles the one counter placed by Scepter"
    );
}

#[test]
fn magistrates_scepter_removes_three_charges_and_resolves_after_source_leaves() {
    let mut engine = main_phase(202_610_282);
    let source = scepter(&mut engine);
    set_charge(&mut engine, source, 2);
    assert!(charge_removal_selection(&mut engine, source, 0, 2).is_none());
    let revision_before_rejection = engine.state.command_index;

    engine
        .apply_scenario_command(0, &activate_ability_for(&engine, source, 1, vec![]))
        .expect_err("two counters cannot pay the exact three-Charge cost");
    assert_eq!(engine.state.command_index, revision_before_rejection);
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Charge),
        2
    );
    assert!(!engine.state.objects[&source].tapped);
    assert!(engine.state.stack.is_empty());

    set_charge(&mut engine, source, 3);
    let exact = charge_removal_selection(&mut engine, source, 0, 3)
        .expect("all three Charge counters are payable");
    let original_generation = semantic::generation(&engine, source);
    engine
        .apply_scenario_command(0, &activate_extra_turn(&engine, source, exact))
        .expect("tap Scepter and remove exactly three of its Charge counters");
    assert!(engine.state.objects[&source].tapped);
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Charge),
        0
    );
    let activated = engine
        .state
        .stack
        .last()
        .expect("activated ability on stack");
    let ability_id = activated.id;
    assert_eq!(activated.controller, 0);
    assert_eq!(activated.source_permanent_id, Some(source));
    assert_eq!(activated.source_zone_change, original_generation);
    assert_eq!(engine.state.turn_instance, 1);

    move_scepter_to_graveyard(&mut engine, 0);
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
    assert_ne!(semantic::generation(&engine, source), original_generation);
    assert!(engine.state.stack.iter().any(|item| item.id == ability_id));
    pass_both_players(&mut engine);
    assert!(engine.state.stack.is_empty());
    assert_eq!(engine.state.active_player_id(), 0);
    assert_eq!(
        engine.state.turn_instance, 1,
        "the queued turn begins at cleanup"
    );

    let normal_cycle = engine.state.turn;
    end_active_turn(&mut engine, 0);
    assert_eq!(
        engine.state.active_player_id(),
        0,
        "Scepter's controller takes the extra turn"
    );
    assert_eq!(engine.state.turn_instance, 2);
    assert_eq!(
        engine.state.turn, normal_cycle,
        "the extra turn is not a normal seat cycle"
    );

    let library_before_extra_draw = engine.state.players[0].library.len();
    advance_to_main1_from_game_start(&mut engine);
    assert_eq!(engine.state.active_player_id(), 0);
    assert_eq!(
        engine.state.players[0].library.len(),
        library_before_extra_draw - 1
    );

    end_active_turn(&mut engine, 0);
    assert_eq!(
        engine.state.active_player_id(),
        1,
        "resume the saved normal successor"
    );
    assert_eq!(engine.state.turn_instance, 3);
    assert_eq!(engine.state.turn, normal_cycle);
}

#[test]
fn magistrates_scepter_countered_ability_does_not_schedule_an_extra_turn() {
    let mut engine = main_phase(202_610_283);
    let source = scepter(&mut engine);
    set_charge(&mut engine, source, 3);
    let exact = charge_removal_selection(&mut engine, source, 0, 3)
        .expect("all three Charge counters are payable");
    engine
        .apply_scenario_command(0, &activate_extra_turn(&engine, source, exact))
        .expect("activate Scepter's extra-turn ability");
    let ability_id = engine.state.stack.last().expect("ability on stack").id;
    assert_eq!(engine.state.turn_instance, 1);

    if engine.state.priority_player_id() != 1 {
        let priority = engine.state.priority_player_id();
        engine
            .apply_scenario_command(priority, &pass())
            .expect("pass priority to the opponent");
    }
    assert_eq!(engine.state.priority_player_id(), 1);
    inject_card_into_hand(&mut engine, 1, "tishanas_tidebinder");
    give_mana(
        &mut engine,
        1,
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    engine
        .apply_scenario_command(
            1,
            &cast_spell(
                hand_index_for_card(&engine, 1, "tishanas_tidebinder"),
                vec![],
            ),
        )
        .expect("cast Tishana's Tidebinder in response to the activated ability");
    pass_both_players(&mut engine);
    engine
        .apply_scenario_command(
            1,
            &tricerules_proto::ruled::v1::RuledCommand {
                cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
                    targets: vec![TargetRef {
                        object_id: ability_id,
                        kind: TargetRefKind::Stack as i32,
                        ..Default::default()
                    }],
                    ..Default::default()
                })),
            },
        )
        .expect("Tidebinder targets the exact Scepter ability");
    pass_both_players(&mut engine);

    assert!(!engine.state.stack.iter().any(|item| item.id == ability_id));
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Charge),
        0
    );
    assert!(engine.state.objects[&source].tapped);
    assert_eq!(engine.state.turn_instance, 1);
    end_active_turn(&mut engine, 0);
    assert_eq!(
        engine.state.active_player_id(),
        1,
        "a countered ability grants no turn"
    );
    assert_eq!(engine.state.turn_instance, 2);
}
