use super::helpers::*;
use crate::helpers::ScenarioCommandExt;
use tricerules_card_model::{AbilityDefinitionId, AbilityId, CardFaceId, ProgramOwnerId};
use tricerules_core::{TurnStep, Zone};
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
            resolution_prompt_id: 0,
            decision: ResolutionChoiceDecision::SelectBranch as i32,
            selected_branch_index: 0,
            ..Default::default()
        })),
    }
}

fn setup(seed: u64) -> (GameEngine, u32) {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
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
        .apply_scenario_command(0, &cast_spell(slot, Vec::new()))
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

fn remove_koya_source(engine: &mut GameEngine) {
    let source = koya_id(engine);
    engine.state.players[0]
        .battlefield
        .retain(|object_id| *object_id != source);
    engine.state.players[0].graveyard.push(source);
    engine.state.objects.get_mut(&source).unwrap().zone = Zone::Graveyard;
    *engine
        .state
        .zone_change_generation
        .entry(source)
        .or_default() += 1;
}

fn resolve_etb(engine: &mut GameEngine, targets: Vec<TargetRef>) {
    let creates_delayed_trigger = !targets.is_empty();
    engine
        .apply_scenario_command(0, &choose_trigger_targets(targets))
        .expect("choose Koya ETB target");
    pass_both_players(engine);
    if creates_delayed_trigger {
        let delayed = &engine.state.active_event_observers[0].payload;
        assert!(matches!(
            delayed,
            tricerules_core::state::EventObserverPayload::StageDelayedTrigger(payload)
                if payload.program_definition.is_some()
        ));
    } else {
        assert!(engine.state.active_event_observers.is_empty());
    }
}

fn advance_to_next_end_step(engine: &mut GameEngine) {
    for _ in 0..10 {
        if engine.state.turn_step == TurnStep::EndStep {
            break;
        }
        let active = engine.state.active_player_id();
        engine
            .apply_scenario_command(active, &primitive_yield())
            .expect("advance toward end step");
    }
    assert_eq!(engine.state.turn_step, TurnStep::EndStep);
    assert_eq!(engine.state.stack.len(), 1, "one Koya delayed trigger");
    let delayed = engine
        .state
        .stack
        .last()
        .expect("delayed trigger stack item");
    assert!(
        delayed
            .triggered_ability
            .as_ref()
            .is_some_and(|ability| ability.program.is_some()),
        "delayed trigger retained its child program"
    );
    assert!(
        delayed.trigger_use_key.is_some(),
        "delayed trigger received an authored resolving occurrence"
    );
    assert_eq!(
        delayed.ability_program_owner(),
        Some(ProgramOwnerId::ability(AbilityDefinitionId {
            card_id: "koya,_death_from_above".into(),
            face_id: CardFaceId::new("koya_death_from_above").unwrap(),
            ability_path: vec![
                AbilityId::new("triggered_01").unwrap(),
                AbilityId::new("delayed_01").unwrap(),
            ],
        })),
        "the exact nested child definition survives its Koya source leaving"
    );
}

fn koya_return_tail_program(include_early_life: bool) -> String {
    let early = if include_early_life {
        "(id: \"early_gain\", kind: Perform(GainLife(who: Controller, amount: 1))),\n            "
    } else {
        ""
    };
    include_str!("../../../tricerules-cards/data/koya,_death_from_above.ron")
        .replace("\r\n", "\n")
        .replacen(
            "steps: [\n            (id: \"pay_or_return\"",
            &format!("steps: [\n            {early}(id: \"pay_or_return\""),
            1,
        )
        .replacen(
            "            ))\n          ], yields: [])))",
            "            )),\n            (id: \"tail\", kind: Perform(GainLife(who: Controller, amount: 1)))\n          ], yields: [])))",
            1,
        )
}

fn koya_return_tail_setup(
    seed: u64,
    include_early_life: bool,
    target_card_id: &str,
) -> (GameEngine, u32) {
    let koya = koya_return_tail_program(include_early_life);
    koya_return_tail_setup_with_source(seed, &koya, target_card_id)
}

fn koya_return_tail_setup_with_source(
    seed: u64,
    koya: &str,
    target_card_id: &str,
) -> (GameEngine, u32) {
    assert!(koya.contains("id: \"tail\""));
    let registry = Box::leak(Box::new(
        tricerules_cards::CardRegistry::from_chunks_and_tokens(
            &[
                koya,
                include_str!("../../../tricerules-cards/data/clone.ron"),
                include_str!("../../../tricerules-cards/data/generated/h/hill_giant.ron"),
                include_str!("../../../tricerules-cards/data/plains.ron"),
                include_str!("../../../tricerules-cards/data/forest.ron"),
            ],
            &[include_str!(
                "../../../tricerules-cards/data/tokens/soldier_w_1_1.ron"
            )],
        )
        .expect("Koya return-tail scenario registry validates"),
    ));
    let decks = Some(vec![
        vec!["plains".to_owned(); 40],
        vec!["forest".to_owned(); 40],
    ]);
    let mut engine = GameEngine::new(registry, seed, &[0, 1], 20, decks, true)
        .expect("Koya return-tail scenario engine");
    advance_to_main1_from_game_start(&mut engine);

    let target = inject_creature_on_battlefield(&mut engine, 1, target_card_id);
    inject_card_into_hand(&mut engine, 0, "koya,_death_from_above");
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
        .apply_scenario_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast Koya with its fixture tail");
    pass_both_players(&mut engine);
    engine
        .apply_scenario_command(0, &choose_trigger_targets(target_object(target)))
        .expect("choose the exact delayed return target");
    pass_both_players(&mut engine);
    if engine.state.objects.contains_key(&target) {
        assert_eq!(engine.state.objects[&target].zone, Zone::Exile);
    } else {
        assert_eq!(
            target_card_id, "soldier_w_1_1",
            "only the exiled fixture token should have ceased"
        );
    }
    (engine, target)
}

fn koya_return_tail_three_player_setup(seed: u64) -> (GameEngine, u32) {
    let koya = koya_return_tail_program(false);
    let registry = Box::leak(Box::new(
        tricerules_cards::CardRegistry::from_chunks_and_tokens(
            &[
                &koya,
                include_str!("../../../tricerules-cards/data/clone.ron"),
                include_str!("../../../tricerules-cards/data/generated/h/hill_giant.ron"),
                include_str!("../../../tricerules-cards/data/plains.ron"),
                include_str!("../../../tricerules-cards/data/forest.ron"),
            ],
            &[],
        )
        .expect("three-player Koya return-tail registry validates"),
    ));
    let decks = Some(vec![
        vec!["plains".to_owned(); 40],
        vec!["forest".to_owned(); 40],
        vec!["forest".to_owned(); 40],
    ]);
    let mut engine = GameEngine::new(registry, seed, &[0, 1, 2], 20, decks, true)
        .expect("three-player Koya return-tail engine");
    advance_to_main1_from_game_start(&mut engine);

    let target = inject_creature_on_battlefield(&mut engine, 1, "clone");
    inject_creature_on_battlefield(&mut engine, 2, "hill_giant");
    inject_card_into_hand(&mut engine, 0, "koya,_death_from_above");
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
        .apply_scenario_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast Koya in the three-player fixture");
    pass_priority_round(&mut engine);
    engine
        .apply_scenario_command(0, &choose_trigger_targets(target_object(target)))
        .expect("choose the P1 Clone for the delayed return");
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.objects[&target].zone, Zone::Exile);
    assert_eq!(engine.state.active_event_observers.len(), 1);
    remove_koya_source(&mut engine);

    (engine, target)
}

