//! Mimic Vat's linked imprint and exact token-copy behavior.
use super::helpers::*;
use tricerules_cards::registry;
use tricerules_cards::{
    ContinuousEffectKind, EffectDuration, Keyword, PermanentTypeFilter, TypeLineAddition,
};
use tricerules_core::{AffectedScope, ContinuousEffect, EngineDeck, GameEngine, TurnStep, Zone};
use tricerules_proto::ruled::v1::{
    dev_command, ruled_command::Cmd, DevCommand, DevMoveCard, DevZone, ResolutionChoiceDecision,
    RuledCommand, SubmitResolutionChoice,
};

#[test]
fn creature_death_observer_retains_the_exact_event_object_for_imprint() {
    let mut engine = GameEngine::new(registry::global(), 2_026_101_001, &[0, 1], 20, None, true)
        .expect("new two-player game");
    advance_to_main1_from_game_start(&mut engine);
    inject_permanent_on_battlefield(&mut engine, 0, "blood_artist");
    let dying = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let battlefield_generation = engine
        .state
        .zone_change_generation
        .get(&dying)
        .copied()
        .unwrap_or(0);
    engine.state.objects.get_mut(&dying).unwrap().damage = 2;

    engine
        .apply_command(0, &pass())
        .expect("lethal damage is checked during priority processing");

    assert_eq!(engine.state.objects[&dying].zone, Zone::Graveyard);
    let trigger = engine
        .state
        .pending_triggers
        .iter()
        .find(|trigger| trigger.card_id == "blood_artist")
        .expect("Blood Artist observes the creature death");
    let observed = trigger
        .trigger_context
        .observed_object
        .expect("death observers retain the exact event object");
    assert_eq!(observed.object_id, dying);
    assert_eq!(observed.zone_change_generation, battlefield_generation);
    assert_eq!(
        engine.state.zone_change_generation[&dying],
        battlefield_generation + 1,
        "the graveyard incarnation is the next generation"
    );
}

const MIMIC_VAT: &str = "mimic_vat";

fn engine_with_vat(seed: u64) -> (GameEngine, u32) {
    engine_with_vat_and_opponent_cards(seed, &["savannah_lions"])
}

fn engine_with_vat_and_opponent_cards(seed: u64, opponent_cards: &[&str]) -> (GameEngine, u32) {
    let decks = Some(vec![
        deck_with("island", &[MIMIC_VAT, "grizzly_bears"]),
        deck_with("forest", opponent_cards),
    ]);
    let mut engine = GameEngine::new(registry::global(), seed, &[0, 1], 20, decks, true)
        .expect("new two-player game");
    advance_to_main1_from_game_start(&mut engine);
    let vat = move_ready_to_battlefield(&mut engine, 0, MIMIC_VAT);
    (engine, vat)
}

fn engine_with_commander_vat(seed: u64) -> (GameEngine, u32, u32) {
    let decks = Some(vec![
        EngineDeck {
            mainboard: deck_with("island", &[MIMIC_VAT, "grizzly_bears"]),
            commanders: Vec::new(),
        },
        EngineDeck {
            mainboard: deck_with("island", &[]),
            commanders: vec!["kokusho,_the_evening_star".to_string()],
        },
    ]);
    let mut engine =
        GameEngine::new_with_commander_decks(registry::global(), seed, &[0, 1], 20, decks, true)
            .expect("new Commander game with Kokusho");
    advance_to_main1_from_game_start(&mut engine);
    let vat = move_ready_to_battlefield(&mut engine, 0, MIMIC_VAT);
    let commander = engine.state.players[1].command_zone[0];
    let actor = engine.state.priority_player_id();
    move_card_to_zone(
        &mut engine,
        actor,
        1,
        "Kokusho, the Evening Star",
        DevZone::Battlefield,
    );
    (engine, vat, commander)
}

fn animate_as_two_two_creature(engine: &mut GameEngine, object_id: u32) {
    for kind in [
        ContinuousEffectKind::Layer4AddTypes(TypeLineAddition {
            card_types: vec![PermanentTypeFilter::Creature],
            ..Default::default()
        }),
        ContinuousEffectKind::Layer7bSetPt {
            power: 2,
            toughness: 2,
        },
    ] {
        engine.state.continuous_effects.push(ContinuousEffect {
            trigger_grant_origin: None,
            source_id: None,
            affected: AffectedScope::Single(object_id),
            kind,
            condition: None,
            duration: EffectDuration::Indefinite,
            timestamp: engine.state.command_index,
        });
    }
}

fn kill_creature(engine: &mut GameEngine, player: usize, card_id: &str, damage: u32) -> u32 {
    let object = move_ready_to_battlefield(engine, player, card_id);
    engine.state.objects.get_mut(&object).unwrap().damage = damage;
    let actor = engine.state.priority_player_id();
    engine
        .apply_command(actor, &pass())
        .expect("lethal damage is checked during priority processing");
    assert_eq!(engine.state.objects[&object].zone, Zone::Graveyard);
    object
}

