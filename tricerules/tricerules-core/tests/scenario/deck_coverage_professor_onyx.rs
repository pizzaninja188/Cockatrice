//! Actual Onyx with prepared zones, mana and loyalty; normal capture and GUI are separate.
use super::helpers::*;
use crate::helpers::ScenarioCommandExt;
use tricerules_cards::primitives::CounterKind;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1 as rv1;

fn game() -> GameEngine {
    let deck = deck_with("swamp", &[]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        26_110_301,
        &[17, 29, 41],
        20,
        Some(vec![deck; 3]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    engine
}
fn onyx(engine: &mut GameEngine, loyalty: u32) -> u32 {
    let object = inject_permanent_on_battlefield(engine, 0, "professor_onyx");
    engine
        .state
        .objects
        .get_mut(&object)
        .unwrap()
        .set_counter(CounterKind::Loyalty, loyalty);
    object
}
fn answer(engine: &mut GameEngine, actor: i32, chosen: Vec<u32>) {
    let pending = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(pending.deciding_player, actor);
    let command = RuledCommand {
        cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
            rv1::SubmitResolutionChoice {
                resolution_prompt_id: pending.resolution_prompt_id.get(),
                chosen_object_ids: chosen,
                ..Default::default()
            },
        )),
    };
    engine.apply_scenario_command(actor, &command).unwrap();
}
fn resolve_top(engine: &mut GameEngine) {
    let live = engine.state.players.iter().filter(|p| !p.has_lost).count();
    for _ in 0..live {
        let actor = engine.state.priority_player_id();
        engine.apply_scenario_command(actor, &pass()).unwrap();
    }
}

#[test]
fn professor_onyx_actual_cast_enters_with_five_and_plus_one_partitions_exact_top_three() {
    let mut engine = game();
    let source = inject_card_into_hand(&mut engine, 0, "professor_onyx");
    give_mana(
        &mut engine,
        17,
        ManaGift {
            c: 4,
            b: 2,
            ..Default::default()
        },
    );
    let command = cast_spell(hand_index_for_card(&engine, 0, "professor_onyx"), vec![]);
    engine.apply_scenario_command(17, &command).unwrap();
    resolve_top(&mut engine);
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Loyalty),
        5
    );
    assert_eq!(engine.characteristics(source).unwrap().mana_value, 6);
    let looked = engine.state.players[0]
        .library
        .iter()
        .take(3)
        .copied()
        .collect::<Vec<_>>();
    let command = activate_ability_for(&engine, source, 0, vec![]);
    engine.apply_scenario_command(17, &command).unwrap();
    resolve_top(&mut engine);
    assert_eq!(engine.state.players[0].life, 19);
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Loyalty),
        6
    );
    let pending = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(pending.presentation.candidates, looked);
    let prompt = pending.resolution_prompt_id.get();
    let rejected = RuledCommand {
        cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
            rv1::SubmitResolutionChoice {
                resolution_prompt_id: prompt,
                chosen_object_ids: vec![looked[0]],
                ..Default::default()
            },
        )),
    };
    let before = engine.diagnostic_snapshot().unwrap();
    assert!(engine.apply_scenario_command(29, &rejected).is_err());
    assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
    answer(&mut engine, 17, vec![looked[1]]);
    assert!(looked
        .iter()
        .all(|id| engine.state.objects[id].zone == Zone::Library));
    answer(&mut engine, 17, vec![looked[2], looked[0]]);
    assert_eq!(engine.state.objects[&looked[1]].zone, Zone::Hand);
    let graveyard = &engine.state.players[0].graveyard;
    assert_eq!(&graveyard[graveyard.len() - 2..], &[looked[2], looked[0]]);
    assert_eq!(engine.state.players[0].life, 19);
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn professor_onyx_actual_minus_three_stages_all_opponents_and_accepts_only_greatest_ties() {
    let mut engine = game();
    let source = onyx(&mut engine, 5);
    let first = inject_creature_on_battlefield(&mut engine, 1, "llanowar_elves");
    let second = inject_creature_on_battlefield(&mut engine, 1, "llanowar_elves");
    let smaller = inject_creature_on_battlefield(&mut engine, 1, "darksteel_myr");
    let other = inject_creature_on_battlefield(&mut engine, 2, "elvish_visionary");
    let other_tie = inject_creature_on_battlefield(&mut engine, 2, "elvish_visionary");
    // The broad creature fixture defaults to 2/2. This actual-card scenario uses printed P/T.
    for object in [first, second, smaller, other, other_tie] {
        let object = engine.state.objects.get_mut(&object).unwrap();
        object.power = None;
        object.toughness = None;
    }
    let command = activate_ability_for(&engine, source, 1, vec![]);
    engine.apply_scenario_command(17, &command).unwrap();
    resolve_top(&mut engine);
    let pending = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(pending.deciding_player, 29);
    assert!(
        pending.presentation.candidates.contains(&first)
            && pending.presentation.candidates.contains(&second)
    );
    assert!(!pending.presentation.candidates.contains(&smaller));
    answer(&mut engine, 29, vec![second]);
    assert_eq!(engine.state.objects[&second].zone, Zone::Battlefield);
    answer(&mut engine, 41, vec![other]);
    assert_eq!(engine.state.objects[&second].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&other].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&first].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Loyalty),
        2
    );
}