fn duplicate_koya_delayed_program_source() -> String {
    koya_return_tail_program(false).replacen(
        "  triggered_abilities: [(",
        "  static_abilities: [(ability_id: \"static_extra\", presentation: Fallback, definition: AdditionalTriggeredAbilityInstances(controllers: Controller, source_filter: (required_subtypes: [\"Bird\"]), condition: None, additional_count: 1))],\n  triggered_abilities: [(",
        1,
    )
}

fn park_delayed_choice(engine: &mut GameEngine) {
    advance_to_next_end_step(engine);
    pass_both_players(engine);
    assert!(engine.state.pending_resolution.is_some());
}

fn advance_to_end_step_with_two_delayed_triggers(engine: &mut GameEngine) {
    for _ in 0..10 {
        if engine.state.turn_step == TurnStep::EndStep {
            break;
        }
        let active = engine.state.active_player_id();
        engine
            .apply_scenario_command(active, &primitive_yield())
            .expect("advance to the shared delayed-trigger event");
    }
    assert_eq!(engine.state.turn_step, TurnStep::EndStep);
    answer_trigger_order_in_engine_order(engine);
    assert_eq!(
        engine.state.stack.len(),
        2,
        "both delayed trigger occurrences are staged"
    );
}

fn decline_with_prompt(prompt_id: u64) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            resolution_prompt_id: prompt_id,
            decision: ResolutionChoiceDecision::Decline as i32,
            ..Default::default()
        })),
    }
}