fn pending_imprint_source_and_event(engine: &GameEngine) -> (u32, u32) {
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the imprint trigger awaits its resolution branch");
    let ResolutionContinuation::AuthoredBranch { stack, .. } = &pending.continuation else {
        panic!("Mimic Vat's imprint trigger uses its authored resolution branch");
    };
    (
        stack
            .item
            .source_permanent_id
            .expect("the triggered ability retains Mimic Vat's source object"),
        stack
            .item
            .trigger_context
            .observed_object
            .expect("the trigger retains the exact creature-death event object")
            .object_id,
    )
}

fn advance_until_imprint_choice(engine: &mut GameEngine) {
    for _ in 0..8 {
        if engine.state.pending_resolution.is_some() {
            return;
        }
        answer_simultaneous_entry_order_in_engine_order(engine);
        answer_trigger_order_in_engine_order(engine);
        pass_both_players(engine);
    }
    panic!("Mimic Vat's imprint trigger must reach its resolution branch");
}

fn resolve_imprint_choice(
    engine: &mut GameEngine,
    decision: ResolutionChoiceDecision,
) -> (u32, u32) {
    if engine.state.pending_resolution.is_none() {
        advance_until_imprint_choice(engine);
    }
    let (source, event_object) = pending_imprint_source_and_event(engine);
    let pending = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(
        pending.presentation.choice_kind,
        ChoiceKind::ResolutionBranch
    );
    let actor = pending.deciding_player;
    engine
        .apply_command(
            actor,
            &RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                    decision: decision as i32,
                    selected_branch_index: 0,
                    ..Default::default()
                })),
            },
        )
        .expect("answer Mimic Vat's resolution branch");
    (source, event_object)
}

fn move_card_to_zone(
    engine: &mut GameEngine,
    actor: i32,
    target_player: i32,
    card: &str,
    zone: DevZone,
) {
    engine.enable_dev_commands();
    engine
        .apply_command(
            actor,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: target_player,
                    dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                        card_name: card.into(),
                        zone: zone as i32,
                        ready: false,
                    })),
                })),
            },
        )
        .expect("move card through the development command");
}

fn choose_stack_trigger_target(object_id: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: Vec::new(),
            targets: vec![TargetRef {
                object_id,
                group_index: 0,
                kind: TargetRefKind::Stack as i32,
                ..Default::default()
            }],
        })),
    }
}

fn activate_vat_copy(engine: &mut GameEngine, vat: u32) {
    grant_pool(engine, 0);
    let command = activate_ability_for(engine, vat, 0, Vec::new());
    engine
        .apply_command(0, &command)
        .expect("activate Mimic Vat's copy ability");
    resolve_entire_stack_two_player(engine);
}

fn cast_metamorph_as_copy_of(engine: &mut GameEngine, player: usize, source: u32) -> u32 {
    let metamorph = inject_card_into_hand(engine, player, "phyrexian_metamorph");
    grant_pool(engine, player);
    let slot = hand_index_for_card(engine, player, "phyrexian_metamorph");
    let actor = engine.state.players[player].id;
    engine
        .apply_command(actor, &cast_spell(slot, Vec::new()))
        .expect("cast Phyrexian Metamorph");
    pass_both_players(engine);
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Metamorph asks for its as-enters copy source");
    assert_eq!(pending.presentation.choice_kind, ChoiceKind::CopySource);
    assert!(pending.presentation.candidates.contains(&source));
    engine
        .apply_command(actor, &submit_resolution_choice(vec![source]))
        .expect("choose the copy source");
    assert!(engine.state.players[player]
        .battlefield
        .contains(&metamorph));
    metamorph
}

fn advance_to_end_step(engine: &mut GameEngine) {
    engine
        .apply_command(0, &primitive_yield())
        .expect("main one to combat");
    engine
        .apply_command(0, &primitive_yield())
        .expect("begin combat");
    if engine.state.turn_step == TurnStep::DeclareAttackers {
        engine
            .apply_command(0, &primitive_yield())
            .expect("skip attackers");
    }
    engine
        .apply_command(0, &primitive_yield())
        .expect("end combat to main two");
    engine
        .apply_command(0, &primitive_yield())
        .expect("main two to end step");
    assert_eq!(engine.state.turn_step, TurnStep::EndStep);
}

fn pass_until_end_step(engine: &mut GameEngine) {
    for _ in 0..8 {
        if engine.state.turn_step == TurnStep::EndStep {
            return;
        }
        pass_priority_round(engine);
    }
    panic!("the active turn reaches its end step");
}

fn advance_until_next_turn_end_step(engine: &mut GameEngine, starting_turn_instance: u64) {
    for _ in 0..80 {
        if engine.state.turn_instance > starting_turn_instance
            && engine.state.turn_step == TurnStep::EndStep
        {
            return;
        }
        let actor = engine.state.priority_player_id();
        let command = if engine.state.cleanup_discard_player == Some(actor) {
            let index = engine.state.player_idx(actor).unwrap();
            discard_cleanup((engine.state.players[index].hand.len() - 1) as u32)
        } else {
            pass()
        };
        engine
            .apply_command(actor, &command)
            .expect("pass through cleanup and the following turn");
        answer_simultaneous_entry_order_in_engine_order(engine);
        answer_trigger_order_in_engine_order(engine);
    }
    panic!("the next turn reaches its end step");
}

