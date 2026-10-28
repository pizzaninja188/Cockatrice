//! Actual Vorinclex casts and the first doubled/halved loyalty-cost interactions.
use super::helpers::*;
use crate::helpers::ScenarioCommandExt;
use tricerules_cards::{primitives::CounterKind, Keyword};
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, ResolutionChoiceDecision, RuledCommand, SubmitResolutionChoice,
};

const VORINCLEX: &str = "vorinclex,_monstrous_raider";
const JACE: &str = "jace,_unraveler_of_secrets";
const LAEZEL: &str = "laezel,_vlaakiths_champion";
const P10: i32 = 10;
const P20: i32 = 20;

fn game(seed: u64) -> GameEngine {
    let registry = tricerules_cards::registry::global();
    for card in [VORINCLEX, JACE, LAEZEL] {
        registry
            .get(card)
            .unwrap_or_else(|| panic!("{card} must be registered for this scenario"));
    }
    let deck = deck_with("island", &[]);
    let mut engine = GameEngine::new(
        registry,
        seed,
        &[P10, P20],
        20,
        Some(vec![deck.clone(), deck]),
        true,
    )
    .expect("two-player Vorinclex scenario game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn cast_registered_card(
    engine: &mut GameEngine,
    seat: usize,
    card_id: &str,
    mana: ManaGift,
) -> u32 {
    let player = engine.state.players[seat].id;
    let source = inject_card_into_hand(engine, seat, card_id);
    give_mana(engine, player, mana);
    let slot = hand_index_for_card(engine, seat, card_id);
    engine
        .apply_scenario_command(player, &cast_spell(slot, vec![]))
        .unwrap_or_else(|_| panic!("cast actual registered {card_id}"));
    assert_eq!(engine.state.players[seat].mana_pool, Default::default());
    pass_both_players(engine);
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    source
}

fn cast_vorinclex(engine: &mut GameEngine, seat: usize) -> u32 {
    cast_registered_card(
        engine,
        seat,
        VORINCLEX,
        ManaGift {
            g: 2,
            c: 4,
            ..Default::default()
        },
    )
}

fn cast_jace(engine: &mut GameEngine, seat: usize) -> u32 {
    cast_registered_card(
        engine,
        seat,
        JACE,
        ManaGift {
            u: 2,
            c: 3,
            ..Default::default()
        },
    )
}

fn proliferate_choice(objects: Vec<u32>, players: Vec<i32>) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            resolution_prompt_id: 0,
            chosen_object_ids: objects,
            chosen_player_ids: players,
            decision: ResolutionChoiceDecision::Unspecified as i32,
            ..Default::default()
        })),
    }
}

#[test]
fn vorinclex_actual_cast_enters_as_a_hasty_trampling_six_six() {
    let mut engine = game(26_102_801);
    let vorinclex = cast_vorinclex(&mut engine, 0);

    assert_eq!(engine.state.objects[&vorinclex].owner, P10);
    assert_eq!(engine.state.objects[&vorinclex].controller, P10);
    assert_eq!(engine.state.objects[&vorinclex].zone, Zone::Battlefield);
    let characteristics = engine.characteristics(vorinclex).unwrap();
    assert_eq!(characteristics.power, Some(6));
    assert_eq!(characteristics.toughness, Some(6));
    assert!(characteristics.has_keyword(Keyword::Trample));
    assert!(characteristics.has_keyword(Keyword::Haste));
}

#[test]
fn controlled_vorinclex_doubles_a_friendly_jaces_actual_plus_one_cost() {
    let mut engine = game(26_102_802);
    let jace = cast_jace(&mut engine, 0);
    assert_eq!(
        engine.state.objects[&jace].counter_count(CounterKind::Loyalty),
        5
    );
    let vorinclex = cast_vorinclex(&mut engine, 0);
    assert_eq!(engine.state.objects[&vorinclex].zone, Zone::Battlefield);
    assert_eq!(engine.state.priority_player_id(), P10);
    assert_eq!(engine.state.stack.len(), 0);

    let activation = activate_ability_for(&engine, jace, 0, vec![]);
    let batch = engine
        .apply_scenario_command(P10, &activation)
        .expect("activate the real Jace +1 while P10 controls Vorinclex");
    assert!(!batch.events.is_empty());
    assert_eq!(
        engine.state.objects[&jace].counter_count(CounterKind::Loyalty),
        7
    );
    assert_eq!(engine.state.stack.len(), 1);
    assert_eq!(engine.state.stack[0].source_permanent_id, Some(jace));
    assert_eq!(engine.state.priority_player_id(), P10);
}

#[test]
fn friendly_vorinclex_doubles_jaces_actual_entry_loyalty() {
    let mut engine = game(26_102_805);
    cast_vorinclex(&mut engine, 0);
    let jace = cast_jace(&mut engine, 0);

    assert_eq!(engine.state.objects[&jace].owner, P10);
    assert_eq!(engine.state.objects[&jace].controller, P10);
    assert_eq!(
        engine.state.objects[&jace].counter_count(CounterKind::Loyalty),
        10
    );
}

