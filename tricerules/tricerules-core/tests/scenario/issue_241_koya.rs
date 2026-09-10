use super::helpers::*;
use tricerules_core::{GameEngine, TurnStep, Zone};
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, ChooseTriggerTarget, ResolutionChoiceDecision, RuledCommand,
    SubmitResolutionChoice,
};

fn choose_trigger_targets(targets: Vec<TargetRef>) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: Vec::new(),
            targets,
        })),
    }
}

fn select_payment_branch() -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            decision: ResolutionChoiceDecision::SelectBranch as i32,
            selected_branch_index: 0,
            ..Default::default()
        })),
    }
}

fn setup(seed: u64) -> (GameEngine, u32) {
    let mut engine = GameEngine::new(
        seed,
        &[0, 1],
        20,
        Some(vec![
            deck_with("plains", &["koya,_death_from_above"]),
            forest_only_deck(),
        ]),
        true,
    )
    .expect("Koya is registered");
    advance_to_main1_from_game_start(&mut engine);
    let target = inject_creature_on_battlefield(&mut engine, 1, "hill_giant");
    ensure_card_in_hand(&mut engine, 0, "koya,_death_from_above");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "koya,_death_from_above");
    engine
        .apply_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast Koya");
    pass_both_players(&mut engine);
    assert_eq!(engine.state.pending_triggers.len(), 1);
    (engine, target)
}

fn koya_id(engine: &GameEngine) -> u32 {
    engine
        .state
        .objects
        .values()
        .find(|object| {
            object.card_id == "koya,_death_from_above" && object.zone == Zone::Battlefield
        })
        .expect("Koya permanent")
        .id
}

fn resolve_etb(engine: &mut GameEngine, targets: Vec<TargetRef>) {
    engine
        .apply_command(0, &choose_trigger_targets(targets))
        .expect("choose Koya ETB target");
    pass_both_players(engine);
}

fn advance_to_next_end_step(engine: &mut GameEngine) {
    for _ in 0..10 {
        if engine.state.turn_step == TurnStep::EndStep {
            break;
        }
        let active = engine.state.active_player_id();
        engine
            .apply_command(active, &primitive_yield())
            .expect("advance toward end step");
    }
    assert_eq!(engine.state.turn_step, TurnStep::EndStep);
    assert_eq!(engine.state.stack.len(), 1, "one Koya delayed trigger");
}

fn park_delayed_choice(engine: &mut GameEngine) {
    advance_to_next_end_step(engine);
    pass_both_players(engine);
    assert!(engine.state.pending_resolution.is_some());
}

#[test]
fn issue_241_zero_targets_creates_no_delayed_trigger_and_self_is_illegal() {
    let (mut zero, _) = setup(241_001);
    resolve_etb(&mut zero, Vec::new());
    assert!(zero.state.active_event_observers.is_empty());

    let (mut illegal, _) = setup(241_002);
    let source = koya_id(&illegal);
    assert!(illegal
        .apply_command(0, &choose_trigger_targets(target_object(source)))
        .is_err());
    assert_eq!(illegal.state.pending_triggers.len(), 1);
}

#[test]
fn issue_241_declining_returns_the_exact_card_under_its_owner() {
    let (mut engine, target) = setup(241_010);
    resolve_etb(&mut engine, target_object(target));
    let exiled_generation = engine.state.zone_change_generation[&target];
    assert_eq!(engine.state.objects[&target].zone, Zone::Exile);
    assert_eq!(engine.state.active_event_observers.len(), 1);

    let source = koya_id(&engine);
    engine.state.players[0]
        .battlefield
        .retain(|oid| *oid != source);
    engine.state.players[0].graveyard.push(source);
    engine.state.objects.get_mut(&source).unwrap().zone = Zone::Graveyard;
    *engine
        .state
        .zone_change_generation
        .entry(source)
        .or_default() += 1;

    park_delayed_choice(&mut engine);
    engine
        .apply_command(
            0,
            &submit_resolution_decision(ResolutionChoiceDecision::Decline),
        )
        .expect("decline payment");
    assert_eq!(engine.state.objects[&target].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&target].controller, 1);
    assert_eq!(
        engine.state.zone_change_generation[&target],
        exiled_generation + 1
    );
}

