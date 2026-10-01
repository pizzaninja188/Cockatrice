//! Exact Glorious Sunrise combat modes and resolving condition/lifetime evidence.

use super::helpers::*;
use tricerules_cards::{CardRegistry, ContinuousEffectKind, EffectDuration, Keyword};
use tricerules_core::state::{AffectedScope, ContinuousEffect};
use tricerules_core::{GameEngine, TurnStep, Zone};
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, ruled_event::Ev, ChooseTriggerTarget, SelectedSpellMode,
};

const SUNRISE: &str = "glorious_sunrise";

fn setup() -> GameEngine {
    let deck = deck_with("mountain", &[]);
    let mut e = GameEngine::new(
        26_100_601,
        &[10, 20, 30],
        20,
        Some(vec![deck.clone(), deck.clone(), deck]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut e);
    inject_card_into_hand(&mut e, 0, SUNRISE);
    give_mana(
        &mut e,
        10,
        ManaGift {
            c: 3,
            g: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 0, SUNRISE);
    e.apply_command(10, &cast_spell(slot, vec![]))
        .expect("actual Sunrise cast with 3GG");
    pass_priority_round(&mut e);
    assert_eq!(e.state.stack.len(), 0);
    assert_eq!(e.state.players[0].mana_pool, Default::default());
    battlefield_object_for_card(&e, 0, SUNRISE);
    e
}

fn combat(e: &mut GameEngine) -> tricerules_proto::ruled::v1::TriggerNeedsTarget {
    assert_eq!(e.state.turn_step, TurnStep::Main1);
    let batch = e
        .apply_command(e.state.active_player_id(), &primitive_yield())
        .unwrap();
    assert_eq!(e.state.turn_step, TurnStep::BeginCombat);
    assert_eq!(e.state.pending_triggers.len(), 1);
    let prompt = batch
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::TriggerNeedsTarget(prompt)) => Some(prompt),
            _ => None,
        })
        .expect("published combat modal prompt");
    assert_eq!(
        (prompt.min_modes, prompt.max_modes, prompt.modes.len()),
        (1, 1, 4)
    );
    assert!(!prompt.may_decline);
    prompt.clone()
}

fn mode(index: u32, targets: Vec<TargetRef>) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            targets: vec![],
            selected_modes: vec![SelectedSpellMode {
                mode_index: index,
                targets,
            }],
        })),
    }
}

#[test]
fn sunrise_complete_definition_and_actual_life_mode() {
    let card = CardRegistry::global()
        .get(SUNRISE)
        .expect("exact Sunrise registered");
    assert_eq!(card.name, "Glorious Sunrise");
    assert_eq!(card.face_count(), 1);
    let face = card.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{3}{G}{G}");
    assert_eq!(face.types, ["Enchantment"]);
    assert!(face.activated_abilities.is_empty());
    assert!(face.static_abilities.is_empty());
    assert_eq!(face.triggered_abilities.len(), 1);
    let trigger = &face.triggered_abilities[0];
    assert!(trigger.intervening_if.is_none());
    assert!(!trigger.may);
    assert!(trigger.effect.is_empty());
    let modes = trigger.modal.as_ref().unwrap();
    assert_eq!(
        (modes.min_modes, modes.max_modes, modes.modes.len()),
        (1, 1, 4)
    );
    let mut e = setup();
    combat(&mut e);
    e.apply_command(10, &mode(3, vec![])).unwrap();
    pass_priority_round(&mut e);
    assert_eq!(
        e.state.players.iter().map(|p| p.life).collect::<Vec<_>>(),
        [23, 20, 20]
    );
    assert!(e.state.stack.is_empty());
}

fn reject(e: &mut GameEngine, actor: i32, command: &RuledCommand) {
    let before = serde_json::to_vec(&e.state).unwrap();
    e.apply_command(actor, command)
        .expect_err("illegal command rejects");
    assert_eq!(serde_json::to_vec(&e.state).unwrap(), before);
}

fn priority(e: &mut GameEngine, actor: i32) {
    for _ in 0..3 {
        if e.state.priority_player_id() == actor {
            return;
        }
        let current = e.state.priority_player_id();
        e.apply_command(current, &pass()).unwrap();
    }
    panic!("priority did not reach {actor}");
}