#[test]
fn mimic_vat_is_registered_with_the_expected_oracle_identity() {
    let card = registry::global()
        .get(MIMIC_VAT)
        .expect("Mimic Vat must have a complete rules definition");

    assert_eq!(card.id, MIMIC_VAT);
    assert_eq!(card.name, "Mimic Vat");
}

#[test]
fn mimic_vat_imprints_nontoken_deaths_from_any_player_and_replaces_the_previous_card() {
    let (mut engine, vat) = engine_with_vat(2_026_101_002);
    let first = kill_creature(&mut engine, 1, "savannah_lions", 1);
    resolve_imprint_choice(&mut engine, ResolutionChoiceDecision::SelectBranch);
    assert_eq!(engine.state.objects[&first].zone, Zone::Exile);

    let second = kill_creature(&mut engine, 0, "grizzly_bears", 2);
    resolve_imprint_choice(&mut engine, ResolutionChoiceDecision::SelectBranch);
    assert_eq!(engine.state.objects[&first].zone, Zone::Graveyard);
    assert!(engine.state.players[1].graveyard.contains(&first));
    assert_eq!(engine.state.objects[&second].zone, Zone::Exile);

    let before = engine.state.players[0].battlefield.clone();
    activate_vat_copy(&mut engine, vat);
    let token = engine.state.players[0]
        .battlefield
        .iter()
        .copied()
        .find(|object| {
            !before.contains(object)
                && engine.state.objects[object].card_id == "grizzly_bears"
                && engine.state.objects[object].token_origin.is_some()
        })
        .expect("the current linked card is copied");
    assert!(engine.effective_has_keyword(token, tricerules_cards::Keyword::Haste));
}

#[test]
fn queued_activation_uses_the_current_link_after_reimprint_and_vat_leaves() {
    let (mut engine, vat) = engine_with_vat(2_026_101_018);
    let first = kill_creature(&mut engine, 1, "savannah_lions", 1);
    resolve_imprint_choice(&mut engine, ResolutionChoiceDecision::SelectBranch);
    assert_eq!(engine.state.objects[&first].zone, Zone::Exile);

    grant_pool(&mut engine, 0);
    let activation = activate_ability_for(&engine, vat, 0, Vec::new());
    engine
        .apply_command(0, &activation)
        .expect("activate Mimic Vat while Savannah Lions is linked");
    assert!(engine.state.stack.iter().any(|item| {
        item.source_permanent_id == Some(vat) && !item.is_triggered && item.card_id == MIMIC_VAT
    }));

    let second = move_ready_to_battlefield(&mut engine, 0, "grizzly_bears");
    engine.state.objects.get_mut(&second).unwrap().damage = 2;
    let actor = engine.state.priority_player_id();
    engine
        .apply_command(actor, &pass())
        .expect("lethal damage creates a death trigger above the queued activation");
    assert_eq!(engine.state.objects[&second].zone, Zone::Graveyard);

    let (source, imprinted_event) =
        resolve_imprint_choice(&mut engine, ResolutionChoiceDecision::SelectBranch);
    assert_eq!(source, vat);
    assert_eq!(imprinted_event, second);
    assert_eq!(engine.state.objects[&first].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&second].zone, Zone::Exile);
    assert!(engine.state.stack.iter().any(|item| {
        item.source_permanent_id == Some(vat) && !item.is_triggered && item.card_id == MIMIC_VAT
    }));

    let actor = engine.state.priority_player_id();
    move_card_to_zone(&mut engine, actor, 0, "Mimic Vat", DevZone::Graveyard);
    assert_eq!(engine.state.objects[&vat].zone, Zone::Graveyard);

    resolve_entire_stack_two_player(&mut engine);
    let token = engine.state.players[0]
        .battlefield
        .iter()
        .copied()
        .find(|object| {
            engine.state.objects[object].card_id == "grizzly_bears"
                && engine.state.objects[object].token_origin.is_some()
        })
        .expect("the queued activation copies the currently linked card after Vat leaves");
    assert!(engine.effective_has_keyword(token, Keyword::Haste));
}

#[test]
fn declining_a_new_imprint_preserves_the_previous_linked_card() {
    let (mut engine, _) = engine_with_vat(2_026_101_003);
    let first = kill_creature(&mut engine, 1, "savannah_lions", 1);
    resolve_imprint_choice(&mut engine, ResolutionChoiceDecision::SelectBranch);
    assert_eq!(engine.state.objects[&first].zone, Zone::Exile);

    let second = kill_creature(&mut engine, 0, "grizzly_bears", 2);
    resolve_imprint_choice(&mut engine, ResolutionChoiceDecision::Decline);
    assert_eq!(engine.state.objects[&first].zone, Zone::Exile);
    assert_eq!(engine.state.objects[&second].zone, Zone::Graveyard);
}