#[test]
fn professor_onyx_actual_ultimate_runs_seven_rounds_after_paying_last_loyalty() {
    let mut engine = game();
    let source = onyx(&mut engine, 8);
    let paid = engine.state.players[2].hand[0];
    let command = activate_ability_for(&engine, source, 2, vec![]);
    engine.apply_scenario_command(17, &command).unwrap();
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
    resolve_top(&mut engine);
    for round in 0..7 {
        answer(&mut engine, 29, vec![]);
        answer(
            &mut engine,
            41,
            if round == 0 { vec![paid] } else { vec![] },
        );
        if round < 6 {
            assert!(!engine.state.players[1].has_lost);
        }
    }
    assert_eq!(
        engine
            .state
            .players
            .iter()
            .map(|p| p.life)
            .collect::<Vec<_>>(),
        [20, -1, 2]
    );
    assert!(engine.state.players[1].has_lost);
    assert_eq!(engine.state.objects[&paid].zone, Zone::Graveyard);
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn professor_onyx_actual_magecraft_cast_and_copy_each_have_fixed_controller_gain() {
    let mut engine = game();
    onyx(&mut engine, 5);
    inject_card_into_hand(&mut engine, 0, "opt");
    inject_card_into_hand(&mut engine, 0, "twincast");
    give_mana(
        &mut engine,
        17,
        ManaGift {
            u: 3,
            ..Default::default()
        },
    );
    let command = cast_spell(hand_index_for_card(&engine, 0, "opt"), vec![]);
    engine.apply_scenario_command(17, &command).unwrap();
    let opt = engine.state.stack[0].id;
    resolve_top(&mut engine);
    assert_eq!(
        engine
            .state
            .players
            .iter()
            .map(|p| p.life)
            .collect::<Vec<_>>(),
        [22, 18, 18]
    );
    let command = cast_spell(
        hand_index_for_card(&engine, 0, "twincast"),
        target_object(opt),
    );
    engine.apply_scenario_command(17, &command).unwrap();
    resolve_top(&mut engine);
    assert_eq!(
        engine
            .state
            .players
            .iter()
            .map(|p| p.life)
            .collect::<Vec<_>>(),
        [24, 16, 16]
    );
    resolve_top(&mut engine);
    if engine.state.pending_resolution.is_some() {
        answer(&mut engine, 17, vec![]);
    }
    resolve_top(&mut engine);
    assert_eq!(
        engine
            .state
            .players
            .iter()
            .map(|p| p.life)
            .collect::<Vec<_>>(),
        [26, 14, 14]
    );
    assert_eq!(
        engine
            .state
            .stack
            .iter()
            .filter(|item| item.is_copy)
            .count(),
        1
    );
}

#[test]
fn professor_onyx_rejects_wrong_controller_and_insufficient_loyalty_atomically() {
    let mut engine = game();
    let source = onyx(&mut engine, 5);
    for (actor, index) in [(29, 0), (17, 2)] {
        let command = activate_ability_for(&engine, source, index, vec![]);
        let before = engine.diagnostic_snapshot().unwrap();
        assert!(engine.apply_scenario_command(actor, &command).is_err());
        assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
    }
}