#[test]
fn opposing_vorinclex_halves_jaces_actual_entry_loyalty() {
    let mut engine = game(26_102_806);
    cast_vorinclex(&mut engine, 0);

    end_active_turn(&mut engine, P10);
    advance_to_main1_from_game_start(&mut engine);
    let jace = cast_jace(&mut engine, 1);

    assert_eq!(engine.state.objects[&jace].owner, P20);
    assert_eq!(engine.state.objects[&jace].controller, P20);
    assert_eq!(
        engine.state.objects[&jace].counter_count(CounterKind::Loyalty),
        2
    );
}

#[test]
fn opposing_vorinclex_does_not_change_jaces_negative_loyalty_cost() {
    let mut engine = game(26_102_807);

    end_active_turn(&mut engine, P10);
    advance_to_main1_from_game_start(&mut engine);
    let jace = cast_jace(&mut engine, 1);
    assert_eq!(
        engine.state.objects[&jace].counter_count(CounterKind::Loyalty),
        5
    );

    end_active_turn(&mut engine, P20);
    advance_to_main1_from_game_start(&mut engine);
    cast_vorinclex(&mut engine, 0);

    end_active_turn(&mut engine, P10);
    advance_to_main1_from_game_start(&mut engine);
    let victim = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    engine
        .apply_scenario_command(
            P20,
            &activate_ability_for(&engine, jace, 1, target_object(victim)),
        )
        .expect("activate Jace's actual -2 ability under the opposing Vorinclex");

    assert_eq!(
        engine.state.objects[&jace].counter_count(CounterKind::Loyalty),
        3
    );
    assert_eq!(engine.state.stack.len(), 1);
    assert_eq!(engine.state.stack[0].source_permanent_id, Some(jace));
}

#[test]
fn opponent_vorinclex_halves_jaces_plus_one_to_zero_and_still_consumes_one_use() {
    let mut engine = game(26_102_803);

    end_active_turn(&mut engine, P10);
    assert_eq!(engine.state.active_player_id(), P20);
    advance_to_main1_from_game_start(&mut engine);
    let jace = cast_jace(&mut engine, 1);
    assert_eq!(engine.state.objects[&jace].owner, P20);
    assert_eq!(
        engine.state.objects[&jace].counter_count(CounterKind::Loyalty),
        5
    );

    end_active_turn(&mut engine, P20);
    assert_eq!(engine.state.active_player_id(), P10);
    advance_to_main1_from_game_start(&mut engine);
    let vorinclex = cast_vorinclex(&mut engine, 0);
    assert_eq!(engine.state.objects[&vorinclex].controller, P10);

    end_active_turn(&mut engine, P10);
    assert_eq!(engine.state.active_player_id(), P20);
    advance_to_main1_from_game_start(&mut engine);
    assert_eq!(engine.state.priority_player_id(), P20);
    assert_eq!(
        engine.state.objects[&jace].counter_count(CounterKind::Loyalty),
        5
    );
    assert!(engine.state.stack.is_empty());

    let first_activation = activate_ability_for(&engine, jace, 0, vec![]);
    engine
        .apply_scenario_command(P20, &first_activation)
        .expect("P20's actual +1 remains activatable when its counter cost is halved to zero");
    assert_eq!(
        engine.state.objects[&jace].counter_count(CounterKind::Loyalty),
        5
    );
    assert_eq!(engine.state.stack.len(), 1);
    assert_eq!(engine.state.stack[0].source_permanent_id, Some(jace));
    assert_eq!(engine.state.priority_player_id(), P20);

    let before_rejection = engine.diagnostic_snapshot().unwrap();
    let second_activation = activate_ability_for(&engine, jace, 0, vec![]);
    assert!(engine
        .apply_scenario_command(P20, &second_activation)
        .is_err());
    assert_eq!(engine.diagnostic_snapshot().unwrap(), before_rejection);
    assert_eq!(
        engine.state.objects[&jace].counter_count(CounterKind::Loyalty),
        5
    );
    assert_eq!(engine.state.stack.len(), 1);
}