#[test]
fn a_departed_death_event_card_cannot_be_imprinted_when_its_trigger_resolves() {
    let (mut engine, _) = engine_with_vat(2_026_101_004);
    let first = kill_creature(&mut engine, 1, "savannah_lions", 1);
    assert_eq!(
        engine.state.objects[&first].zone,
        Zone::Graveyard,
        "the event card is in its graveyard while the trigger waits"
    );
    let actor = engine.state.priority_player_id();
    move_card_to_zone(&mut engine, actor, 1, "Savannah Lions", DevZone::Hand);
    resolve_entire_stack_two_player(&mut engine);
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.objects[&first].zone, Zone::Hand);
    assert!(!engine.state.players[0].exile.contains(&first));
}

#[test]
fn commander_death_triggers_vat_before_its_state_based_move_to_command() {
    let (mut engine, vat, commander) = engine_with_commander_vat(2_026_101_015);
    engine.state.objects.get_mut(&commander).unwrap().damage = 5;
    let actor = engine.state.priority_player_id();
    engine
        .apply_command(actor, &pass())
        .expect("lethal damage puts Kokusho into its owner's graveyard");

    assert_eq!(engine.state.objects[&commander].zone, Zone::Graveyard);
    let observed_death = engine
        .state
        .staged_trigger_groups
        .iter()
        .flat_map(|group| &group.triggers)
        .find(|trigger| trigger.card_id == MIMIC_VAT)
        .map(|trigger| trigger.trigger_context.observed_object)
        .or_else(|| {
            engine
                .state
                .pending_trigger_order
                .as_ref()
                .and_then(|order| {
                    order
                        .candidates
                        .iter()
                        .find(|trigger| trigger.card_id == MIMIC_VAT)
                })
                .map(|trigger| trigger.trigger_context.observed_object)
        })
        .or_else(|| {
            engine
                .state
                .pending_triggers
                .iter()
                .find(|trigger| trigger.card_id == MIMIC_VAT)
                .map(|trigger| trigger.trigger_context.observed_object)
        })
        .or_else(|| {
            engine
                .state
                .stack
                .iter()
                .find(|item| item.card_id == MIMIC_VAT && item.is_triggered)
                .map(|item| item.trigger_context.observed_object)
        })
        .expect("Mimic Vat triggers when the commander dies");
    assert_eq!(
        observed_death.map(|object| object.object_id),
        Some(commander),
        "the trigger retains the exact commander death event"
    );

    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the commander state-based action offers its owner a command-zone choice");
    assert_eq!(pending.deciding_player, 1);
    assert!(pending.presentation.prompt.contains("command zone"));
    engine
        .apply_command(
            1,
            &RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                    decision: ResolutionChoiceDecision::SelectBranch as i32,
                    selected_branch_index: 0,
                    ..Default::default()
                })),
            },
        )
        .expect("move the dead commander to the command zone as a state-based action");
    assert_eq!(engine.state.objects[&commander].zone, Zone::Command);

    for _ in 0..8 {
        answer_simultaneous_entry_order_in_engine_order(&mut engine);
        answer_trigger_order_in_engine_order(&mut engine);
        assert!(
            engine.state.pending_resolution.is_none(),
            "the departed commander cannot be chosen for Mimic Vat's exile branch"
        );
        if engine.state.stack.is_empty() && engine.state.pending_triggers.is_empty() {
            break;
        }
        pass_both_players(&mut engine);
    }
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_triggers.is_empty());

    let battlefield_before_activation = engine.state.players[0].battlefield.clone();
    activate_vat_copy(&mut engine, vat);
    assert_eq!(
        engine.state.players[0].battlefield, battlefield_before_activation,
        "no card remained linked after the commander moved from its graveyard"
    );
}

#[test]
fn manifested_instant_can_be_imprinted_but_cannot_be_copied() {
    let (mut engine, vat) = engine_with_vat(2_026_101_016);
    let instant = inject_library_card(&mut engine, 0, "lightning_bolt");
    let second = inject_library_card(&mut engine, 0, "forest");
    engine.state.players[0]
        .library
        .retain(|object_id| *object_id != instant && *object_id != second);
    engine.state.players[0].library.push_front(second);
    engine.state.players[0].library.push_front(instant);
    inject_card_into_hand(&mut engine, 0, "manifest_dread");
    grant_pool(&mut engine, 0);
    let slot = hand_index_for_card(&engine, 0, "manifest_dread");
    engine
        .apply_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast Manifest Dread");
    pass_both_players(&mut engine);
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Manifest Dread asks which top card to manifest");
    assert_eq!(pending.presentation.choice_kind, ChoiceKind::ManifestDread);
    assert!(pending.presentation.candidates.contains(&instant));
    engine
        .apply_command(
            pending.deciding_player,
            &submit_resolution_choice(vec![instant]),
        )
        .expect("manifest Lightning Bolt face down");
    assert_eq!(engine.state.objects[&instant].zone, Zone::Battlefield);
    assert!(engine.state.objects[&instant].face_down);
    assert!(engine
        .characteristics(instant)
        .unwrap()
        .has_type("Creature"));

    engine.state.objects.get_mut(&instant).unwrap().damage = 2;
    let actor = engine.state.priority_player_id();
    engine
        .apply_command(actor, &pass())
        .expect("lethal damage puts the manifested card into its owner's graveyard");
    assert_eq!(engine.state.objects[&instant].zone, Zone::Graveyard);
    resolve_imprint_choice(&mut engine, ResolutionChoiceDecision::SelectBranch);
    assert_eq!(engine.state.objects[&instant].zone, Zone::Exile);
    assert!(!registry::global()
        .get("lightning_bolt")
        .unwrap()
        .primary_face()
        .is_permanent());

    let battlefield_before_activation = engine.state.players[0].battlefield.clone();
    activate_vat_copy(&mut engine, vat);
    assert_eq!(
        engine.state.players[0].battlefield, battlefield_before_activation,
        "Mimic Vat cannot create a token copy of an imprinted instant"
    );
}

