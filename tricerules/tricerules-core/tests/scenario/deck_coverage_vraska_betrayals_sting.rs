//! Actual registered Vraska, with explicitly prepared pools/zones/counters. Normal-session
//! and physical client acceptance are separate evidence.
use super::helpers::*;
use crate::helpers::ScenarioCommandExt;
use tricerules_cards::primitives::{CounterKind, Keyword};
use tricerules_core::Zone;
use tricerules_proto::ruled::v1 as rv1;

const VRASKA: &str = "vraska,_betrayals_sting";

fn game() -> GameEngine {
    let deck = deck_with("swamp", &[]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        26_110_300,
        &[17, 29, 41],
        20,
        Some(vec![deck; 3]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn prepared_vraska(engine: &mut GameEngine, loyalty: u32) -> u32 {
    let object = inject_permanent_on_battlefield(engine, 0, VRASKA);
    engine
        .state
        .objects
        .get_mut(&object)
        .unwrap()
        .set_counter(CounterKind::Loyalty, loyalty);
    object
}

fn choice(engine: &mut GameEngine, objects: Vec<u32>, players: Vec<i32>) -> RuledCommand {
    let prompt = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("actual Vraska choice");
    assert_eq!(prompt.deciding_player, 17);
    RuledCommand {
        cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
            rv1::SubmitResolutionChoice {
                chosen_object_ids: objects,
                chosen_player_ids: players,
                resolution_prompt_id: prompt.resolution_prompt_id.get(),
                ..Default::default()
            },
        )),
    }
}

fn move_card(engine: &mut GameEngine, name: &str, zone: rv1::DevZone) {
    engine.enable_dev_commands();
    engine
        .apply_scenario_command(
            17,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::DevCommand(rv1::DevCommand {
                    target_player_id: 17,
                    dev: Some(rv1::dev_command::Dev::MoveCard(rv1::DevMoveCard {
                        card_name: name.into(),
                        zone: zone as i32,
                        ready: false,
                    })),
                })),
            },
        )
        .unwrap();
}

fn resolve_top(engine: &mut GameEngine) -> rv1::RuledEventBatch {
    let count = engine
        .state
        .players
        .iter()
        .filter(|player| !player.has_lost)
        .count();
    let mut last = None;
    for _ in 0..count {
        let actor = engine.state.priority_player_id();
        last = Some(engine.apply_scenario_command(actor, &pass()).unwrap());
    }
    last.unwrap()
}

#[test]
fn vraska_actual_cast_records_phyrexian_payment_and_both_season_orders() {
    for (life, season, compleated_first, expected) in [
        (false, false, false, 6),
        (true, false, false, 4),
        (false, true, false, 12),
        (true, true, true, 8),
        (true, true, false, 10),
    ] {
        let mut engine = game();
        if season {
            inject_permanent_on_battlefield(&mut engine, 0, "doubling_season");
        }
        let vraska = inject_card_into_hand(&mut engine, 0, VRASKA);
        give_mana(
            &mut engine,
            17,
            ManaGift {
                c: 4,
                b: if life { 1 } else { 2 },
                ..Default::default()
            },
        );
        let command = cast_spell_flex(
            hand_index_for_card(&engine, 0, VRASKA),
            vec![],
            vec![rv1::FlexPipPayment {
                pip_index: 2,
                pay_life: life,
                mana_color: 0,
            }],
        );
        engine.apply_scenario_command(17, &command).unwrap();
        assert_eq!(engine.state.players[0].life, if life { 18 } else { 20 });
        let batch = resolve_top(&mut engine);
        if life && season {
            let pending = engine.state.pending_resolution.as_ref().unwrap();
            assert_eq!(
                pending.presentation.choice_kind,
                rv1::ChoiceKind::ReplacementEffect
            );
            let offered = batch
                .events
                .iter()
                .find_map(|event| match &event.ev {
                    Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(offered)) => Some(offered),
                    _ => None,
                })
                .unwrap();
            let option = offered
                .replacement_options
                .iter()
                .find(|option| option.effect_summary.contains("Compleated") == compleated_first)
                .unwrap()
                .application_id;
            let command = choice(&mut engine, vec![option], vec![]);
            let before = engine.diagnostic_snapshot().unwrap();
            assert!(engine.apply_scenario_command(29, &command).is_err());
            assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
            engine.apply_scenario_command(17, &command).unwrap();
        }
        assert!(engine.state.pending_resolution.is_none());
        assert_eq!(engine.state.objects[&vraska].zone, Zone::Battlefield);
        assert_eq!(
            engine.state.objects[&vraska].counter_count(CounterKind::Loyalty),
            expected
        );
        assert_eq!(engine.characteristics(vraska).unwrap().mana_value, 6);
    }
}

