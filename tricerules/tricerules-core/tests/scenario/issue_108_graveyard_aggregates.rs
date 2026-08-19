//! Issue #108: controller-relative graveyard card and distinct-card-type conditions.

use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::{GameEngine, TurnStep, Zone};
use tricerules_proto::ruled::v1::dev_command::Dev;
use tricerules_proto::ruled::v1::ruled_event::Ev;
use tricerules_proto::ruled::v1::{DevCommand, DevPutCardInZone, DevZone, RuledCommand};

fn put_ready(player: i32, card_name: &str) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::DevCommand(DevCommand {
            target_player_id: player,
            dev: Some(Dev::PutCardInZone(DevPutCardInZone {
                card_name: card_name.to_owned(),
                zone: DevZone::Battlefield as i32,
                ready: true,
            })),
        })),
    }
}

fn put(engine: &mut GameEngine, player: i32, card_name: &str, card_id: &str) -> u32 {
    engine
        .apply_command(player, &put_ready(player, card_name))
        .unwrap_or_else(|error| panic!("put {card_name}: {error:?}"));
    battlefield_object_for_card(engine, player as usize, card_id)
}

fn issue_engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("swamp", &[]), deck_with("island", &[])]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new game");
    engine.enable_dev_commands();
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn remove_from_graveyard(engine: &mut GameEngine, player: usize, object_id: u32) {
    engine.state.players[player]
        .graveyard
        .retain(|candidate| *candidate != object_id);
    engine
        .state
        .objects
        .get_mut(&object_id)
        .expect("graveyard object")
        .zone = Zone::Exile;
}

fn declare_as_attacker(engine: &mut GameEngine, attacker: u32) {
    engine
        .apply_command(0, &primitive_yield())
        .expect("move to beginning of combat");
    pass_both_players(engine);
    assert_eq!(engine.state.turn_step, TurnStep::DeclareAttackers);
    engine
        .apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare attacker");
}

#[test]
fn issue_108_crypt_feaster_checks_seven_cards_at_trigger_and_resolution() {
    let mut below_threshold = issue_engine(108_001);
    let crypt = put(&mut below_threshold, 0, "Crypt Feaster", "crypt_feaster");
    for _ in 0..6 {
        inject_graveyard_card(&mut below_threshold, 0, "swamp");
    }
    inject_graveyard_card(&mut below_threshold, 0, "zombie_b_2_2");
    inject_graveyard_card(&mut below_threshold, 1, "island");
    declare_as_attacker(&mut below_threshold, crypt);
    assert!(
        below_threshold.state.stack.is_empty(),
        "an opponent's card and a token must not satisfy threshold"
    );

    let mut loses_threshold = issue_engine(108_002);
    let crypt = put(&mut loses_threshold, 0, "Crypt Feaster", "crypt_feaster");
    let graveyard_cards = (0..7)
        .map(|_| inject_graveyard_card(&mut loses_threshold, 0, "swamp"))
        .collect::<Vec<_>>();
    declare_as_attacker(&mut loses_threshold, crypt);
    assert_eq!(loses_threshold.state.stack.len(), 1);
    remove_from_graveyard(&mut loses_threshold, 0, graveyard_cards[0]);
    pass_both_players(&mut loses_threshold);
    assert_eq!(loses_threshold.effective_power(crypt), Some(3));

    let mut keeps_threshold = issue_engine(108_003);
    let crypt = put(&mut keeps_threshold, 0, "Crypt Feaster", "crypt_feaster");
    for _ in 0..7 {
        inject_graveyard_card(&mut keeps_threshold, 0, "swamp");
    }
    declare_as_attacker(&mut keeps_threshold, crypt);
    pass_both_players(&mut keeps_threshold);
    assert_eq!(keeps_threshold.effective_power(crypt), Some(5));
    assert_eq!(keeps_threshold.effective_toughness(crypt), Some(4));
}

#[test]
fn issue_108_spineseeker_uses_live_distinct_printed_card_types() {
    let mut engine = issue_engine(108_004);
    let centipede = put(
        &mut engine,
        0,
        "Spineseeker Centipede",
        "spineseeker_centipede",
    );
    assert_eq!(engine.effective_power(centipede), Some(2));
    assert_eq!(engine.effective_toughness(centipede), Some(1));
    assert!(!engine.effective_has_keyword(centipede, Keyword::Vigilance));

    inject_graveyard_card(&mut engine, 0, "ornithopter");
    inject_graveyard_card(&mut engine, 1, "eyeblights_ending");
    inject_graveyard_card(&mut engine, 0, "zombie_b_2_2");
    engine.state.players[0].graveyard.push(u32::MAX);
    let stale = inject_graveyard_card(&mut engine, 0, "eyeblights_ending");
    engine
        .state
        .objects
        .get_mut(&stale)
        .expect("stale card")
        .zone = Zone::Exile;
    assert_eq!(
        engine.effective_power(centipede),
        Some(2),
        "opponents' cards, tokens, missing objects, and stale graveyard entries are ignored"
    );

    let kindred_instant = inject_graveyard_card(&mut engine, 0, "eyeblights_ending");
    assert_eq!(
        engine.effective_power(centipede),
        Some(3),
        "Artifact Creature plus Kindred Instant supplies four distinct card types"
    );
    assert_eq!(engine.effective_toughness(centipede), Some(3));
    assert!(engine.effective_has_keyword(centipede, Keyword::Vigilance));

    inject_graveyard_card(&mut engine, 0, "ornithopter");
    assert_eq!(
        engine.effective_power(centipede),
        Some(3),
        "duplicate card types do not change delirium"
    );

    remove_from_graveyard(&mut engine, 0, kindred_instant);
    assert_eq!(engine.effective_power(centipede), Some(2));
    assert_eq!(engine.effective_toughness(centipede), Some(1));
    assert!(!engine.effective_has_keyword(centipede, Keyword::Vigilance));
}

#[test]
fn issue_108_spineseeker_searches_for_and_reveals_a_basic_land() {
    let mut engine = issue_engine(108_005);
    let forest = inject_library_card(&mut engine, 0, "forest");
    let taiga = inject_library_card(&mut engine, 0, "taiga");
    put(
        &mut engine,
        0,
        "Spineseeker Centipede",
        "spineseeker_centipede",
    );
    assert_eq!(
        engine.state.stack.len(),
        1,
        "the ETB ability is on the stack"
    );

    engine.apply_command(0, &pass()).expect("controller passes");
    let search_batch = engine
        .apply_command(1, &pass())
        .expect("opponent passes and the ETB ability resolves");
    let choice = find_resolution_choice(&search_batch).expect("basic-land search choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibrarySearch);
    assert_eq!((choice.min, choice.max), (0, 1));
    assert!(choice.candidate_object_ids.contains(&forest));
    assert!(!choice.candidate_object_ids.contains(&taiga));

    let completion = engine
        .apply_command(0, &submit_resolution_choice(vec![forest]))
        .expect("choose and reveal Forest");
    assert_eq!(engine.state.objects[&forest].zone, Zone::Hand);
    assert!(engine.state.players[0].hand.contains(&forest));
    assert!(completion.events.iter().any(|event| {
        matches!(&event.ev, Some(Ev::Log(log)) if log.text == "P0 reveals Forest.")
    }));
    assert!(completion.events.iter().any(|event| {
        matches!(&event.ev, Some(Ev::Log(log)) if log.text == "P0 shuffles their library.")
    }));
    assert!(engine.state.pending_resolution.is_none());
}