#[test]
fn animated_nontoken_artifact_can_be_imprinted_and_copied_as_a_noncreature() {
    let (mut engine, vat) = engine_with_vat(2_026_101_017);
    let artifact = inject_permanent_on_battlefield(&mut engine, 1, "howling_mine");
    animate_as_two_two_creature(&mut engine, artifact);
    let animated = engine.characteristics(artifact).expect("animated artifact");
    assert!(animated.is_artifact() && animated.is_creature());

    engine.state.objects.get_mut(&artifact).unwrap().damage = 2;
    engine
        .apply_command(engine.state.priority_player_id(), &pass())
        .expect("lethal damage puts the animated artifact into its owner's graveyard");
    assert_eq!(engine.state.objects[&artifact].zone, Zone::Graveyard);

    resolve_imprint_choice(&mut engine, ResolutionChoiceDecision::SelectBranch);
    assert_eq!(engine.state.objects[&artifact].zone, Zone::Exile);
    let printed_artifact = engine.characteristics(artifact).expect("linked artifact");
    assert!(printed_artifact.is_artifact());
    assert!(!printed_artifact.is_creature());

    let battlefield_before_activation = engine.state.players[0].battlefield.clone();
    activate_vat_copy(&mut engine, vat);
    let token = engine.state.players[0]
        .battlefield
        .iter()
        .copied()
        .find(|object| {
            !battlefield_before_activation.contains(object)
                && engine.state.objects[object].card_id == "howling_mine"
                && engine.state.objects[object].is_token()
        })
        .expect("Mimic Vat copies the linked noncreature artifact");
    let copied_artifact = engine.characteristics(token).expect("copied artifact");
    assert!(copied_artifact.is_artifact());
    assert!(!copied_artifact.is_creature());
    assert!(
        engine.effective_has_keyword(token, Keyword::Haste),
        "the noncreature artifact copy still has Mimic Vat's haste effect"
    );
}

#[test]
fn mimic_vat_ignores_token_creature_deaths() {
    let (mut engine, _) = engine_with_vat(2_026_101_005);
    let token = inject_creature_on_battlefield(&mut engine, 1, "soldier_w_1_1");
    assert!(engine.state.objects[&token].token_origin.is_some());
    engine.state.objects.get_mut(&token).unwrap().damage = 2;
    engine
        .apply_command(engine.state.priority_player_id(), &pass())
        .expect("lethal damage is checked during priority processing");
    assert!(
        !engine.state.objects.contains_key(&token),
        "tokens cease to exist after leaving the battlefield"
    );
    assert!(engine.state.pending_triggers.is_empty());
}

#[test]
fn simultaneous_deaths_keep_the_vat_pair_after_its_source_leaves() {
    let (mut engine, vat) = engine_with_vat(2_026_101_008);
    let first = move_ready_to_battlefield(&mut engine, 1, "savannah_lions");
    let second = move_ready_to_battlefield(&mut engine, 0, "grizzly_bears");
    engine.state.objects.get_mut(&first).unwrap().damage = 1;
    engine.state.objects.get_mut(&second).unwrap().damage = 2;
    engine
        .apply_command(engine.state.priority_player_id(), &pass())
        .expect("one state-based-action check destroys both creatures");
    assert_eq!(engine.state.objects[&first].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&second].zone, Zone::Graveyard);
    assert_eq!(
        engine
            .state
            .pending_trigger_order
            .as_ref()
            .map(|order| order.candidates.len()),
        Some(2),
        "the two same-controller triggers await CR 603.3b ordering"
    );
    answer_trigger_order_in_engine_order(&mut engine);
    let stacked_vat_triggers = engine
        .state
        .stack
        .iter()
        .filter(|item| item.is_triggered && item.source_permanent_id == Some(vat))
        .count();
    assert_eq!(
        stacked_vat_triggers, 2,
        "the Vat triggers once for each nontoken creature in the simultaneous event"
    );

    let actor = engine.state.priority_player_id();
    move_card_to_zone(&mut engine, actor, 0, "Mimic Vat", DevZone::Graveyard);
    assert_eq!(engine.state.objects[&vat].zone, Zone::Graveyard);

    let (source, imprinted_event) =
        resolve_imprint_choice(&mut engine, ResolutionChoiceDecision::SelectBranch);
    assert_eq!(source, vat, "the trigger retains the exact Vat incarnation");
    assert!([first, second].contains(&imprinted_event));
    assert_eq!(engine.state.objects[&imprinted_event].zone, Zone::Exile);

    let (_, declined_event) =
        resolve_imprint_choice(&mut engine, ResolutionChoiceDecision::Decline);
    assert_ne!(declined_event, imprinted_event);
    assert_eq!(engine.state.objects[&declined_event].zone, Zone::Graveyard);
}

