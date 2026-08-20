use crate::helpers::*;
use tricerules_cards::CounterKind;
use tricerules_core::state::PlayerState;
use tricerules_proto::ruled::v1::{
    dev_command, ruled_command, DevCommand, DevPutCardInZone, DevZone,
};

fn engine_with_p0_cards(seed: u64, cards: &[&str]) -> GameEngine {
    let mut p0: Vec<String> = cards.iter().map(|card| (*card).to_string()).collect();
    while p0.len() < 7 {
        p0.push("mountain".to_string());
    }
    let decks = Some(vec![p0, vec!["forest".to_string(); 7]]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn plus_one_counters(engine: &GameEngine, object_id: u32) -> u32 {
    engine.state.objects[&object_id]
        .counters
        .get(&CounterKind::PlusOnePlusOne)
        .copied()
        .unwrap_or(0)
}

fn dev_put(player: i32, card_name: &str) -> RuledCommand {
    RuledCommand {
        cmd: Some(ruled_command::Cmd::DevCommand(DevCommand {
            target_player_id: player,
            dev: Some(dev_command::Dev::PutCardInZone(DevPutCardInZone {
                card_name: card_name.to_string(),
                zone: DevZone::Battlefield as i32,
                ready: false,
            })),
        })),
    }
}

#[test]
fn threshold_land_uses_every_players_life_total() {
    for (seed, third_player_life, expected_tapped) in [(97_001, 14, true), (97_002, 13, false)] {
        let mut engine = engine_with_p0_cards(seed, &["razortrap_gorge"]);
        engine.state.players[0].life = 20;
        engine.state.players[1].life = 14;
        engine
            .state
            .players
            .push(PlayerState::new(2, third_player_life));

        let land = hand_index_for_card(&engine, 0, "razortrap_gorge");
        engine
            .apply_command(0, &play_land(land))
            .expect("play Razortrap Gorge");

        let object_id = battlefield_object_for_card(&engine, 0, "razortrap_gorge");
        assert_eq!(
            engine.state.objects[&object_id].tapped, expected_tapped,
            "the condition must inspect every seat, not only one opponent"
        );
    }
}

#[test]
fn multiple_globes_use_the_existing_replacement_order_prompt() {
    let mut engine = engine_with_p0_cards(
        97_003,
        &[
            "dragonstorm_globe",
            "dragonstorm_globe",
            "sparktongue_dragon",
        ],
    );
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 9,
            r: 2,
            ..Default::default()
        },
    );

    for _ in 0..2 {
        let globe = hand_index_for_card(&engine, 0, "dragonstorm_globe");
        engine
            .apply_command(0, &cast_spell(globe, vec![]))
            .expect("cast Dragonstorm Globe");
        pass_both_players(&mut engine);
    }

    let dragon = hand_index_for_card(&engine, 0, "sparktongue_dragon");
    engine
        .apply_command(0, &cast_spell(dragon, vec![]))
        .expect("cast Sparktongue Dragon");
    pass_both_players(&mut engine);

    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("two Globe replacements require a CR 616 choice");
    assert_eq!(
        pending.presentation.choice_kind,
        ChoiceKind::ReplacementEffect
    );
    assert_eq!(pending.deciding_player, 0);
    assert_eq!(pending.presentation.candidates.len(), 2);
    assert!(matches!(
        &pending.continuation,
        ResolutionContinuation::EntryReplacement { .. }
    ));
    let application = pending.presentation.candidates[0];

    assert!(engine
        .apply_command(1, &submit_resolution_choice(vec![application]))
        .is_err());
    assert!(engine
        .apply_command(0, &submit_resolution_choice(vec![u32::MAX]))
        .is_err());
    engine
        .apply_command(0, &submit_resolution_choice(vec![application]))
        .expect("choose the first Globe replacement");

    let object_id = battlefield_object_for_card(&engine, 0, "sparktongue_dragon");
    assert_eq!(plus_one_counters(&engine, object_id), 2);
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine
        .apply_command(0, &submit_resolution_choice(vec![application]))
        .is_err());
    assert_eq!(plus_one_counters(&engine, object_id), 2);
}

#[test]
fn globe_only_affects_dragons_its_controller_controls() {
    let mut engine = engine_with_p0_cards(97_004, &[]);
    engine.enable_dev_commands();
    engine
        .apply_command(0, &dev_put(0, "Dragonstorm Globe"))
        .expect("put Globe");

    engine
        .apply_command(0, &dev_put(1, "Sparktongue Dragon"))
        .expect("put opponent's Dragon");
    let opposing_dragon = battlefield_object_for_card(&engine, 1, "sparktongue_dragon");
    assert_eq!(plus_one_counters(&engine, opposing_dragon), 0);

    engine
        .apply_command(0, &dev_put(0, "Grizzly Bears"))
        .expect("put controlled non-Dragon");
    let bear = battlefield_object_for_card(&engine, 0, "grizzly_bears");
    assert_eq!(plus_one_counters(&engine, bear), 0);

    engine
        .apply_command(0, &dev_put(0, "Sparktongue Dragon"))
        .expect("put controlled Dragon");
    let controlled_dragon = battlefield_object_for_card(&engine, 0, "sparktongue_dragon");
    assert_eq!(plus_one_counters(&engine, controlled_dragon), 1);
    assert!(engine
        .state
        .stack
        .iter()
        .any(|item| { item.source_permanent_id == Some(controlled_dragon) && item.is_triggered }));
}

#[test]
fn copy_first_rechecks_the_globe_dragon_predicate() {
    for (seed, copy_source_card, expected_counters) in [
        (97_005, "sparktongue_dragon", 1),
        (97_006, "grizzly_bears", 0),
    ] {
        let mut engine = engine_with_p0_cards(seed, &["dragonstorm_globe", "clone"]);
        give_mana(
            &mut engine,
            0,
            ManaGift {
                c: 6,
                u: 1,
                ..Default::default()
            },
        );
        let globe = hand_index_for_card(&engine, 0, "dragonstorm_globe");
        engine
            .apply_command(0, &cast_spell(globe, vec![]))
            .expect("cast Globe");
        pass_both_players(&mut engine);

        let source = inject_creature_on_battlefield(&mut engine, 1, copy_source_card);
        let clone = hand_index_for_card(&engine, 0, "clone");
        engine
            .apply_command(0, &cast_spell(clone, vec![]))
            .expect("cast Clone");
        pass_both_players(&mut engine);
        assert_eq!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .expect("copy source choice")
                .presentation
                .choice_kind,
            ChoiceKind::CopySource
        );
        engine
            .apply_command(0, &submit_resolution_choice(vec![source]))
            .expect("choose copy source");

        let clone_id = battlefield_object_for_card(&engine, 0, "clone");
        assert_eq!(
            plus_one_counters(&engine, clone_id),
            expected_counters,
            "Globe must re-evaluate the projected copied type"
        );
    }
}
