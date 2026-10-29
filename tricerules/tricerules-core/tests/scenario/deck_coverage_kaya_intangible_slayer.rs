//! Actual-card coverage for Kaya, Intangible Slayer.

use super::helpers::*;
use crate::helpers::ScenarioCommandExt;
use tricerules_cards::primitives::{AbilityCost, CounterKind, Keyword};
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, ruled_event::Ev, ChoiceKind, DevCommand, DevMoveCard, DevZone,
    ResolutionChoiceDecision, RuledCommand, RuledEventBatch, SubmitResolutionChoice, TargetRef,
    TargetRefKind,
};

const KAYA: &str = "kaya,_intangible_slayer";

fn setup(players: &[i32]) -> GameEngine {
    let decks = (0..players.len())
        .map(|seat| {
            if seat == 0 {
                deck_with("swamp", &[KAYA])
            } else {
                deck_with("forest", &[])
            }
        })
        .collect();
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        281_028,
        players,
        30,
        Some(decks),
        true,
    )
    .expect("registered Kaya scenario engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn put_kaya(engine: &mut GameEngine, seat: usize, loyalty: u32) -> u32 {
    let source = inject_permanent_on_battlefield(engine, seat, KAYA);
    engine
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .set_counter(CounterKind::Loyalty, loyalty);
    engine.initial_response_batch();
    source
}

fn ability_index(ability_id: &str) -> u32 {
    tricerules_cards::registry::global()
        .get(KAYA)
        .expect("Kaya is registered")
        .primary_face()
        .activated_abilities
        .iter()
        .position(|ability| ability.ability_id.as_str() == ability_id)
        .unwrap_or_else(|| panic!("Kaya ability {ability_id} is present")) as u32
}

fn permanent_target(engine: &GameEngine, object_id: u32) -> TargetRef {
    TargetRef {
        object_id,
        kind: TargetRefKind::Permanent as i32,
        expected_zone_change_generation: engine
            .state
            .zone_change_generation
            .get(&object_id)
            .copied(),
        ..Default::default()
    }
}

fn activate_kaya(
    engine: &mut GameEngine,
    actor: i32,
    source: u32,
    ability_id: &str,
    targets: Vec<TargetRef>,
) -> Result<RuledEventBatch, tricerules_core::EngineError> {
    apply_ability(engine, actor, source, ability_index(ability_id), targets)
}

fn resolve_until_idle(engine: &mut GameEngine) {
    for _ in 0..64 {
        if let Some(pending) = engine.state.pending_resolution.as_ref() {
            assert_ne!(
                pending.presentation.choice_kind,
                ChoiceKind::ResolutionBranch,
                "this Kaya scenario must not skip an optional opponent choice"
            );
            let actor = pending.deciding_player;
            let candidates = pending.presentation.candidates.clone();
            engine
                .apply_scenario_command(actor, &submit_resolution_choice(candidates))
                .expect("complete the engine-offered resolution ordering choice");
            continue;
        }
        if engine.state.stack.is_empty() && engine.state.pending_triggers.is_empty() {
            return;
        }
        pass_priority_round(engine);
    }
    panic!(
        "Kaya scenario did not finish: stack={}, pending={:?}",
        engine.state.stack.len(),
        engine.state.pending_resolution
    );
}

fn answer_scry_branch(engine: &mut GameEngine, actor: i32, branch_index: u32) -> RuledEventBatch {
    let command = RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            decision: ResolutionChoiceDecision::SelectBranch as i32,
            selected_branch_index: branch_index,
            ..Default::default()
        })),
    };
    engine
        .apply_scenario_command(actor, &command)
        .expect("answer the current Kaya Scry 1 opt-in")
}

fn private_look_viewers(batch: &RuledEventBatch) -> Vec<Option<i32>> {
    batch
        .events
        .iter()
        .filter_map(|event| match event.ev.as_ref() {
            Some(Ev::Log(log)) if log.private_card_look.is_some() => Some(log.visible_to_player_id),
            _ => None,
        })
        .collect()
}