fn response(e: &mut GameEngine, seat: usize, card: &str, targets: Vec<TargetRef>) {
    let actor = e.state.players[seat].id;
    priority(e, actor);
    inject_card_into_hand(e, seat, card);
    give_mana(
        e,
        actor,
        ManaGift {
            c: 5,
            u: 2,
            g: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(e, seat, card);
    e.apply_command(actor, &cast_spell(slot, targets))
        .expect("actual response card cast");
    pass_priority_round(e);
}

fn advance_main(e: &mut GameEngine, actor: i32) {
    for _ in 0..150 {
        if e.state.active_player_id() == actor && e.state.turn_step == TurnStep::Main1 {
            return;
        }
        if e.state.cleanup_discard_player.is_some() {
            resolve_cleanup_discards_if_any(e);
        } else {
            pass_priority_round(e);
        }
    }
    panic!("did not advance to Main1 for {actor}");
}

fn suppress(e: &mut GameEngine, source: u32) {
    e.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(source),
        kind: ContinuousEffectKind::Layer6RemoveAllAbilities,
        condition: None,
        duration: EffectDuration::Indefinite,
        timestamp: e.state.command_index,
    });
}

fn transfer_control(e: &mut GameEngine, oid: u32, seat: usize) {
    // Existing control fixture: no spell producer is claimed for this branch.
    for player in &mut e.state.players {
        player.battlefield.retain(|id| *id != oid);
    }
    let actor = e.state.players[seat].id;
    e.state.players[seat].battlefield.push(oid);
    let object = e.state.objects.get_mut(&oid).unwrap();
    object.base_controller = actor;
    object.controller = actor;
}

fn view(
    e: &mut GameEngine,
    seat: usize,
    oid: u32,
) -> tricerules_proto::ruled::v1::BattlefieldObject {
    e.initial_response_batch()
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::ZoneView(zone)) => zone.per_player.get(seat),
            _ => None,
        })
        .and_then(|zone| {
            zone.battlefield_objects
                .iter()
                .find(|object| object.object_id == oid)
        })
        .cloned()
        .expect("published battlefield object")
}

fn grant(e: &mut GameEngine, land: u32) {
    let prompt = combat(e);
    let land_mode = &prompt.modes[1];
    assert!(
        land_mode.selectable,
        "opponent land makes land mode selectable"
    );
    assert!(
        land_mode.needs_target,
        "land mode publishes its target requirement"
    );
    let targets = land_mode.targets.as_ref().expect("land mode target schema");
    assert!(
        !targets.groups.is_empty(),
        "land mode publishes its target group"
    );
    assert!(targets.groups[0].valid_permanent_ids.contains(&land));
    e.apply_command(10, &mode(1, target_object(land))).unwrap();
    pass_priority_round(e);
}

fn grant_index(e: &mut GameEngine, seat: usize, land: u32) -> u32 {
    let object = view(e, seat, land);
    assert_eq!(
        object.activated_abilities.len(),
        2,
        "original Forest mana ability is retained"
    );
    object
        .activated_abilities
        .iter()
        .find(|ability| ability.text.contains("{G}{G}{G}"))
        .expect("published granted three-green tap ability")
        .ability_index
}

#[test]
fn sunrise_pump_snapshots_own_creatures_and_expires_at_cleanup() {
    let mut e = setup();
    let own = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let opponent = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    combat(&mut e);
    e.apply_command(10, &mode(0, vec![])).unwrap();
    pass_priority_round(&mut e);
    assert_eq!(
        (e.effective_power(own), e.effective_toughness(own)),
        (Some(3), Some(3))
    );
    assert!(e
        .characteristics(own)
        .unwrap()
        .keywords
        .contains(&Keyword::Trample));
    assert_eq!(e.effective_power(opponent), Some(2));
    assert!(!e
        .characteristics(opponent)
        .unwrap()
        .keywords
        .contains(&Keyword::Trample));
    let later = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    assert_eq!(e.effective_power(later), Some(2));
    assert!(!e
        .characteristics(later)
        .unwrap()
        .keywords
        .contains(&Keyword::Trample));
    advance_main(&mut e, 20);
    assert_eq!(
        (e.effective_power(own), e.effective_toughness(own)),
        (Some(2), Some(2))
    );
    assert!(!e
        .characteristics(own)
        .unwrap()
        .keywords
        .contains(&Keyword::Trample));
}