#[test]
fn issue_241_same_parent_delayed_occurrences_have_independent_answers() {
    let doubled_koya = duplicate_koya_delayed_program_source();
    let registry = Box::leak(Box::new(
        tricerules_cards::CardRegistry::from_chunks_and_tokens(
            &[
                &doubled_koya,
                include_str!("../../../tricerules-cards/data/clone.ron"),
                include_str!("../../../tricerules-cards/data/generated/h/hill_giant.ron"),
                include_str!("../../../tricerules-cards/data/plains.ron"),
                include_str!("../../../tricerules-cards/data/forest.ron"),
            ],
            &[],
        )
        .expect("duplicated Koya registry validates"),
    ));
    let decks = Some(vec![
        vec!["plains".to_owned(); 40],
        vec!["forest".to_owned(); 40],
    ]);
    let mut engine = GameEngine::new(registry, 241_060, &[0, 1], 20, decks, true)
        .expect("duplicated Koya scenario engine");
    advance_to_main1_from_game_start(&mut engine);
    let first_target = inject_creature_on_battlefield(&mut engine, 1, "hill_giant");
    let second_target = inject_creature_on_battlefield(&mut engine, 1, "hill_giant");
    inject_card_into_hand(&mut engine, 0, "koya,_death_from_above");
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
        .apply_scenario_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast Koya with duplicated ETB");
    pass_both_players(&mut engine);
    answer_trigger_order_in_engine_order(&mut engine);
    assert!(engine.state.pending_triggers.len() == 1);
    engine
        .apply_scenario_command(0, &choose_trigger_targets(target_object(first_target)))
        .expect("choose first delayed return target");
    answer_trigger_order_in_engine_order(&mut engine);
    engine
        .apply_scenario_command(0, &choose_trigger_targets(target_object(second_target)))
        .expect("choose second delayed return target");
    pass_both_players(&mut engine);
    pass_both_players(&mut engine);
    assert_eq!(engine.state.active_event_observers.len(), 2);

    let source = koya_id(&engine);
    let source_generation = engine.state.zone_change_generation[&source];

    advance_to_end_step_with_two_delayed_triggers(&mut engine);
    pass_both_players(&mut engine);
    let first_offer = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("first resolving child has its own payment choice");
    let first_prompt_id = first_offer.resolution_prompt_id;
    let first_key = first_offer
        .continuation
        .stack()
        .expect("first child parked stack")
        .item
        .trigger_use_key
        .clone()
        .expect("first child occurrence key");

    engine
        .apply_scenario_command(0, &decline_with_prompt(first_prompt_id.get()))
        .expect("answer the first occurrence");
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.stack.len(), 1);
    pass_both_players(&mut engine);
    let second_offer = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("second resolving child has an independent payment choice");
    let second_prompt_id = second_offer.resolution_prompt_id;
    let second_key = second_offer
        .continuation
        .stack()
        .expect("second child parked stack")
        .item
        .trigger_use_key
        .clone()
        .expect("second child occurrence key");

    assert_eq!(first_key.object_id, second_key.object_id);
    assert_eq!(
        first_key.zone_change_generation,
        second_key.zone_change_generation
    );
    assert_eq!(first_key.object_id, source);
    assert_eq!(first_key.zone_change_generation, source_generation);
    assert_ne!(first_key.ability_origin, second_key.ability_origin);
    assert_ne!(first_prompt_id, second_prompt_id);

    let before_stale_answer = engine
        .diagnostic_snapshot()
        .expect("second occurrence snapshot");
    assert!(engine
        .apply_scenario_command(0, &decline_with_prompt(first_prompt_id.get()))
        .is_err());
    assert_eq!(
        engine
            .diagnostic_snapshot()
            .expect("unchanged second occurrence"),
        before_stale_answer,
        "the first child's stale answer cannot consume the second occurrence"
    );
    engine
        .apply_scenario_command(0, &decline_with_prompt(second_prompt_id.get()))
        .expect("answer the second occurrence independently");

    assert_eq!(engine.state.objects[&first_target].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&second_target].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.players[0].life, 22,
        "both child tails run once"
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.active_event_observers.is_empty());
}

