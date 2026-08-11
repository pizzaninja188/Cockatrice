use crate::helpers::*;

fn engine_with_card(seed: u64, card_id: &str) -> GameEngine {
    let decks = Some(vec![deck_with("mountain", &[card_id]), forest_only_deck()]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, card_id);
    engine
}

fn enter_declare_attackers(engine: &mut GameEngine) {
    engine
        .apply_command(0, &primitive_yield())
        .expect("main1 to beginning of combat");
    pass_both_players(engine);
    assert_eq!(
        engine.state.turn_step,
        tricerules_core::TurnStep::DeclareAttackers
    );
}

fn advance_to_main2(engine: &mut GameEngine) -> RuledEventBatch {
    for _ in 0..20 {
        let actor = engine.state.priority_player_id();
        let batch = engine
            .apply_command(actor, &pass())
            .expect("pass through combat");
        if engine.state.turn_step == tricerules_core::TurnStep::Main2 {
            return batch;
        }
    }
    panic!("combat did not reach main2");
}

fn advance_to_next_turn(engine: &mut GameEngine) {
    let starting_turn = engine.state.turn;
    for _ in 0..20 {
        let (actor, command) = match engine.state.cleanup_discard_player {
            Some(player) => {
                let player_index = engine.state.player_idx(player).expect("cleanup player");
                let excess = engine.state.players[player_index].hand.len() - 7;
                (player, discard_cleanup_batch((0..excess as u32).collect()))
            }
            None => (engine.state.priority_player_id(), pass()),
        };
        engine
            .apply_command(actor, &command)
            .expect("pass through the end of the turn");
        if engine.state.turn > starting_turn {
            return;
        }
    }
    panic!("game did not reach the next turn");
}

fn last_zone_view(batch: &RuledEventBatch) -> &tricerules_proto::ruled::v1::ZoneViewSync {
    batch
        .events
        .iter()
        .rev()
        .find_map(|event| match &event.ev {
            Some(Ev::ZoneView(view)) => Some(view),
            _ => None,
        })
        .expect("batch carries a zone view")
}

fn battlefield_power(
    view: &tricerules_proto::ruled::v1::ZoneViewSync,
    player_id: i32,
    object_id: u32,
) -> u32 {
    view.per_player
        .iter()
        .find(|player| player.player_id == player_id)
        .expect("player view")
        .battlefield_objects
        .iter()
        .find(|object| object.object_id == object_id)
        .expect("battlefield object")
        .power
}

#[test]
fn attacking_scope_trumpet_blast_snapshots_only_current_attackers() {
    let mut engine = engine_with_card(76_001, "trumpet_blast");
    let attacker = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let bystander = inject_creature_on_battlefield(&mut engine, 0, "savannah_lions");
    enter_declare_attackers(&mut engine);
    engine
        .apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare one attacker");

    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 3,
            ..Default::default()
        },
    );
    let spell = hand_index_for_card(&engine, 0, "trumpet_blast");
    engine
        .apply_command(0, &cast_spell(spell, vec![]))
        .expect("cast Trumpet Blast after attackers");
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.effective_power(attacker), Some(4));
    assert_eq!(
        engine.effective_power(bystander),
        Some(2),
        "a creature that is not attacking is outside the snapshot"
    );

    advance_to_main2(&mut engine);
    assert_eq!(
        engine.effective_power(attacker),
        Some(4),
        "the one-shot bonus remains after the creature stops attacking"
    );
    advance_to_next_turn(&mut engine);
    assert_eq!(
        engine.effective_power(attacker),
        Some(2),
        "the one-shot bonus expires at cleanup"
    );
}

#[test]
fn attacking_scope_trumpet_blast_before_attackers_affects_nothing() {
    let mut engine = engine_with_card(76_002, "trumpet_blast");
    let attacker = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    engine
        .apply_command(0, &primitive_yield())
        .expect("main1 to beginning of combat");

    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 3,
            ..Default::default()
        },
    );
    let spell = hand_index_for_card(&engine, 0, "trumpet_blast");
    engine
        .apply_command(0, &cast_spell(spell, vec![]))
        .expect("cast Trumpet Blast before attackers");
    resolve_entire_stack_two_player(&mut engine);

    pass_both_players(&mut engine);
    assert_eq!(
        engine.state.turn_step,
        tricerules_core::TurnStep::DeclareAttackers
    );
    engine
        .apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare attacker after Trumpet Blast resolved");
    assert_eq!(
        engine.effective_power(attacker),
        Some(2),
        "one-shot characteristic effects do not acquire later attackers"
    );
}

#[test]
fn attacking_scope_warded_battlements_tracks_current_combat_membership() {
    let mut engine = engine_with_card(76_003, "warded_battlements");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 3,
            ..Default::default()
        },
    );
    let battlements = hand_index_for_card(&engine, 0, "warded_battlements");
    engine
        .apply_command(0, &cast_spell(battlements, vec![]))
        .expect("cast Warded Battlements");
    resolve_entire_stack_two_player(&mut engine);

    let attacker = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let bystander = inject_creature_on_battlefield(&mut engine, 0, "savannah_lions");
    let opponent = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    enter_declare_attackers(&mut engine);
    let declared = engine
        .apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare one attacker");

    assert_eq!(engine.effective_power(attacker), Some(3));
    assert_eq!(engine.effective_power(bystander), Some(2));
    assert_eq!(engine.effective_power(opponent), Some(2));

    let declared_view = last_zone_view(&declared);
    assert!(
        !declared_view.battlefields_unchanged,
        "combat membership changes derived characteristics and must resend battlefields"
    );
    assert_eq!(battlefield_power(declared_view, 0, attacker), 3);
    assert_eq!(battlefield_power(declared_view, 0, bystander), 2);

    let main2 = advance_to_main2(&mut engine);
    assert_eq!(engine.effective_power(attacker), Some(2));
    let main2_view = last_zone_view(&main2);
    assert!(!main2_view.battlefields_unchanged);
    assert_eq!(battlefield_power(main2_view, 0, attacker), 2);
}

#[test]
fn attacking_scope_one_shot_keyword_grants_do_not_acquire_later_creatures() {
    let mut engine = engine_with_card(76_004, "overrun");
    let existing = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 5,
            ..Default::default()
        },
    );
    let overrun = hand_index_for_card(&engine, 0, "overrun");
    engine
        .apply_command(0, &cast_spell(overrun, vec![]))
        .expect("cast Overrun");
    resolve_entire_stack_two_player(&mut engine);
    assert!(engine.effective_has_keyword(existing, tricerules_cards::Keyword::Trample));

    let newcomer = inject_creature_on_battlefield(&mut engine, 0, "savannah_lions");
    assert!(
        !engine.effective_has_keyword(newcomer, tricerules_cards::Keyword::Trample),
        "CR 611.2c locks the affected set when Overrun resolves"
    );
}