#[test]
fn sunrise_land_grant_follows_current_control_and_has_source_independent_lifetime() {
    for transfer in [false, true] {
        let mut e = setup();
        let land = inject_permanent_on_battlefield(&mut e, 1, "forest");
        grant(&mut e, land);
        let index = grant_index(&mut e, 1, land);
        let command = activate_ability_for(&e, land, index, vec![]);
        reject(&mut e, 10, &command);
        let source = battlefield_object_for_card(&e, 0, SUNRISE);
        response(&mut e, 0, "boomerang", target_object(source));
        assert_eq!(e.state.objects[&source].zone, Zone::Hand);
        if transfer {
            transfer_control(&mut e, land, 2);
        }
        let seat = if transfer { 2 } else { 1 };
        let actor = e.state.players[seat].id;
        let published = grant_index(&mut e, seat, land);
        assert_eq!(published, index);
        priority(&mut e, actor);
        let before = e.state.players[seat].mana_pool.green;
        e.apply_command(actor, &activate_ability_for(&e, land, published, vec![]))
            .unwrap();
        assert_eq!(e.state.players[seat].mana_pool.green, before + 3);
        assert!(e.state.objects[&land].tapped);
        assert!(
            e.state.stack.is_empty(),
            "mana ability resolves immediately"
        );
        advance_main(&mut e, 20);
        assert_eq!(
            view(&mut e, seat, land).activated_abilities.len(),
            1,
            "only granted ability expires"
        );
    }
}

#[test]
fn sunrise_land_grant_never_follows_a_new_incarnation() {
    for resolve_before_bounce in [false, true] {
        let mut e = setup();
        let land = inject_permanent_on_battlefield(&mut e, 1, "forest");
        let generation = e
            .state
            .zone_change_generation
            .get(&land)
            .copied()
            .unwrap_or(0);
        combat(&mut e);
        e.apply_command(10, &mode(1, target_object(land))).unwrap();
        if resolve_before_bounce {
            pass_priority_round(&mut e);
            grant_index(&mut e, 1, land);
        }
        response(&mut e, 0, "boomerang", target_object(land));
        assert_eq!(e.state.objects[&land].zone, Zone::Hand);
        assert_eq!(move_ready_to_battlefield(&mut e, 1, "forest"), land);
        assert!(e.state.zone_change_generation[&land] > generation);
        if !resolve_before_bounce {
            pass_priority_round(&mut e);
        }
        assert!(e.state.stack.is_empty());
        assert_eq!(view(&mut e, 1, land).activated_abilities.len(), 1);
    }
}

#[test]
fn sunrise_draw_qualification_is_checked_only_at_resolution() {
    for branch in 0..4 {
        let mut e = setup();
        let bear = inject_creature_on_battlefield(
            &mut e,
            if branch == 2 { 1 } else { 0 },
            "grizzly_bears",
        );
        if branch == 1 {
            response(&mut e, 0, "giant_growth", target_object(bear));
        }
        if branch == 3 {
            response(&mut e, 0, "giant_growth", target_object(bear));
        }
        if branch == 2 {
            response(&mut e, 1, "giant_growth", target_object(bear));
        }
        combat(&mut e);
        e.apply_command(10, &mode(2, vec![]))
            .expect("draw mode is legal without a qualifying own creature");
        if branch == 0 {
            response(&mut e, 0, "giant_growth", target_object(bear));
        }
        if branch == 1 {
            response(&mut e, 1, "boomerang", target_object(bear));
        }
        if branch == 3 {
            transfer_control(&mut e, bear, 1);
        }
        let hand = e.state.players[0].hand.len();
        let library = e.state.players[0].library.len();
        let draws = e.state.turn_history.current.player(10).cards_drawn;
        pass_priority_round(&mut e);
        let expected = usize::from(branch == 0);
        assert_eq!(e.state.players[0].hand.len(), hand + expected);
        assert_eq!(e.state.players[0].library.len(), library - expected);
        assert_eq!(
            e.state.turn_history.current.player(10).cards_drawn,
            draws + expected as u32
        );
        assert!(e.state.stack.is_empty());
    }
}