#[test]
fn issue_241_zero_targets_creates_no_delayed_trigger_and_self_is_illegal() {
    let (mut zero, _) = setup(241_001);
    resolve_etb(&mut zero, Vec::new());
    assert!(zero.state.active_event_observers.is_empty());

    let (mut illegal, _) = setup(241_002);
    let source = koya_id(&illegal);
    assert!(illegal
        .apply_scenario_command(0, &choose_trigger_targets(target_object(source)))
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
        .apply_scenario_command(
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
        .apply_scenario_command(0, &select_payment_branch())
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
        .apply_scenario_command(0, &select_payment_branch())
        .expect("enter payment");
    engine
        .apply_scenario_command(0, &activate_ability(swamp, 0, Vec::new()))
        .expect("tap Swamp during payment");
    assert!(engine.state.objects[&swamp].tapped);
    assert_eq!(engine.state.players[0].mana_pool.black, 1);

    engine
        .apply_scenario_command(
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
        .apply_scenario_command(
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
        .apply_scenario_command(
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
            .apply_scenario_command(
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

#[test]
fn issue_241_return_program_resumes_after_clone_entry_choice() {
    let (mut engine, clone) = koya_return_tail_setup(241_050, false, "clone");
    remove_koya_source(&mut engine);
    let hill_giant = inject_creature_on_battlefield(&mut engine, 1, "hill_giant");
    let exiled_generation = engine
        .state
        .zone_change_generation
        .get(&clone)
        .copied()
        .unwrap_or(0);
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
        .apply_scenario_command(0, &select_payment_branch())
        .expect("select the payment fragment");
    engine
        .apply_scenario_command(
            0,
            &submit_resolution_decision(ResolutionChoiceDecision::Decline),
        )
        .expect("decline payment and enter the return branch");

    let copy_prompt = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("returning Clone reaches its actual entry-copy prompt");
    assert_eq!(
        copy_prompt.deciding_player, 1,
        "the owner chooses Clone's copy source"
    );
    assert_eq!(
        copy_prompt.presentation.choice_kind,
        tricerules_proto::ruled::v1::ChoiceKind::CopySource
    );
    assert!(
        copy_prompt.presentation.candidates.contains(&hill_giant),
        "the production copy-source menu offers the surviving Hill Giant"
    );
    engine
        .apply_scenario_command(1, &submit_resolution_choice(vec![hill_giant]))
        .expect("complete the production Clone entry choice");

    assert_eq!(engine.state.objects[&clone].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&clone].owner, 1);
    assert_eq!(
        engine.state.objects[&clone]
            .copiable_values
            .as_ref()
            .expect("returned Clone keeps its entry copy values")
            .source_card_id,
        "hill_giant"
    );
    assert_eq!(engine.effective_power(clone), Some(3));
    assert_eq!(engine.effective_toughness(clone), Some(3));
    assert_eq!(
        engine.state.zone_change_generation[&clone],
        exiled_generation + 1
    );
    assert_eq!(
        engine.state.players[0].life, 21,
        "the delayed child's tail resumes once"
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn issue_241_return_program_resumes_after_an_unreplaced_entry() {
    let (mut engine, target) = koya_return_tail_setup(241_053, false, "hill_giant");
    let initial_generation = engine.state.zone_change_generation[&target];
    park_delayed_choice(&mut engine);
    engine
        .apply_scenario_command(
            0,
            &submit_resolution_decision(ResolutionChoiceDecision::Decline),
        )
        .expect("decline into the exact-object return");

    assert_eq!(engine.state.objects[&target].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.zone_change_generation[&target],
        initial_generation + 1
    );
    assert_eq!(
        engine.state.players[0].life, 21,
        "the common tail runs once"
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn issue_241_departed_clone_owner_skips_entry_but_runs_tail() {
    let (mut engine, clone) = koya_return_tail_three_player_setup(241_056);
    advance_to_next_end_step(&mut engine);
    pass_priority_round(&mut engine);
    engine
        .apply_scenario_command(
            0,
            &submit_resolution_decision(ResolutionChoiceDecision::Decline),
        )
        .expect("decline into the exact-object return");

    let copy_choice = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the returned Clone asks its owner for a copy source");
    assert_eq!(copy_choice.deciding_player, 1);
    assert_eq!(
        copy_choice.presentation.choice_kind,
        tricerules_proto::ruled::v1::ChoiceKind::CopySource
    );
    assert!(copy_choice.presentation.candidates.iter().any(|id| {
        engine
            .state
            .objects
            .get(id)
            .is_some_and(|object| object.card_id == "hill_giant" && object.controller == 2)
    }));

    engine
        .apply_scenario_command(1, &concede())
        .expect("Clone owner leaves while its production entry choice is pending");

    assert!(engine.state.players[1].has_lost);
    assert!(!engine.state.objects.contains_key(&clone));
    assert_eq!(engine.state.players[0].life, 21);
    assert_eq!(engine.state.players[2].life, 20);
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn issue_241_stale_exact_return_skips_the_move_but_runs_the_tail() {
    let (mut engine, target) = koya_return_tail_setup(241_054, false, "hill_giant");
    let captured_generation = engine.state.zone_change_generation[&target];
    engine.state.players[1]
        .exile
        .retain(|object_id| *object_id != target);
    engine.state.players[1].hand.push(target);
    engine.state.objects.get_mut(&target).unwrap().zone = Zone::Hand;
    *engine
        .state
        .zone_change_generation
        .entry(target)
        .or_default() += 1;
    engine.state.players[1]
        .hand
        .retain(|object_id| *object_id != target);
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
        .apply_scenario_command(
            0,
            &submit_resolution_decision(ResolutionChoiceDecision::Decline),
        )
        .expect("stale captured object makes the return a no-op");

    assert_eq!(engine.state.objects[&target].zone, Zone::Exile);
    assert_eq!(
        engine.state.players[0].life, 21,
        "the following tail still runs once"
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn issue_241_ceased_exact_return_runs_the_program_tail() {
    let (mut engine, token) = koya_return_tail_setup(241_055, false, "soldier_w_1_1");
    assert!(
        !engine.state.objects.contains_key(&token),
        "the selected token ceases after the triggering exile"
    );
    assert_eq!(engine.state.active_event_observers.len(), 1);

    park_delayed_choice(&mut engine);
    engine
        .apply_scenario_command(
            0,
            &submit_resolution_decision(ResolutionChoiceDecision::Decline),
        )
        .expect("decline into the no-op exact return of the ceased token");

    assert!(!engine.state.objects.contains_key(&token));
    assert_eq!(
        engine.state.players[0].life, 21,
        "the program tail runs once after the ceased-reference no-op"
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn issue_241_missing_captured_reference_fails_before_any_program_instruction() {
    let (mut engine, target) = koya_return_tail_setup(241_051, true, "hill_giant");
    advance_to_next_end_step(&mut engine);
    let delayed = engine.state.stack.last_mut().expect("delayed ability");
    assert!(delayed.ability_program_owner().is_some());
    delayed.trigger_context.observed_object = None;
    let life_before_resolution = engine.state.players[0].life;

    let first = engine.state.priority_player_id();
    engine
        .apply_scenario_command(first, &pass())
        .expect("first player passes to the malformed delayed ability");
    let second = engine.state.priority_player_id();
    let command_index_before_resolution = engine.state.command_index;
    let failure = engine.apply_scenario_command(second, &pass());

    assert!(matches!(
        failure,
        Err(tricerules_core::EngineError::ProgramInvariant(
            "resolution program is missing its captured delayed-object reference"
        ))
    ));
    assert_eq!(engine.state.command_index, command_index_before_resolution);
    assert_eq!(engine.state.players[0].life, life_before_resolution);
    assert_eq!(engine.state.objects[&target].zone, Zone::Exile);
    assert_eq!(engine.state.stack.len(), 1);
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn issue_241_missing_captured_reference_is_revalidated_before_the_resume_tail() {
    let (mut engine, target) = koya_return_tail_setup(241_052, true, "hill_giant");
    park_delayed_choice(&mut engine);
    assert_eq!(engine.state.players[0].life, 21, "the first step ran once");
    let continuation = &mut engine
        .state
        .pending_resolution
        .as_mut()
        .expect("parked delayed choice")
        .continuation;
    let delayed = &mut continuation
        .stack_mut()
        .expect("parked delayed stack item")
        .item;
    assert!(delayed.ability_program_owner().is_some());
    delayed.trigger_context.observed_object = None;
    let before_invalid_resume = engine
        .diagnostic_snapshot()
        .expect("diagnostic pending program state");

    let failure = engine.apply_scenario_command(
        0,
        &submit_resolution_decision(ResolutionChoiceDecision::Decline),
    );
    assert!(matches!(
        failure,
        Err(tricerules_core::EngineError::ProgramInvariant(
            "resolution program is missing its captured delayed-object reference"
        ))
    ));
    assert_eq!(
        engine
            .diagnostic_snapshot()
            .expect("diagnostic state after rejected resume"),
        before_invalid_resume,
        "the failed resume does not run ReturnTriggeredCard or the following life-gain tail"
    );
    assert_eq!(engine.state.players[0].life, 21);
    assert_eq!(engine.state.objects[&target].zone, Zone::Exile);
}