fn battlefield_token_ids(engine: &GameEngine) -> Vec<u32> {
    engine
        .state
        .objects
        .iter()
        .filter_map(|(object_id, object)| {
            (object.is_token() && object.zone == Zone::Battlefield).then_some(*object_id)
        })
        .collect()
}

fn dev_move(player_id: i32, zone: DevZone, card_name: &str) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::DevCommand(DevCommand {
            target_player_id: player_id,
            dev: Some(tricerules_proto::ruled::v1::dev_command::Dev::MoveCard(
                DevMoveCard {
                    card_name: card_name.to_string(),
                    zone: zone as i32,
                    ready: false,
                },
            )),
        })),
    }
}

#[test]
fn kaya_intangible_slayer_has_exact_identity_and_three_abilities() {
    let card = tricerules_cards::registry::global()
        .get(KAYA)
        .expect("Kaya, Intangible Slayer must have a complete ruled definition");
    assert_eq!(card.name, "Kaya, Intangible Slayer");
    assert_eq!(card.face_count(), 1);

    let face = card.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{3}{W}{W}{B}{B}");
    assert_eq!(face.supertypes, ["Legendary"]);
    assert_eq!(face.types, ["Planeswalker", "Kaya"]);
    assert_eq!(face.loyalty, Some(6));
    assert_eq!(face.keywords, [Keyword::Hexproof]);
    assert_eq!(face.activated_abilities.len(), 3);
    let loyalty_costs = face
        .activated_abilities
        .iter()
        .map(|ability| match ability.costs.as_slice() {
            [AbilityCost::Loyalty(amount)] => *amount,
            costs => panic!("expected one loyalty cost, found {costs:?}"),
        })
        .collect::<Vec<_>>();
    assert_eq!(loyalty_costs, [2, 0, -3]);
    assert_eq!(face.activated_abilities[0].effect.len(), 2);
    assert_eq!(face.activated_abilities[1].effect.len(), 2);
    assert_eq!(face.activated_abilities[2].effect.len(), 1);

    let target_groups = &face.activated_abilities[2]
        .targeting
        .as_ref()
        .expect("the -3 has target legality")
        .groups;
    assert_eq!(target_groups.len(), 1);
    assert_eq!(target_groups[0].min, 1);
    assert_eq!(target_groups[0].max, 1);
    assert_eq!(target_groups[0].effect_indices, [0]);
}

#[test]
fn kaya_plus_two_each_opponent_loses_three_and_controller_gains_three_once() {
    let mut engine = setup(&[10, 20, 30]);
    let source = put_kaya(&mut engine, 0, 6);
    let before = engine
        .state
        .players
        .iter()
        .map(|player| player.life)
        .collect::<Vec<_>>();

    activate_kaya(&mut engine, 10, source, "activated_01", vec![]).expect("activate Kaya's +2");
    pass_priority_round(&mut engine);

    assert_eq!(engine.state.players[0].life, before[0] + 3);
    assert_eq!(engine.state.players[1].life, before[1] - 3);
    assert_eq!(engine.state.players[2].life, before[2] - 3);
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Loyalty),
        8
    );
}