#[test]
fn sunrise_life_mode_uses_controller_combat_and_captured_controller() {
    let mut departing = setup();
    let departing_source = battlefield_object_for_card(&departing, 0, SUNRISE);
    combat(&mut departing);
    departing.apply_command(10, &mode(3, vec![])).unwrap();
    response(
        &mut departing,
        0,
        "boomerang",
        target_object(departing_source),
    );
    assert_eq!(departing.state.objects[&departing_source].zone, Zone::Hand);
    pass_priority_round(&mut departing);
    assert_eq!(
        departing.state.players[0].life, 23,
        "captured ability survives actual source departure"
    );
    let mut e = setup();
    let source = battlefield_object_for_card(&e, 0, SUNRISE);
    combat(&mut e);
    transfer_control(&mut e, source, 2);
    e.apply_command(10, &mode(3, vec![])).unwrap();
    pass_priority_round(&mut e);
    assert_eq!(
        e.state.players.iter().map(|p| p.life).collect::<Vec<_>>(),
        [23, 20, 20]
    );
    transfer_control(&mut e, source, 0);
    advance_main(&mut e, 20);
    e.apply_command(20, &primitive_yield()).unwrap();
    assert_eq!(e.state.turn_step, TurnStep::BeginCombat);
    assert!(e.state.pending_triggers.is_empty());
    assert!(e.state.stack.is_empty(), "no opponent-turn combat trigger");
}

#[test]
fn sunrise_multiple_sources_keep_independent_modal_trigger_identities() {
    let mut e = setup();
    let first = battlefield_object_for_card(&e, 0, SUNRISE);
    let second = inject_card_into_hand(&mut e, 0, SUNRISE);
    give_mana(
        &mut e,
        10,
        ManaGift {
            c: 3,
            g: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 0, SUNRISE);
    e.apply_command(10, &cast_spell(slot, vec![])).unwrap();
    pass_priority_round(&mut e);
    assert_ne!(first, second);
    e.apply_command(10, &primitive_yield()).unwrap();
    let candidates = &e.state.pending_trigger_order.as_ref().unwrap().candidates;
    assert_eq!(candidates.len(), 2);
    assert_ne!(candidates[0].object_id, candidates[1].object_id);
    for _ in 0..2 {
        answer_trigger_order_in_engine_order(&mut e);
        e.apply_command(10, &mode(3, vec![])).unwrap();
    }
    let sources: std::collections::BTreeSet<_> = e
        .state
        .stack
        .iter()
        .map(|s| s.source_permanent_id)
        .collect();
    assert_eq!(sources, [Some(first), Some(second)].into_iter().collect());
    pass_priority_round(&mut e);
    pass_priority_round(&mut e);
    assert_eq!(e.state.players[0].life, 26);
    assert!(e.state.pending_triggers.is_empty());
    assert!(e.state.stack.is_empty());
}

#[test]
fn sunrise_modal_rejections_and_source_suppression_preserve_capture_contract() {
    let mut e = setup();
    let creature = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    combat(&mut e);
    reject(&mut e, 20, &mode(3, vec![]));
    let mut empty = mode(3, vec![]);
    if let Some(Cmd::ChooseTriggerTarget(choice)) = empty.cmd.as_mut() {
        choice.selected_modes.clear();
    }
    reject(&mut e, 10, &empty);
    for extra in [0, 3] {
        let mut command = mode(3, vec![]);
        if let Some(Cmd::ChooseTriggerTarget(choice)) = command.cmd.as_mut() {
            choice.selected_modes.push(SelectedSpellMode {
                mode_index: extra,
                targets: vec![],
            });
        }
        reject(&mut e, 10, &command);
    }
    reject(&mut e, 10, &mode(4, vec![]));
    reject(&mut e, 10, &mode(1, vec![]));
    reject(&mut e, 10, &mode(1, target_object(creature)));
    reject(&mut e, 10, &mode(3, target_object(creature)));
    let mut top_targets = mode(3, vec![]);
    if let Some(Cmd::ChooseTriggerTarget(choice)) = top_targets.cmd.as_mut() {
        choice.targets = target_object(creature);
    }
    reject(&mut e, 10, &top_targets);
    let mut decline = mode(3, vec![]);
    if let Some(Cmd::ChooseTriggerTarget(choice)) = decline.cmd.as_mut() {
        choice.decline = true;
    }
    reject(&mut e, 10, &decline);
    e.apply_command(10, &mode(3, vec![])).unwrap();
    pass_priority_round(&mut e);
    assert_eq!(e.state.players[0].life, 23);
    for before_capture in [true, false] {
        let mut e = setup();
        let source = battlefield_object_for_card(&e, 0, SUNRISE);
        if before_capture {
            suppress(&mut e, source);
        }
        e.apply_command(10, &primitive_yield()).unwrap();
        assert_eq!(e.state.pending_triggers.len(), usize::from(!before_capture));
        if !before_capture {
            suppress(&mut e, source);
            e.apply_command(10, &mode(3, vec![])).unwrap();
            pass_priority_round(&mut e);
            assert_eq!(e.state.players[0].life, 23);
        }
    }
}