#[test]
fn vraska_zero_draws_and_loses_life_once_before_both_tekuthal_proliferations() {
    let mut engine = game();
    let vraska = prepared_vraska(&mut engine, 6);
    inject_creature_on_battlefield(&mut engine, 0, "tekuthal,_inquiry_dominus");
    engine.state.players[1]
        .counters
        .insert(CounterKind::Poison, 8);
    let before_hand = engine.state.players[0].hand.len();
    let command = activate_ability_for(&engine, vraska, 0, vec![]);
    engine.apply_scenario_command(17, &command).unwrap();
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.players[0].hand.len(), before_hand + 1);
    assert_eq!(engine.state.players[0].life, 19);
    for leaf in 0..2 {
        assert_eq!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .unwrap()
                .presentation
                .choice_kind,
            rv1::ChoiceKind::Proliferate
        );
        let command = choice(&mut engine, vec![vraska], vec![29]);
        let before = engine.diagnostic_snapshot().unwrap();
        assert!(engine.apply_scenario_command(41, &command).is_err());
        assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
        engine.apply_scenario_command(17, &command).unwrap();
        assert_eq!(engine.state.players[0].life, 19);
        assert_eq!(engine.state.players[0].hand.len(), before_hand + 1);
        assert_eq!(
            engine.state.objects[&vraska].counter_count(CounterKind::Loyalty),
            7 + leaf
        );
    }
    assert_eq!(engine.state.players[1].counters[&CounterKind::Poison], 10);
    assert!(engine.state.players[1].has_lost);
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn vraska_minus_two_keeps_the_exact_permanent_and_grants_all_five_treasure_mana_options() {
    for color in 0..5 {
        let mut engine = game();
        let vraska = prepared_vraska(&mut engine, 6);
        let owner_index = if color == 4 { 1 } else { 0 };
        let victim = inject_creature_under_foreign_control(
            &mut engine,
            owner_index,
            0,
            "tekuthal,_inquiry_dominus",
        );
        engine
            .state
            .objects
            .get_mut(&victim)
            .unwrap()
            .summoning_sick = true;
        engine
            .state
            .objects
            .get_mut(&victim)
            .unwrap()
            .set_counter(CounterKind::PlusOnePlusOne, 2);
        let generation = engine
            .state
            .zone_change_generation
            .get(&victim)
            .copied()
            .unwrap_or(0);
        let command = activate_ability_for(&engine, vraska, 1, target_object(victim));
        engine.apply_scenario_command(17, &command).unwrap();
        pass_priority_round(&mut engine);
        let result = engine.characteristics(victim).unwrap();
        assert_eq!(result.types, ["Artifact", "Treasure"]);
        assert!(result.is_legendary());
        assert!(!result.has_keyword(Keyword::Flying));
        assert_eq!(
            engine.state.objects[&victim].card_id,
            "tekuthal,_inquiry_dominus"
        );
        assert_eq!(
            engine.state.objects[&victim].counter_count(CounterKind::PlusOnePlusOne),
            2
        );
        assert_eq!(
            engine
                .state
                .zone_change_generation
                .get(&victim)
                .copied()
                .unwrap_or(0),
            generation
        );
        assert_eq!(
            engine.state.objects[&vraska].counter_count(CounterKind::Loyalty),
            4
        );
        move_card(
            &mut engine,
            "Vraska, Betrayal's Sting",
            rv1::DevZone::Graveyard,
        );
        assert_eq!(
            engine.characteristics(victim).unwrap().types,
            ["Artifact", "Treasure"]
        );
        let batch = engine.initial_response_batch();
        let public = batch
            .events
            .iter()
            .find_map(|event| match &event.ev {
                Some(rv1::ruled_event::Ev::ZoneView(view)) => view
                    .per_player
                    .iter()
                    .flat_map(|player| &player.battlefield_objects)
                    .find(|object| object.object_id == victim),
                _ => None,
            })
            .unwrap();
        assert!(public
            .rules_annotation_labels
            .contains(&"Types: Artifact, Treasure".into()));
        assert!(!public.is_creature);
        assert_eq!(public.activated_abilities.len(), 1);
        let ability = &public.activated_abilities[0];
        assert!(ability.is_mana_ability && ability.activatable);
        let mut command = activate_ability_for(&engine, victim, ability.ability_index, vec![]);
        let Some(rv1::ruled_command::Cmd::ActivateAbility(activation)) = command.cmd.as_mut()
        else {
            unreachable!()
        };
        activation.mana_option_index = color;
        engine.apply_scenario_command(17, &command).unwrap();
        assert_eq!(engine.state.objects[&victim].zone, Zone::Graveyard);
        assert!(engine.state.players[owner_index]
            .graveyard
            .contains(&victim));
        let pool = &engine.state.players[0].mana_pool;
        let actual = [pool.white, pool.blue, pool.black, pool.red, pool.green];
        let mut expected = [0; 5];
        expected[color as usize] = 1;
        assert_eq!(actual, expected);
        assert!(engine.state.stack.is_empty());
    }
}