#[test]
fn kaya_zero_draws_before_ordered_optional_scry_choices_and_defers_library_moves() {
    let mut engine = setup(&[10, 20, 30, 40]);
    let source = put_kaya(&mut engine, 0, 6);
    let hand_before = engine.state.players[0].hand.len();
    let library_before = engine.state.players[0].library.len();

    activate_kaya(&mut engine, 10, source, "activated_02", vec![])
        .expect("activate Kaya's zero ability");
    pass_priority_round(&mut engine);

    assert_eq!(engine.state.players[0].hand.len(), hand_before + 2);
    assert_eq!(engine.state.players[0].library.len(), library_before - 2);
    let first_opt_in = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("first opponent receives the opt-in after the draw")
        .presentation
        .clone();
    assert_eq!(first_opt_in.choice_kind, ChoiceKind::ResolutionBranch);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .deciding_player,
        20
    );

    let p20_library_before = engine.state.players[1].library.clone();
    let p40_library_before = engine.state.players[3].library.clone();
    let mut opt_in_batch = RuledEventBatch::default();
    for (actor, branch_index) in [(20, 0), (30, 1), (40, 0)] {
        let pending = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("each opponent receives an ordered Scry 1 choice");
        assert_eq!(pending.deciding_player, actor);
        assert_eq!(
            pending.presentation.choice_kind,
            ChoiceKind::ResolutionBranch
        );
        assert!(
            branch_index <= 1,
            "only the current two opt-in choices are used"
        );
        opt_in_batch = answer_scry_branch(&mut engine, actor, branch_index);
    }

    assert_eq!(private_look_viewers(&opt_in_batch), [Some(20), Some(40)]);
    let first_placement = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("accepted Scry 1 choices open a later placement stage");
    assert_eq!(first_placement.deciding_player, 20);
    assert_eq!(
        first_placement.presentation.choice_kind,
        ChoiceKind::LibraryTop
    );
    let first_top = *first_placement
        .presentation
        .candidates
        .first()
        .expect("Scry 1 publishes the current top card");
    assert_eq!(
        engine.state.players[1].library, p20_library_before,
        "the engine does not move a card when it publishes the placement stage"
    );

    let first_answer = engine
        .apply_scenario_command(20, &submit_resolution_choice(vec![first_top]))
        .expect("first accepter chooses to put the looked-at card on the bottom");
    assert_eq!(
        engine.state.players[1].library, p20_library_before,
        "the first accepted placement is deferred until every accepter chooses"
    );
    assert_eq!(engine.state.players[3].library, p40_library_before);
    assert!(first_answer
        .events
        .iter()
        .any(|event| matches!(event.ev, Some(Ev::ResolutionChoiceRequired(_)))));

    let second_placement = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the remaining accepter receives their placement prompt");
    assert_eq!(second_placement.deciding_player, 40);
    assert_eq!(
        second_placement.presentation.choice_kind,
        ChoiceKind::LibraryTop
    );
    let second_top = *second_placement
        .presentation
        .candidates
        .first()
        .expect("second accepter has one top card to scry");
    engine
        .apply_scenario_command(40, &submit_resolution_choice(vec![second_top]))
        .expect("last accepter completes the placement stage");
    assert!(engine.state.pending_resolution.is_none());
    assert_ne!(engine.state.players[1].library, p20_library_before);
    assert_ne!(engine.state.players[3].library, p40_library_before);
}