#[test]
fn simultaneous_disk_sweep_keeps_vat_triggers_when_all_sources_leave_together() {
    let (mut engine, vat) = engine_with_vat(2_026_101_013);
    let disk = inject_permanent_on_battlefield(&mut engine, 0, "nevinyrrals_disk");
    let lion = move_ready_to_battlefield(&mut engine, 1, "savannah_lions");
    let bear = move_ready_to_battlefield(&mut engine, 0, "grizzly_bears");

    grant_pool(&mut engine, 0);
    let activation = activate_ability_for(&engine, disk, 0, vec![]);
    engine
        .apply_command(0, &activation)
        .expect("activate Nevinyrral's Disk");
    pass_both_players(&mut engine);

    for object in [disk, vat, lion, bear] {
        assert_eq!(
            engine.state.objects[&object].zone,
            Zone::Graveyard,
            "the Disk destroys itself, Mimic Vat, and both creatures together"
        );
    }
    assert_eq!(
        engine
            .state
            .pending_trigger_order
            .as_ref()
            .map(|order| order.candidates.len()),
        Some(2)
    );
    answer_trigger_order_in_engine_order(&mut engine);

    let (source, accepted_event) =
        resolve_imprint_choice(&mut engine, ResolutionChoiceDecision::SelectBranch);
    assert_eq!(source, vat, "the dead Vat remains the trigger's source");
    assert!([lion, bear].contains(&accepted_event));
    assert_eq!(engine.state.objects[&accepted_event].zone, Zone::Exile);

    let (source, declined_event) =
        resolve_imprint_choice(&mut engine, ResolutionChoiceDecision::Decline);
    assert_eq!(source, vat);
    assert_ne!(accepted_event, declined_event);
    assert_eq!(engine.state.objects[&declined_event].zone, Zone::Graveyard);
}

#[test]
fn an_empty_vat_creates_no_token_and_a_real_copy_exiles_at_the_next_end_step() {
    let (mut empty, empty_vat) = engine_with_vat(2_026_101_006);
    activate_vat_copy(&mut empty, empty_vat);
    assert!(empty.state.players[0]
        .battlefield
        .iter()
        .all(|object| empty.state.objects[object].token_origin.is_none()));

    let (mut engine, vat) = engine_with_vat(2_026_101_007);
    let creature = kill_creature(&mut engine, 1, "savannah_lions", 1);
    resolve_imprint_choice(&mut engine, ResolutionChoiceDecision::SelectBranch);
    assert_eq!(engine.state.objects[&creature].zone, Zone::Exile);
    activate_vat_copy(&mut engine, vat);
    let token = engine.state.players[0]
        .battlefield
        .iter()
        .copied()
        .find(|object| {
            engine.state.objects[object].card_id == "savannah_lions"
                && engine.state.objects[object].token_origin.is_some()
        })
        .expect("copy of the linked card enters the battlefield");
    assert!(engine.effective_has_keyword(token, tricerules_cards::Keyword::Haste));

    advance_to_end_step(&mut engine);
    assert_eq!(engine.state.stack.len(), 1, "the delayed exile is staged");
    resolve_entire_stack_two_player(&mut engine);
    assert!(!engine.state.objects.contains_key(&token));
}

#[test]
fn activation_during_end_step_waits_for_the_following_turn_end_step() {
    let (mut engine, vat) = engine_with_vat(2_026_101_014);
    let creature = kill_creature(&mut engine, 1, "savannah_lions", 1);
    resolve_imprint_choice(&mut engine, ResolutionChoiceDecision::SelectBranch);
    assert_eq!(engine.state.objects[&creature].zone, Zone::Exile);

    advance_to_end_step(&mut engine);
    activate_vat_copy(&mut engine, vat);
    let token = engine.state.players[0]
        .battlefield
        .iter()
        .copied()
        .find(|object| {
            engine.state.objects[object].card_id == "savannah_lions"
                && engine.state.objects[object].is_token()
        })
        .expect("Mimic Vat creates its copy during the end step");
    assert!(engine.effective_has_keyword(token, tricerules_cards::Keyword::Haste));
    assert!(engine.state.stack.is_empty());

    let activation_turn = engine.state.turn_instance;
    advance_until_next_turn_end_step(&mut engine, activation_turn);
    assert_eq!(engine.state.turn_step, TurnStep::EndStep);
    assert_eq!(engine.state.stack.len(), 1);
    resolve_entire_stack_two_player(&mut engine);
    assert!(!engine.state.objects.contains_key(&token));
}