#[test]
fn vraska_conversion_expires_on_reentry_and_rejects_a_stale_creature_target() {
    for leave_before_resolution in [false, true] {
        let mut engine = game();
        let vraska = prepared_vraska(&mut engine, 6);
        let victim = inject_creature_on_battlefield(&mut engine, 0, "llanowar_elves");
        let generation = engine
            .state
            .zone_change_generation
            .get(&victim)
            .copied()
            .unwrap_or(0);
        let command = activate_ability_for(&engine, vraska, 1, target_object(victim));
        engine.apply_scenario_command(17, &command).unwrap();
        if !leave_before_resolution {
            pass_priority_round(&mut engine);
        }
        move_card(&mut engine, "Llanowar Elves", rv1::DevZone::Graveyard);
        move_card(&mut engine, "Llanowar Elves", rv1::DevZone::Battlefield);
        assert_eq!(engine.state.objects[&victim].zone, Zone::Battlefield);
        assert!(engine.state.zone_change_generation[&victim] > generation);
        if leave_before_resolution {
            pass_priority_round(&mut engine);
        }
        let current = engine.characteristics(victim).unwrap();
        assert!(current.is_creature() && current.has_type("Elf"));
        assert!(!current.has_type("Treasure"));
    }
}

#[test]
fn vraska_actual_ultimate_targets_any_player_after_source_sacrifices_all_loyalty() {
    for target in [17, 29, 41] {
        for before in [0, 8, 9] {
            let mut engine = game();
            let vraska = prepared_vraska(&mut engine, 9);
            let index = engine.state.player_idx(target).unwrap();
            engine.state.players[index]
                .counters
                .insert(CounterKind::Poison, before);
            let command = activate_ability_for(
                &engine,
                vraska,
                2,
                vec![rv1::TargetRef {
                    kind: rv1::TargetRefKind::Player as i32,
                    object_id: target as u32,
                    ..Default::default()
                }],
            );
            engine.apply_scenario_command(17, &command).unwrap();
            assert_eq!(engine.state.objects[&vraska].zone, Zone::Graveyard);
            pass_priority_round(&mut engine);
            assert_eq!(
                engine.state.players[index].counters[&CounterKind::Poison],
                9
            );
            assert!(engine.state.pending_resolution.is_none());
        }
    }
}

#[test]
fn vraska_rejects_insufficient_loyalty_wrong_controller_and_noncreature_targets_atomically() {
    let mut engine = game();
    let vraska = prepared_vraska(&mut engine, 6);
    let land = inject_permanent_on_battlefield(&mut engine, 0, "swamp");
    for (actor, index, targets) in [
        (29, 0, vec![]),
        (17, 1, target_object(land)),
        (17, 2, target_player(41)),
    ] {
        let command = activate_ability_for(&engine, vraska, index, targets);
        let before = engine.diagnostic_snapshot().unwrap();
        assert!(engine.apply_scenario_command(actor, &command).is_err());
        assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
    }
}