#[test]
fn kaya_hexproof_and_loyalty_timing_block_opponent_targets_and_wrong_phase() {
    let mut engine = setup(&[10, 20]);
    let source = put_kaya(&mut engine, 0, 6);
    let other_permanent = inject_permanent_on_battlefield(&mut engine, 1, "forest");
    let chaos_warp = inject_card_into_hand(&mut engine, 1, "chaos_warp");
    let opponent_hand_index = engine.state.players[1]
        .hand
        .iter()
        .position(|object_id| *object_id == chaos_warp)
        .expect("opponent's Chaos Warp is in hand");
    engine.initial_response_batch();
    let opponent_batch = engine
        .apply_scenario_command(10, &pass())
        .expect("active player passes priority to the opponent");
    let opponent_actions = opponent_batch
        .legal_by_player
        .get(&20)
        .expect("current opponent offers");
    let offered_targets = opponent_actions
        .valid_targets_by_hand_slot
        .get(&((opponent_hand_index as u32) << 8))
        .expect("Chaos Warp's current legal target group");
    let group = offered_targets.groups.first().expect("one target group");
    assert!(group.valid_permanent_ids.contains(&other_permanent));
    assert!(
        !group.valid_permanent_ids.contains(&source),
        "opponent cannot target Kaya through Hexproof"
    );

    assert!(activate_kaya(&mut engine, 20, source, "activated_01", vec![]).is_err());
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Loyalty),
        6,
        "an opponent cannot activate Kaya's loyalty abilities"
    );

    let mut wrong_phase = setup(&[10, 20]);
    let wrong_phase_source = put_kaya(&mut wrong_phase, 0, 6);
    pass_priority_round(&mut wrong_phase);
    assert_ne!(
        wrong_phase.state.turn_step,
        tricerules_core::TurnStep::Main1
    );
    assert!(activate_kaya(
        &mut wrong_phase,
        10,
        wrong_phase_source,
        "activated_01",
        vec![]
    )
    .is_err());
    assert_eq!(
        wrong_phase.state.objects[&wrong_phase_source].counter_count(CounterKind::Loyalty),
        6
    );

    let mut once_per_turn = setup(&[10, 20]);
    let once_source = put_kaya(&mut once_per_turn, 0, 6);
    activate_kaya(&mut once_per_turn, 10, once_source, "activated_01", vec![])
        .expect("first loyalty ability is legal in main phase");
    pass_priority_round(&mut once_per_turn);
    assert!(activate_kaya(&mut once_per_turn, 10, once_source, "activated_02", vec![]).is_err());
    assert_eq!(
        once_per_turn.state.objects[&once_source].counter_count(CounterKind::Loyalty),
        8,
        "rejecting a second loyalty activation preserves its state"
    );
}

#[test]
fn kaya_minus_three_copies_creature_exiles_source_and_triggers_entry_once() {
    let mut engine = setup(&[10, 20]);
    let source = put_kaya(&mut engine, 0, 3);
    let target = inject_creature_on_battlefield(&mut engine, 1, "elvish_visionary");
    let hand_before = engine.state.players[0].hand.len();
    let library_before = engine.state.players[0].library.len();
    let tokens_before = battlefield_token_ids(&engine);

    let target_ref = permanent_target(&engine, target);
    activate_kaya(&mut engine, 10, source, "activated_03", vec![target_ref])
        .expect("activate Kaya's -3 targeting Elvish Visionary");
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&target].zone, Zone::Battlefield);
    resolve_until_idle(&mut engine);

    assert_eq!(engine.state.objects[&target].zone, Zone::Exile);
    let new_tokens = battlefield_token_ids(&engine)
        .into_iter()
        .filter(|object_id| !tokens_before.contains(object_id))
        .collect::<Vec<_>>();
    assert_eq!(new_tokens.len(), 1);
    let token = new_tokens[0];
    assert_eq!(engine.state.objects[&token].owner, 10);
    assert_eq!(engine.state.objects[&token].controller, 10);
    let stats = engine
        .characteristics(token)
        .expect("copied token characteristics");
    assert_eq!(stats.colors, [tricerules_cards::primitives::Color::White]);
    assert_eq!((stats.power, stats.toughness), (Some(1), Some(1)));
    assert!(stats.has_type("Creature"));
    assert!(stats.has_type("Elf"));
    assert!(stats.has_type("Spirit"));
    assert!(engine.effective_has_keyword(token, Keyword::Flying));
    assert_eq!(engine.state.players[0].hand.len(), hand_before + 1);
    assert_eq!(engine.state.players[0].library.len(), library_before - 1);
}

