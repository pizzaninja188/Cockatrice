use super::helpers::*;
use tricerules_cards::{primitives::CounterKind, CardRegistry};
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, ResolutionChoiceDecision, RuledCommand, SubmitResolutionChoice,
};

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![
        std::iter::repeat_n("forest".to_string(), 20).collect(),
        std::iter::repeat_n("mountain".to_string(), 20).collect(),
    ]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn select_branch(index: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            chosen_object_ids: Vec::new(),
            decision: ResolutionChoiceDecision::SelectBranch as i32,
            selected_branch_index: index,
            cast_spell: None,
        })),
    }
}

fn cast_endure_creature(engine: &mut GameEngine, card_id: &str) -> u32 {
    let object_id = inject_card_into_hand(engine, 0, card_id);
    grant_pool(engine, 0);
    let index = hand_index_for_card(engine, 0, card_id);
    engine
        .apply_command(0, &cast_spell(index, Vec::new()))
        .expect("cast Endure creature");
    pass_both_players(engine);
    object_id
}

fn transfer_control(engine: &mut GameEngine, object_id: u32, to_player: usize) {
    for player in &mut engine.state.players {
        player
            .battlefield
            .retain(|candidate| *candidate != object_id);
    }
    let player_id = engine.state.players[to_player].id;
    engine.state.players[to_player].battlefield.push(object_id);
    let object = engine.state.objects.get_mut(&object_id).expect("source");
    object.base_controller = player_id;
    object.controller = player_id;
}

#[test]
fn issue_142_endure_cards_are_registered() {
    let registry = CardRegistry::global();

    for card_id in [
        "kin-tree_nurturer",
        "dusyut_earthcarver",
        "sandskitter_outrider",
    ] {
        assert!(
            registry.get(card_id).is_some(),
            "issue #142 requires {card_id} to be implemented"
        );
    }
}

#[test]
fn endure_one_two_and_three_offer_both_branches_and_apply_the_choice() {
    for (seed, card_id, amount, token_id) in [
        (142_001, "kin-tree_nurturer", 1, "spirit_w_1_1"),
        (142_002, "sandskitter_outrider", 2, "spirit_w_2_2"),
        (142_003, "dusyut_earthcarver", 3, "spirit_w_3_3"),
    ] {
        let mut counters_engine = engine(seed);
        let source = cast_endure_creature(&mut counters_engine, card_id);
        pass_both_players(&mut counters_engine);
        let pending = counters_engine
            .state
            .pending_resolution
            .as_ref()
            .expect("Endure branch choice");
        assert_eq!(pending.deciding_player, 0);
        counters_engine
            .apply_command(0, &select_branch(0))
            .expect("choose counters");
        assert_eq!(
            counters_engine
                .state
                .objects
                .get(&source)
                .expect("source")
                .counter_count(CounterKind::PlusOnePlusOne),
            amount
        );
        assert!(battlefield_token_oids(&counters_engine, 0, token_id).is_empty());

        let mut token_engine = engine(seed + 100);
        cast_endure_creature(&mut token_engine, card_id);
        pass_both_players(&mut token_engine);
        token_engine
            .apply_command(0, &select_branch(1))
            .expect("choose Spirit");
        assert_eq!(battlefield_token_oids(&token_engine, 0, token_id).len(), 1);
    }
}

#[test]
fn removed_source_forces_the_spirit_fallback_without_a_prompt() {
    let mut engine = engine(142_010);
    let source = cast_endure_creature(&mut engine, "kin-tree_nurturer");
    inject_card_into_hand(&mut engine, 0, "lightning_bolt");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let bolt = hand_index_for_card(&engine, 0, "lightning_bolt");
    engine
        .apply_command(0, &cast_spell(bolt, target_object(source)))
        .expect("cast Lightning Bolt over the Endure trigger");
    pass_both_players(&mut engine);
    assert_eq!(
        engine.state.objects.get(&source).expect("source").zone,
        tricerules_core::Zone::Graveyard
    );

    pass_both_players(&mut engine);
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(battlefield_token_oids(&engine, 0, "spirit_w_1_1").len(), 1);
}

#[test]
fn source_controller_chooses_and_receives_the_spirit() {
    let mut engine = engine(142_020);
    let source = cast_endure_creature(&mut engine, "sandskitter_outrider");
    transfer_control(&mut engine, source, 1);

    pass_both_players(&mut engine);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .expect("Endure prompt")
            .deciding_player,
        1
    );
    engine
        .apply_command(1, &select_branch(1))
        .expect("source controller chooses Spirit");
    assert_eq!(battlefield_token_oids(&engine, 1, "spirit_w_2_2").len(), 1);
    assert!(battlefield_token_oids(&engine, 0, "spirit_w_2_2").is_empty());
}

#[test]
fn source_controller_lki_routes_forced_fallback_after_source_leaves() {
    let mut engine = engine(142_030);
    let source = cast_endure_creature(&mut engine, "kin-tree_nurturer");
    transfer_control(&mut engine, source, 1);
    inject_card_into_hand(&mut engine, 0, "lightning_bolt");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let bolt = hand_index_for_card(&engine, 0, "lightning_bolt");
    engine
        .apply_command(0, &cast_spell(bolt, target_object(source)))
        .expect("cast Lightning Bolt");
    pass_both_players(&mut engine);
    pass_both_players(&mut engine);

    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(battlefield_token_oids(&engine, 1, "spirit_w_1_1").len(), 1);
    assert!(battlefield_token_oids(&engine, 0, "spirit_w_1_1").is_empty());
}

#[test]
fn leave_and_return_generation_forces_fallback_and_stale_counter_choice_is_restored() {
    let mut engine = engine(142_040);
    let source = cast_endure_creature(&mut engine, "kin-tree_nurturer");
    pass_both_players(&mut engine);
    let original_generation = engine
        .state
        .zone_change_generation
        .get(&source)
        .copied()
        .unwrap_or(0);
    engine
        .state
        .last_known_controller_by_generation
        .insert((source, original_generation), 0);
    *engine
        .state
        .zone_change_generation
        .entry(source)
        .or_insert(0) += 2;

    assert!(engine.apply_command(0, &select_branch(0)).is_err());
    assert!(engine.state.pending_resolution.is_some());
    engine
        .apply_command(0, &select_branch(1))
        .expect("fallback remains legal");
    assert_eq!(battlefield_token_oids(&engine, 0, "spirit_w_1_1").len(), 1);
    assert_eq!(
        engine
            .state
            .objects
            .get(&source)
            .expect("returned source")
            .counter_count(CounterKind::PlusOnePlusOne),
        0
    );
}

#[test]
fn endure_replay_is_deterministic_for_the_same_seed_and_commands() {
    fn replay() -> (usize, u32, u64) {
        let mut engine = engine(142_050);
        let source = cast_endure_creature(&mut engine, "dusyut_earthcarver");
        pass_both_players(&mut engine);
        engine
            .apply_command(0, &select_branch(0))
            .expect("choose counters");
        (
            battlefield_token_oids(&engine, 0, "spirit_w_3_3").len(),
            engine
                .state
                .objects
                .get(&source)
                .expect("source")
                .counter_count(CounterKind::PlusOnePlusOne),
            engine.state.command_index,
        )
    }

    assert_eq!(replay(), replay());
}