#[test]
fn linked_copy_entry_choice_resumes_before_haste_and_the_delayed_exile_are_added() {
    let (mut engine, vat) = engine_with_vat(2_026_101_009);
    let sorcerer = inject_creature_on_battlefield(&mut engine, 0, "prodigal_sorcerer");
    let metamorph = cast_metamorph_as_copy_of(&mut engine, 0, sorcerer);
    assert_eq!(
        engine.characteristics(metamorph).unwrap().names,
        ["Prodigal Sorcerer"]
    );

    engine.state.objects.get_mut(&metamorph).unwrap().damage = 1;
    engine
        .apply_command(engine.state.priority_player_id(), &pass())
        .expect("Metamorph dies");
    assert_eq!(engine.state.objects[&metamorph].zone, Zone::Graveyard);
    resolve_imprint_choice(&mut engine, ResolutionChoiceDecision::SelectBranch);
    assert_eq!(engine.state.objects[&metamorph].zone, Zone::Exile);

    activate_vat_copy(&mut engine, vat);
    let entry_choice = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the linked Metamorph token parks for its as-enters choice");
    assert_eq!(
        entry_choice.presentation.choice_kind,
        ChoiceKind::CopySource
    );
    assert!(entry_choice.presentation.candidates.contains(&sorcerer));
    engine
        .apply_command(0, &submit_resolution_choice(vec![sorcerer]))
        .expect("finish the token's as-enters copy choice");

    let token = engine.state.players[0]
        .battlefield
        .iter()
        .copied()
        .find(|object| {
            engine.state.objects[object]
                .token_origin
                .as_ref()
                .is_some_and(|values| values.source_card_id == "phyrexian_metamorph")
        })
        .expect("the token entered after its copy choice");
    assert_eq!(
        engine.state.objects[&token]
            .copiable_values
            .as_ref()
            .map(|values| values.source_card_id.as_str()),
        Some("prodigal_sorcerer")
    );
    assert!(engine.effective_has_keyword(token, tricerules_cards::Keyword::Haste));
    assert!(engine.state.active_event_observers.iter().any(|observer| {
        observer.watched.is_some_and(|watched| {
            watched.object_id == token
                && watched.zone_change_generation == engine.state.zone_change_generation[&token]
        })
    }));
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn doubling_season_haste_and_one_delayed_exile_cover_the_complete_token_cohort() {
    let (mut engine, vat) = engine_with_vat(2_026_101_010);
    inject_permanent_on_battlefield(&mut engine, 0, "doubling_season");
    let creature = kill_creature(&mut engine, 1, "savannah_lions", 1);
    resolve_imprint_choice(&mut engine, ResolutionChoiceDecision::SelectBranch);
    assert_eq!(engine.state.objects[&creature].zone, Zone::Exile);

    let before = engine.state.players[0].battlefield.clone();
    activate_vat_copy(&mut engine, vat);
    answer_simultaneous_entry_order_in_engine_order(&mut engine);
    let tokens = engine.state.players[0]
        .battlefield
        .iter()
        .copied()
        .filter(|object| {
            !before.contains(object)
                && engine.state.objects[object].card_id == "savannah_lions"
                && engine.state.objects[object].is_token()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        tokens.len(),
        2,
        "Doubling Season replaces one token with two"
    );
    for token in &tokens {
        assert!(engine.effective_has_keyword(*token, tricerules_cards::Keyword::Haste));
    }
    assert_eq!(
        engine.state.observed_object_cohorts.len(),
        1,
        "one delayed ability retains one exact cohort for this creation instruction"
    );

    let departed = tokens[0];
    let surviving = tokens[1];
    engine.state.objects.get_mut(&departed).unwrap().damage = 1;
    engine
        .apply_command(engine.state.priority_player_id(), &pass())
        .expect("one token leaves before the end step");
    assert!(!engine.state.objects.contains_key(&departed));
    assert_eq!(engine.state.objects[&surviving].zone, Zone::Battlefield);
    assert_eq!(engine.state.observed_object_cohorts.len(), 1);

    pass_until_end_step(&mut engine);
    assert_eq!(
        engine.state.stack.len(),
        1,
        "the cohort delay triggers once"
    );
    resolve_entire_stack_two_player(&mut engine);
    assert!(!engine.state.objects.contains_key(&surviving));
    assert!(engine.state.observed_object_cohorts.is_empty());
}

#[test]
fn countering_the_cohort_exile_keeps_both_hasty_tokens_and_releases_the_cohort() {
    let (mut engine, vat) = engine_with_vat(2_026_101_011);
    inject_permanent_on_battlefield(&mut engine, 0, "doubling_season");
    let creature = kill_creature(&mut engine, 1, "savannah_lions", 1);
    resolve_imprint_choice(&mut engine, ResolutionChoiceDecision::SelectBranch);
    assert_eq!(engine.state.objects[&creature].zone, Zone::Exile);
    let before = engine.state.players[0].battlefield.clone();
    activate_vat_copy(&mut engine, vat);
    answer_simultaneous_entry_order_in_engine_order(&mut engine);
    let tokens = engine.state.players[0]
        .battlefield
        .iter()
        .copied()
        .filter(|object| {
            !before.contains(object)
                && engine.state.objects[object].card_id == "savannah_lions"
                && engine.state.objects[object].is_token()
        })
        .collect::<Vec<_>>();
    assert_eq!(tokens.len(), 2);
    assert_eq!(engine.state.observed_object_cohorts.len(), 1);

    pass_until_end_step(&mut engine);
    let delayed_exile = engine.state.stack.last().expect("one cohort trigger").id;
    assert_eq!(engine.state.stack.len(), 1);
    engine
        .apply_command(0, &pass())
        .expect("pass priority to the opponent");
    let tidebinder = inject_card_into_hand(&mut engine, 1, "tishanas_tidebinder");
    grant_pool(&mut engine, 1);
    let slot = hand_index_for_card(&engine, 1, "tishanas_tidebinder");
    engine
        .apply_command(1, &cast_spell(slot, Vec::new()))
        .expect("cast Tidebinder in response to the cohort trigger");
    pass_both_players(&mut engine);
    assert_eq!(engine.state.pending_triggers.len(), 1);
    engine
        .apply_command(1, &choose_stack_trigger_target(delayed_exile))
        .expect("Tidebinder targets the delayed exile ability");
    pass_both_players(&mut engine);

    assert!(engine
        .state
        .stack
        .iter()
        .all(|item| item.id != delayed_exile));
    assert_eq!(engine.state.objects[&tidebinder].zone, Zone::Battlefield);
    for token in tokens {
        assert_eq!(engine.state.objects[&token].zone, Zone::Battlefield);
        assert!(engine.state.players[0].battlefield.contains(&token));
        assert!(engine.effective_has_keyword(token, tricerules_cards::Keyword::Haste));
    }
    assert!(engine.state.observed_object_cohorts.is_empty());
}

#[test]
fn a_copied_vat_uses_its_own_linked_exile_occurrence() {
    let (mut engine, original_vat) =
        engine_with_vat_and_opponent_cards(2_026_101_012, &["savannah_lions", "savannah_lions"]);
    let first_link = kill_creature(&mut engine, 1, "savannah_lions", 1);
    resolve_imprint_choice(&mut engine, ResolutionChoiceDecision::SelectBranch);
    assert_eq!(engine.state.objects[&first_link].zone, Zone::Exile);

    let copy = cast_metamorph_as_copy_of(&mut engine, 0, original_vat);
    assert!(engine.characteristics(copy).unwrap().has_name("Mimic Vat"));

    let lion = move_ready_to_battlefield(&mut engine, 1, "savannah_lions");
    let bear = move_ready_to_battlefield(&mut engine, 0, "grizzly_bears");
    engine.state.objects.get_mut(&lion).unwrap().damage = 1;
    engine.state.objects.get_mut(&bear).unwrap().damage = 2;
    engine
        .apply_command(engine.state.priority_player_id(), &pass())
        .expect("both lethal creatures die in the same state-based-action event");
    assert_eq!(engine.state.objects[&lion].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&bear].zone, Zone::Graveyard);

    let trigger_order = engine
        .state
        .pending_trigger_order
        .as_ref()
        .expect("the two Vat occurrences order their four simultaneous triggers");
    assert_eq!(trigger_order.candidates.len(), 4);
    let trigger_id = |source, event| {
        trigger_order
            .candidates
            .iter()
            .find(|trigger| {
                trigger.source_permanent_id == source
                    && trigger
                        .trigger_context
                        .observed_object
                        .is_some_and(|observed| observed.object_id == event)
            })
            .expect("the selected Vat observed this exact creature's death")
            .object_id
    };
    let order = [
        trigger_id(copy, lion),
        trigger_id(original_vat, bear),
        trigger_id(original_vat, lion),
    ];
    for object_id in order {
        let deciding_player = engine
            .state
            .pending_trigger_order
            .as_ref()
            .expect("the remaining simultaneous triggers are ordered")
            .deciding_player;
        engine
            .apply_command(deciding_player, &submit_trigger_order(object_id))
            .expect("put the selected trigger on the stack");
    }

    advance_until_imprint_choice(&mut engine);
    assert_eq!(
        pending_imprint_source_and_event(&engine),
        (copy, bear),
        "the copied Vat resolves the Bear event first"
    );
    assert_eq!(
        resolve_imprint_choice(&mut engine, ResolutionChoiceDecision::SelectBranch),
        (copy, bear)
    );
    assert_eq!(engine.state.objects[&bear].zone, Zone::Exile);

    advance_until_imprint_choice(&mut engine);
    assert_eq!(
        pending_imprint_source_and_event(&engine),
        (original_vat, lion),
        "the original Vat then imprints Lions independently"
    );
    assert_eq!(
        resolve_imprint_choice(&mut engine, ResolutionChoiceDecision::SelectBranch),
        (original_vat, lion)
    );
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.objects[&first_link].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&lion].zone, Zone::Exile);
    assert_eq!(engine.state.objects[&bear].zone, Zone::Exile);

    let before = engine.state.players[0].battlefield.clone();
    activate_vat_copy(&mut engine, original_vat);
    activate_vat_copy(&mut engine, copy);
    let token_cards = engine.state.players[0]
        .battlefield
        .iter()
        .filter(|object| !before.contains(object) && engine.state.objects[object].is_token())
        .map(|object| engine.state.objects[object].card_id.clone())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        token_cards,
        ["grizzly_bears".to_string(), "savannah_lions".to_string()]
            .into_iter()
            .collect()
    );
}