#[test]
fn issue_241_paying_leaves_the_card_exiled() {
    let (mut engine, target) = setup(241_020);
    resolve_etb(&mut engine, target_object(target));
    park_delayed_choice(&mut engine);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 1,
            c: 3,
            ..Default::default()
        },
    );
    engine
        .apply_command(0, &select_payment_branch())
        .expect("choose to pay");
    submit_mana_resolution_decision(&mut engine, 0, ResolutionChoiceDecision::PayMana)
        .expect("pay {3}{B}");
    assert_eq!(engine.state.objects[&target].zone, Zone::Exile);
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn issue_241_declining_from_payment_rewinds_only_payment_time_mana() {
    let (mut engine, target) = setup(241_025);
    resolve_etb(&mut engine, target_object(target));
    park_delayed_choice(&mut engine);
    let swamp = inject_permanent_on_battlefield(&mut engine, 0, "swamp");
    engine.state.players[0].mana_pool.colorless = 1;
    engine
        .apply_command(0, &select_payment_branch())
        .expect("enter payment");
    engine
        .apply_command(0, &activate_ability(swamp, 0, Vec::new()))
        .expect("tap Swamp during payment");
    assert!(engine.state.objects[&swamp].tapped);
    assert_eq!(engine.state.players[0].mana_pool.black, 1);

    engine
        .apply_command(
            0,
            &submit_resolution_decision(ResolutionChoiceDecision::Decline),
        )
        .expect("decline after entering payment");
    assert_eq!(engine.state.objects[&target].zone, Zone::Battlefield);
    assert!(!engine.state.objects[&swamp].tapped);
    assert_eq!(engine.state.players[0].mana_pool.black, 0);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 1);
}

#[test]
fn issue_241_a_later_exile_incarnation_is_not_returned() {
    let (mut engine, target) = setup(241_030);
    resolve_etb(&mut engine, target_object(target));
    let captured_generation = engine.state.zone_change_generation[&target];

    engine.state.players[1].exile.retain(|oid| *oid != target);
    engine.state.players[1].hand.push(target);
    engine.state.objects.get_mut(&target).unwrap().zone = Zone::Hand;
    *engine
        .state
        .zone_change_generation
        .entry(target)
        .or_default() += 1;
    engine.state.players[1].hand.retain(|oid| *oid != target);
    engine.state.players[1].exile.push(target);
    engine.state.objects.get_mut(&target).unwrap().zone = Zone::Exile;
    *engine
        .state
        .zone_change_generation
        .entry(target)
        .or_default() += 1;
    assert!(engine.state.zone_change_generation[&target] > captured_generation);

    park_delayed_choice(&mut engine);
    engine
        .apply_command(
            0,
            &submit_resolution_decision(ResolutionChoiceDecision::Decline),
        )
        .expect("decline payment");
    assert_eq!(engine.state.objects[&target].zone, Zone::Exile);
}

#[test]
fn issue_241_exiled_token_still_creates_the_delayed_payment_choice() {
    let (mut engine, _) = setup(241_035);
    let token = inject_creature_on_battlefield(&mut engine, 1, "soldier_w_1_1");
    resolve_etb(&mut engine, target_object(token));
    assert!(!engine.state.objects.contains_key(&token));
    assert_eq!(engine.state.active_event_observers.len(), 1);

    park_delayed_choice(&mut engine);
    engine
        .apply_command(
            0,
            &submit_resolution_decision(ResolutionChoiceDecision::Decline),
        )
        .expect("decline the token's payment choice");
    assert!(!engine.state.objects.contains_key(&token));
}

#[test]
fn issue_241_accepted_commands_replay_deterministically() {
    fn run() -> (u64, Zone, u64, Vec<i32>, usize, usize) {
        let (mut engine, target) = setup(241_040);
        resolve_etb(&mut engine, target_object(target));
        park_delayed_choice(&mut engine);
        engine
            .apply_command(
                0,
                &submit_resolution_decision(ResolutionChoiceDecision::Decline),
            )
            .unwrap();
        (
            engine.state.command_index,
            engine.state.objects[&target].zone,
            engine.state.zone_change_generation[&target],
            engine
                .state
                .players
                .iter()
                .map(|player| player.life)
                .collect(),
            engine.state.stack.len(),
            engine.state.active_event_observers.len(),
        )
    }

    assert_eq!(run(), run());
}