#[test]
fn opposing_vorinclex_zeroes_proliferate_object_and_player_without_false_recipient_logs() {
    let mut engine = game(26_102_804);
    cast_vorinclex(&mut engine, 0);

    end_active_turn(&mut engine, P10);
    advance_to_main1_from_game_start(&mut engine);
    let bastion = inject_permanent_on_battlefield(&mut engine, 1, "karns_bastion");
    let bear = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    engine
        .state
        .objects
        .get_mut(&bear)
        .unwrap()
        .set_counter(CounterKind::PlusOnePlusOne, 1);
    let actor_index = engine.state.player_idx(P20).expect("Proliferate actor");
    engine.state.players[actor_index]
        .counters
        .insert(CounterKind::Poison, 1);
    give_mana(
        &mut engine,
        P20,
        ManaGift {
            c: 4,
            ..Default::default()
        },
    );

    engine
        .apply_scenario_command(P20, &activate_ability_for(&engine, bastion, 1, vec![]))
        .expect("activate Karn's Bastion on P20's turn");
    pass_both_players(&mut engine);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .presentation
            .choice_kind,
        tricerules_proto::ruled::v1::ChoiceKind::Proliferate
    );
    let batch = engine
        .apply_scenario_command(P20, &proliferate_choice(vec![bear], vec![P20]))
        .expect("Proliferate both the permanent and player counters to zero");

    assert_eq!(
        engine.state.objects[&bear].counter_count(CounterKind::PlusOnePlusOne),
        1
    );
    assert_eq!(
        engine.state.players[actor_index].counters[&CounterKind::Poison],
        1
    );
    let logs = batch
        .events
        .iter()
        .filter_map(|event| match event.ev.as_ref()? {
            tricerules_proto::ruled::v1::ruled_event::Ev::Log(log) => Some(log.text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        logs.iter()
            .filter(|log| **log == "P20 proliferates.")
            .count(),
        1
    );
    assert!(
        !logs
            .iter()
            .any(|log| log.contains("Grizzly Bears gets one of each kind of counter")),
        "zero object counters must not be logged as placed: {logs:?}"
    );
    assert!(
        !logs
            .iter()
            .any(|log| log.contains("P20 gets one of each kind of counter")),
        "zero player counters must not be logged as placed: {logs:?}"
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn laezel_then_opposing_vorinclex_replays_the_accepted_cost_choice() {
    use prost::Message;

    fn fixture() -> (GameEngine, u32, u32, u32) {
        let mut engine = game(26_102_808);
        end_active_turn(&mut engine, P10);
        advance_to_main1_from_game_start(&mut engine);
        let jace = cast_jace(&mut engine, 1);
        let laezel = inject_creature_on_battlefield(&mut engine, 1, LAEZEL);

        end_active_turn(&mut engine, P20);
        advance_to_main1_from_game_start(&mut engine);
        let vorinclex = cast_vorinclex(&mut engine, 0);

        end_active_turn(&mut engine, P10);
        advance_to_main1_from_game_start(&mut engine);
        (engine, vorinclex, laezel, jace)
    }

    fn replay_one(
        original: &mut GameEngine,
        replay: &mut GameEngine,
        actor: i32,
        command: RuledCommand,
    ) -> tricerules_proto::ruled::v1::RuledEventBatch {
        let decoded = RuledCommand::decode(command.encode_to_vec().as_slice())
            .expect("decode accepted wire command");
        let actual = original
            .apply_command(actor, &command)
            .expect("accepted command");
        let repeated = replay
            .apply_command(actor, &decoded)
            .expect("replayed accepted wire command");
        assert_eq!(actual, repeated);
        assert_eq!(
            original.diagnostic_snapshot().unwrap(),
            replay.diagnostic_snapshot().unwrap()
        );
        actual
    }

    let (mut original, vorinclex, laezel, jace) = fixture();
    let (mut replay, replay_vorinclex, replay_laezel, replay_jace) = fixture();
    assert_eq!(
        (vorinclex, laezel, jace),
        (replay_vorinclex, replay_laezel, replay_jace)
    );
    assert_eq!(original.state.priority_player_id(), P20);

    let activation = activate_ability_for(&original, jace, 0, vec![]);
    let offer = replay_one(&mut original, &mut replay, P20, activation);
    let choice = find_resolution_choice(&offer).expect("Lae'zel and Vorinclex require ordering");
    assert_eq!(
        choice.choice_kind,
        tricerules_proto::ruled::v1::ChoiceKind::ReplacementEffect as i32
    );
    assert_eq!(choice.candidate_object_ids.len(), 2);
    assert_ne!(choice.resolution_prompt_id, 0);
    let laezel_index = choice
        .candidate_names
        .iter()
        .position(|name| name.contains("Lae'zel"))
        .expect("the real Lae'zel replacement is offered");
    assert!(choice
        .candidate_names
        .iter()
        .any(|name| name.contains("Vorinclex")));

    let mut answer = submit_resolution_choice(vec![choice.candidate_object_ids[laezel_index]]);
    let Some(Cmd::SubmitResolutionChoice(submission)) = answer.cmd.as_mut() else {
        unreachable!()
    };
    submission.resolution_prompt_id = choice.resolution_prompt_id;
    let paid = replay_one(&mut original, &mut replay, P20, answer);
    assert!(find_resolution_choice(&paid).is_none());
    assert_eq!(
        original.state.objects[&jace].counter_count(CounterKind::Loyalty),
        6
    );
    assert_eq!(original.state.stack.len(), 1);
    assert_eq!(
        original.diagnostic_snapshot().unwrap(),
        replay.diagnostic_snapshot().unwrap()
    );
}