#[test]
fn kaya_minus_three_copies_non_aura_enchantment_with_added_type() {
    let mut engine = setup(&[10, 20]);
    let source = put_kaya(&mut engine, 0, 6);
    let target = inject_permanent_on_battlefield(&mut engine, 1, "phyrexian_arena");
    let tokens_before = battlefield_token_ids(&engine);
    let target_stats = engine
        .characteristics(target)
        .expect("Arena characteristics");
    assert!(target_stats.has_type("Enchantment"));
    assert!(!target_stats.has_type("Aura"));

    let target_ref = permanent_target(&engine, target);
    activate_kaya(&mut engine, 10, source, "activated_03", vec![target_ref])
        .expect("activate Kaya's -3 targeting a non-Aura enchantment");
    pass_priority_round(&mut engine);

    assert_eq!(engine.state.objects[&target].zone, Zone::Exile);
    let new_tokens = battlefield_token_ids(&engine)
        .into_iter()
        .filter(|object_id| !tokens_before.contains(object_id))
        .collect::<Vec<_>>();
    assert_eq!(new_tokens.len(), 1);
    let token = new_tokens[0];
    let stats = engine
        .characteristics(token)
        .expect("copied Arena characteristics");
    assert_eq!(stats.colors, [tricerules_cards::primitives::Color::White]);
    assert_eq!((stats.power, stats.toughness), (Some(1), Some(1)));
    assert!(stats.has_type("Creature"));
    assert!(stats.has_type("Enchantment"));
    assert!(stats.has_type("Spirit"));
    assert!(engine.effective_has_keyword(token, Keyword::Flying));
}

#[test]
fn kaya_minus_three_auras_do_not_create_a_copy() {
    let mut engine = setup(&[10, 20]);
    let source = put_kaya(&mut engine, 0, 6);
    let enchanted_creature = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let aura = inject_permanent_on_battlefield(&mut engine, 1, "pacifism");
    engine.state.objects.get_mut(&aura).unwrap().attached_to = Some(
        tricerules_core::AttachmentRecipient::Object(enchanted_creature),
    );
    assert!(engine
        .characteristics(aura)
        .expect("Pacifism characteristics")
        .has_type("Aura"));
    let tokens_before = battlefield_token_ids(&engine);

    let aura_ref = permanent_target(&engine, aura);
    activate_kaya(&mut engine, 10, source, "activated_03", vec![aura_ref])
        .expect("an Aura is still a legal Enchantment target");
    pass_priority_round(&mut engine);

    assert_eq!(engine.state.objects[&aura].zone, Zone::Exile);
    assert_eq!(battlefield_token_ids(&engine), tokens_before);
}

#[test]
fn kaya_minus_three_rejects_land_target_without_paying_loyalty() {
    let mut engine = setup(&[10, 20]);
    let source = put_kaya(&mut engine, 0, 6);
    let land = inject_permanent_on_battlefield(&mut engine, 1, "forest");
    let stack_before = engine.state.stack.len();

    let land_ref = permanent_target(&engine, land);
    assert!(activate_kaya(&mut engine, 10, source, "activated_03", vec![land_ref],).is_err());
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Loyalty),
        6
    );
    assert_eq!(engine.state.stack.len(), stack_before);
    assert_eq!(engine.state.objects[&land].zone, Zone::Battlefield);
}

#[test]
fn kaya_minus_three_all_targets_becoming_illegal_keep_the_loyalty_cost_paid() {
    let mut engine = setup(&[10, 20]);
    let source = put_kaya(&mut engine, 0, 6);
    let target = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let tokens_before = battlefield_token_ids(&engine);

    let target_ref = permanent_target(&engine, target);
    activate_kaya(&mut engine, 10, source, "activated_03", vec![target_ref])
        .expect("activate Kaya's -3 with one legal target");
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Loyalty),
        3
    );
    engine.enable_dev_commands();
    engine
        .apply_scenario_command(10, &dev_move(20, DevZone::Graveyard, "Grizzly Bears"))
        .expect("move the sole target before the ability resolves");
    assert_eq!(engine.state.objects[&target].zone, Zone::Graveyard);
    pass_priority_round(&mut engine);

    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Loyalty),
        3
    );
    assert_eq!(battlefield_token_ids(&engine), tokens_before);
    assert!(engine.state.stack.is_empty());
}
