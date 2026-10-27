//! Teferi, Temporal Archmage's actual -10 payment and instant-timing emblem permission.
use super::helpers::*;
use crate::helpers::ScenarioCommandExt;
use tricerules_cards::primitives::{CounterKind, StaticEmblemEffect};
use tricerules_core::Zone;

const TEFERI: &str = "teferi,_temporal_archmage";
const JACE: &str = "jace,_unraveler_of_secrets";
const OATH_OF_TEFERI: &str = "oath_of_teferi";
const CHAIN_VEIL: &str = "the_chain_veil";
const CARTH: &str = "carth_the_lion";
const CONTROLLER: i32 = 10;
const OPPONENT: i32 = 20;
const THIRD_PLAYER: i32 = 30;
const FOURTH_PLAYER: i32 = 40;

fn setup(seed: u64) -> GameEngine {
    let registry = tricerules_cards::registry::global();
    for card in [
        TEFERI,
        JACE,
        OATH_OF_TEFERI,
        CHAIN_VEIL,
        CARTH,
        "lightning_bolt",
        "bonesplitter",
    ] {
        registry
            .get(card)
            .unwrap_or_else(|| panic!("{card} must be registered"));
    }
    let deck = deck_with("forest", &[]);
    let mut engine = GameEngine::new(
        registry,
        seed,
        &[CONTROLLER, OPPONENT],
        20,
        Some(vec![deck.clone(), deck]),
        true,
    )
    .expect("two-player Teferi emblem scenario game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn setup_four_player(seed: u64) -> GameEngine {
    let registry = tricerules_cards::registry::global();
    for card in [TEFERI, JACE, "lightning_bolt"] {
        registry
            .get(card)
            .unwrap_or_else(|| panic!("{card} must be registered"));
    }
    let deck = deck_with("forest", &[]);
    let mut engine = GameEngine::new(
        registry,
        seed,
        &[CONTROLLER, OPPONENT, THIRD_PLAYER, FOURTH_PLAYER],
        20,
        Some(vec![deck.clone(), deck.clone(), deck.clone(), deck]),
        true,
    )
    .expect("four-player Teferi emblem scenario game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn add_synthetic_continuous_effect(
    engine: &mut GameEngine,
    object_id: u32,
    kind: tricerules_cards::ContinuousEffectKind,
) {
    use tricerules_cards::EffectDuration;
    use tricerules_core::{AffectedScope, ContinuousEffect};

    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(object_id),
        kind,
        condition: None,
        duration: EffectDuration::Indefinite,
        timestamp: engine.state.command_index + 1,
    });
}

fn stage_synthetic_layer2_control(engine: &mut GameEngine, object_id: u32, controller: i32) {
    use tricerules_cards::{ContinuousEffectKind, ControllerReference};

    let owner = engine.state.objects[&object_id].owner;
    let base_controller = engine.state.objects[&object_id].base_controller;
    let previous_controller = engine.state.objects[&object_id].controller;
    add_synthetic_continuous_effect(
        engine,
        object_id,
        ContinuousEffectKind::Layer2Control {
            controller: ControllerReference::Fixed(controller),
        },
    );

    // Synthetic staging keeps the materialized controller cache and per-player battlefield index
    // coherent with the fixed Layer 2 effect. Physical owner and layer-2 base controller stay put.
    let previous_index = engine
        .state
        .player_idx(previous_controller)
        .expect("previous controller seat");
    let controller_index = engine
        .state
        .player_idx(controller)
        .expect("new controller seat");
    if previous_index != controller_index {
        engine.state.players[previous_index]
            .battlefield
            .retain(|candidate| *candidate != object_id);
    }
    if !engine.state.players[controller_index]
        .battlefield
        .contains(&object_id)
    {
        engine.state.players[controller_index]
            .battlefield
            .push(object_id);
    }
    engine
        .state
        .objects
        .get_mut(&object_id)
        .expect("controlled battlefield object")
        .controller = controller;
    assert_eq!(engine.state.objects[&object_id].owner, owner);
    assert_eq!(
        engine.state.objects[&object_id].base_controller,
        base_controller
    );
    assert_eq!(engine.state.objects[&object_id].controller, controller);
    assert!(engine.state.players[controller_index]
        .battlefield
        .contains(&object_id));
    engine.initial_response_batch();
}

fn drain_stack_answering_jace_scry(engine: &mut GameEngine) {
    for _ in 0..32 {
        if let Some(pending) = engine.state.pending_resolution.as_ref() {
            assert_eq!(pending.deciding_player, CONTROLLER);
            engine
                .apply_scenario_command(CONTROLLER, &submit_resolution_choice(vec![]))
                .expect("keep Jace's scried card on top, then draw");
            continue;
        }
        if engine.state.stack.is_empty() {
            return;
        }
        pass_priority_round(engine);
    }
    panic!("Jace's Scry choice and remaining stack items did not finish");
}

fn setup_with_marker_and_jace(seed: u64) -> (GameEngine, u32) {
    let mut engine = setup(seed);
    let teferi = cast_teferi(&mut engine, 0);
    activate_ultimate_and_create_marker(&mut engine, CONTROLLER, teferi);
    let jace = cast_jace(&mut engine, 0);
    (engine, jace)
}

fn cast_teferi(engine: &mut GameEngine, seat: usize) -> u32 {
    let player = engine.state.players[seat].id;
    let source = inject_card_into_hand(engine, seat, TEFERI);
    give_mana(
        engine,
        player,
        ManaGift {
            u: 2,
            c: 4,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(engine, seat, TEFERI);
    engine
        .apply_scenario_command(player, &cast_spell(slot, vec![]))
        .expect("pay {4}{U}{U} to cast Teferi");
    pass_both_players(engine);
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Loyalty),
        5,
        "the actual Teferi entry uses the registered starting loyalty"
    );
    source
}

fn cast_jace(engine: &mut GameEngine, seat: usize) -> u32 {
    let player = engine.state.players[seat].id;
    let source = inject_card_into_hand(engine, seat, JACE);
    give_mana(
        engine,
        player,
        ManaGift {
            u: 2,
            c: 3,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(engine, seat, JACE);
    engine
        .apply_scenario_command(player, &cast_spell(slot, vec![]))
        .expect("pay {3}{U}{U} to cast Jace");
    pass_both_players(engine);
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Loyalty),
        5,
        "the actual Jace entry uses the registered starting loyalty"
    );
    source
}

fn cast_bolt(engine: &mut GameEngine, seat: usize, targets: Vec<TargetRef>) -> u32 {
    let player = engine.state.players[seat].id;
    let source = inject_card_into_hand(engine, seat, "lightning_bolt");
    give_mana(
        engine,
        player,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(engine, seat, "lightning_bolt");
    engine
        .apply_scenario_command(player, &cast_spell(slot, targets))
        .expect("pay {R} and cast Lightning Bolt at instant speed");
    assert_eq!(engine.state.stack.last().unwrap().card_id, "lightning_bolt");
    source
}

fn cast_oath_of_teferi_first(engine: &mut GameEngine) -> u32 {
    assert!(engine.state.players[0].battlefield.is_empty());
    let oath = inject_card_into_hand(engine, 0, OATH_OF_TEFERI);
    give_mana(
        engine,
        CONTROLLER,
        ManaGift {
            w: 1,
            u: 1,
            c: 3,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(engine, 0, OATH_OF_TEFERI);
    engine
        .apply_scenario_command(CONTROLLER, &cast_spell(slot, vec![]))
        .expect("cast actual Oath of Teferi for {3}{W}{U} on an empty battlefield");
    assert_eq!(engine.state.stack.last().unwrap().card_id, OATH_OF_TEFERI);
    pass_priority_round(engine);
    assert_eq!(engine.state.objects[&oath].zone, Zone::Battlefield);
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.pending_triggers.is_empty());
    oath
}

fn activate_and_resolve_teferi_plus_one(engine: &mut GameEngine, teferi: u32) {
    let before = engine.state.objects[&teferi].counter_count(CounterKind::Loyalty);
    let offer = ability_info(engine, teferi, 0);
    assert!(offer.activatable, "Oath permits this Teferi +1 activation");
    let activation = activate_ability_for(engine, teferi, 0, vec![]);
    engine
        .apply_scenario_command(CONTROLLER, &activation)
        .expect("activate Teferi's registered +1");
    assert_eq!(
        engine.state.objects[&teferi].counter_count(CounterKind::Loyalty),
        before + 1
    );
    pass_priority_round(engine);
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Teferi's actual +1 parks its mandatory library choice");
    assert_eq!(pending.deciding_player, CONTROLLER);
    assert_eq!(pending.presentation.choice_kind, ChoiceKind::LibraryLook);
    assert_eq!((pending.presentation.min, pending.presentation.max), (1, 1));
    let selected = *pending
        .presentation
        .candidates
        .first()
        .expect("the actual library has a card for Teferi's mandatory choice");
    engine
        .apply_scenario_command(CONTROLLER, &submit_resolution_choice(vec![selected]))
        .expect("make Teferi's mandatory private library choice");
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_none());
}

fn advance_from_p10_through_p20_to_p10(engine: &mut GameEngine) {
    end_active_turn(engine, CONTROLLER);
    assert_eq!(engine.state.active_player_id(), OPPONENT);
    advance_to_main1_from_game_start(engine);
    end_active_turn(engine, OPPONENT);
    assert_eq!(engine.state.active_player_id(), CONTROLLER);
    advance_to_main1_from_game_start(engine);
}

fn cast_and_resolve_carth(engine: &mut GameEngine) -> u32 {
    let carth = inject_card_into_hand(engine, 0, CARTH);
    give_mana(
        engine,
        CONTROLLER,
        ManaGift {
            b: 1,
            g: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(engine, 0, CARTH);
    engine
        .apply_scenario_command(CONTROLLER, &cast_spell(slot, vec![]))
        .expect("cast actual Carth the Lion for {2}{B}{G}");
    pass_both_players(engine);
    assert_eq!(engine.state.objects[&carth].zone, Zone::Battlefield);
    let entry_trigger = engine
        .state
        .stack
        .last()
        .expect("actual Carth entry trigger is on the stack")
        .id;
    let mut batch = pass_both_players_with_batch(engine);
    if engine.state.pending_resolution.is_some() {
        let pending = engine.state.pending_resolution.as_ref().unwrap();
        assert_eq!(pending.deciding_player, CONTROLLER);
        assert_eq!(pending.presentation.choice_kind, ChoiceKind::LibraryLook);
        assert_eq!((pending.presentation.min, pending.presentation.max), (0, 0));
        batch = engine
            .apply_scenario_command(CONTROLLER, &submit_resolution_choice(vec![]))
            .expect("decline Carth's optional entry selection");
    }
    assert!(!batch
        .events
        .iter()
        .any(|event| matches!(&event.ev, Some(Ev::CardsRevealed(_)))));
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert_eq!(
        batch
            .events
            .iter()
            .filter(|event| matches!(
                &event.ev, Some(Ev::StackResolved(resolved)) if resolved.object_id == entry_trigger
            ))
            .count(),
        1,
        "Carth's real entry look resolves after declining its optional selection"
    );
    carth
}

fn cast_registered_spell(
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
        .unwrap_or_else(|_| panic!("cast registered {card_id}"));
    resolve_entire_stack_two_player(engine);
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    assert!(engine.state.pending_resolution.is_none());
    source
}

fn ability_info(
    engine: &mut GameEngine,
    source: u32,
    index: u32,
) -> tricerules_proto::ruled::v1::AbilityInfo {
    engine
        .initial_response_batch()
        .events
        .iter()
        .find_map(|event| {
            let Some(Ev::ZoneView(view)) = &event.ev else {
                return None;
            };
            view.per_player
                .iter()
                .flat_map(|player| &player.battlefield_objects)
                .find(|object| object.object_id == source)
                .and_then(|object| {
                    object
                        .activated_abilities
                        .iter()
                        .find(|ability| ability.ability_index == index)
                })
                .cloned()
        })
        .expect("the engine publishes the source's activated ability offer")
}

fn reject_without_mutation(engine: &mut GameEngine, player: i32, command: &RuledCommand) {
    let before = engine
        .diagnostic_snapshot()
        .expect("snapshot before rejected activation");
    assert!(engine.apply_scenario_command(player, command).is_err());
    assert_eq!(
        engine
            .diagnostic_snapshot()
            .expect("snapshot after rejected activation"),
        before,
        "a refused activation leaves costs and state untouched"
    );
}

fn activate_ultimate_and_create_marker(engine: &mut GameEngine, player: i32, teferi: u32) -> u32 {
    // Loyalty setup is synthetic, whether the registered card was cast or injected for a
    // multiplayer fixture. Payment, source departure and the actual -10 ability remain engine-driven.
    let previous_marker_ids = engine
        .state
        .static_emblems
        .iter()
        .map(|emblem| emblem.object_id)
        .collect::<Vec<_>>();
    engine
        .state
        .objects
        .get_mut(&teferi)
        .expect("cast Teferi")
        .set_counter(CounterKind::Loyalty, 10);
    let command = activate_ability_for(engine, teferi, 2, vec![]);
    engine
        .apply_scenario_command(player, &command)
        .expect("pay the actual -10 loyalty cost");
    assert_eq!(
        engine.state.objects[&teferi].counter_count(CounterKind::Loyalty),
        0,
        "the full -10 is paid before resolution"
    );
    assert_eq!(
        engine.state.objects[&teferi].zone,
        Zone::Graveyard,
        "zero loyalty sends the source to its graveyard before the ultimate resolves"
    );
    assert_eq!(engine.state.stack.len(), 1);
    assert_eq!(engine.state.stack[0].source_permanent_id, Some(teferi));

    pass_priority_round(engine);
    assert!(engine.state.stack.is_empty());
    assert_eq!(
        engine.state.static_emblems.len(),
        previous_marker_ids.len() + 1,
        "the resolved ultimate creates exactly one new marker"
    );
    let emblem = engine
        .state
        .static_emblems
        .iter()
        .find(|emblem| {
            !previous_marker_ids.contains(&emblem.object_id)
                && emblem.effects == [StaticEmblemEffect::PlaneswalkerLoyaltyInstantTiming]
        })
        .expect("the resolved ultimate creates its static permission marker");
    assert_eq!(emblem.controller, player);
    let marker_id = emblem.object_id;

    let view = engine
        .initial_response_batch()
        .events
        .into_iter()
        .find_map(|event| match event.ev {
            Some(Ev::ZoneView(view)) => Some(view),
            _ => None,
        })
        .expect("the engine board view includes emblem markers");
    assert_eq!(view.per_player.len(), engine.state.players.len());
    let marker_scopes = view
        .per_player
        .iter()
        .filter(|player_view| {
            player_view
                .static_emblems
                .iter()
                .any(|scoped_emblem| scoped_emblem.object_id == marker_id)
        })
        .map(|player_view| player_view.player_id)
        .collect::<Vec<_>>();
    assert_eq!(
        marker_scopes,
        [player],
        "the marker appears only under its controller's board scope"
    );
    marker_id
}

#[test]
fn teferi_minus_ten_uses_a_paid_cast_and_leaves_a_controller_scoped_marker() {
    let mut engine = setup(26_102_721);
    let teferi = cast_teferi(&mut engine, 0);
    let marker = activate_ultimate_and_create_marker(&mut engine, CONTROLLER, teferi);

    let emblem = engine
        .state
        .static_emblems
        .iter()
        .find(|emblem| emblem.object_id == marker)
        .expect("the marker remains after Teferi leaves");
    assert_eq!(emblem.controller, CONTROLLER);
    assert_eq!(
        emblem.effects,
        [StaticEmblemEffect::PlaneswalkerLoyaltyInstantTiming]
    );
}

#[test]
fn teferi_emblem_offers_one_offturn_jace_activation_above_a_bolt() {
    let mut engine = setup(26_102_722);
    let teferi = cast_teferi(&mut engine, 0);
    activate_ultimate_and_create_marker(&mut engine, CONTROLLER, teferi);
    let jace = cast_jace(&mut engine, 0);
    let bonesplitter = inject_permanent_on_battlefield(&mut engine, 0, "bonesplitter");
    let creature = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");

    end_active_turn(&mut engine, CONTROLLER);
    assert_eq!(engine.state.active_player_id(), OPPONENT);
    let bolt = cast_bolt(&mut engine, 1, target_object(jace));
    engine
        .apply_scenario_command(OPPONENT, &pass())
        .expect("the Bolt caster passes priority to Teferi's controller");
    assert_eq!(engine.state.priority_player_id(), CONTROLLER);
    assert_eq!(engine.state.stack.len(), 1);
    assert_eq!(engine.state.stack[0].card_id, "lightning_bolt");
    assert_eq!(engine.state.stack[0].id, bolt);

    let offered = ability_info(&mut engine, jace, 0);
    assert!(
        offered.activatable,
        "the controller's emblem makes the real Jace +1 available offturn over a nonempty stack"
    );
    assert_eq!(offered.cost_label, "+1");
    let activation = activate_ability_for(&engine, jace, 0, vec![]);
    engine
        .apply_scenario_command(CONTROLLER, &activation)
        .expect("activate Jace in response to Lightning Bolt");
    assert_eq!(
        engine.state.objects[&jace].counter_count(CounterKind::Loyalty),
        6,
        "the actual +1 cost is applied exactly once"
    );
    assert_eq!(engine.state.stack.len(), 2);
    assert_eq!(engine.state.stack[0].card_id, "lightning_bolt");
    assert_eq!(
        engine.state.stack[1].source_permanent_id,
        Some(jace),
        "Jace's response is above the unresolved Bolt"
    );

    // Jace's controller passes priority back; the Bolt caster can add another instant. This
    // returns priority to the controller with a live stack, isolating the one-use loyalty limit.
    engine
        .apply_scenario_command(CONTROLLER, &pass())
        .expect("the active-player response passes priority back to the Bolt caster");
    assert_eq!(engine.state.priority_player_id(), OPPONENT);
    cast_bolt(&mut engine, 1, target_player(CONTROLLER));
    assert_eq!(engine.state.stack.len(), 3);
    engine
        .apply_scenario_command(OPPONENT, &pass())
        .expect("the second Bolt caster passes priority back to Jace's controller");
    assert_eq!(engine.state.priority_player_id(), CONTROLLER);
    let spent = ability_info(&mut engine, jace, 0);
    assert!(
        !spent.activatable,
        "the same Jace cannot activate a second loyalty ability during this turn"
    );
    let second_activation = activate_ability_for(&engine, jace, 0, vec![]);
    reject_without_mutation(&mut engine, CONTROLLER, &second_activation);

    // The emblem does not bypass an ordinary sorcery-speed Equip activation on a nonwalker.
    give_mana(
        &mut engine,
        CONTROLLER,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    let equip = ability_info(&mut engine, bonesplitter, 0);
    assert_eq!(equip.cost_label, "{1}");
    assert!(
        !equip.activatable,
        "Teferi's permission applies to planeswalker loyalty abilities, not nonwalker Equip"
    );
    let equip_command = activate_ability_for(&engine, bonesplitter, 0, target_object(creature));
    reject_without_mutation(&mut engine, CONTROLLER, &equip_command);
}

#[test]
fn teferi_emblem_does_not_grant_offturn_timing_without_its_controller_marker() {
    let mut engine = setup(26_102_723);
    let jace = cast_jace(&mut engine, 0);

    end_active_turn(&mut engine, CONTROLLER);
    assert_eq!(engine.state.active_player_id(), OPPONENT);
    let first_bolt = cast_bolt(&mut engine, 1, target_player(CONTROLLER));
    engine
        .apply_scenario_command(OPPONENT, &pass())
        .expect("the Bolt caster passes priority to Jace's controller");
    assert_eq!(engine.state.priority_player_id(), CONTROLLER);
    assert_eq!(engine.state.stack[0].id, first_bolt);
    let no_marker_offer = ability_info(&mut engine, jace, 0);
    assert!(
        !no_marker_offer.activatable,
        "without a Teferi emblem, Jace's loyalty ability stays sorcery-speed"
    );
    let no_marker_command = activate_ability_for(&engine, jace, 0, vec![]);
    reject_without_mutation(&mut engine, CONTROLLER, &no_marker_command);
    pass_both_players(&mut engine);
    assert!(engine.state.stack.is_empty());

    pass_priority_round(&mut engine); // upkeep -> draw
    pass_priority_round(&mut engine); // draw -> main1
    assert_eq!(engine.state.turn_step, tricerules_core::TurnStep::Main1);
    let teferi = cast_teferi(&mut engine, 1);
    let marker = activate_ultimate_and_create_marker(&mut engine, OPPONENT, teferi);
    assert_eq!(
        engine
            .state
            .static_emblems
            .iter()
            .find(|emblem| emblem.object_id == marker)
            .unwrap()
            .controller,
        OPPONENT,
        "the only permission marker belongs to the opponent"
    );

    let foreign_bolt = cast_bolt(&mut engine, 1, target_player(CONTROLLER));
    engine
        .apply_scenario_command(OPPONENT, &pass())
        .expect("the Bolt caster passes priority to Jace's controller");
    assert_eq!(engine.state.priority_player_id(), CONTROLLER);
    assert_eq!(engine.state.stack.last().unwrap().id, foreign_bolt);
    let foreign_marker_offer = ability_info(&mut engine, jace, 0);
    assert!(
        !foreign_marker_offer.activatable,
        "an opponent's Teferi emblem does not grant timing to this Jace"
    );
    let foreign_marker_command = activate_ability_for(&engine, jace, 0, vec![]);
    reject_without_mutation(&mut engine, CONTROLLER, &foreign_marker_command);
}

#[test]
fn teferi_cleanup_priority_flag_refreshes_the_cached_loyalty_offer() {
    let (mut engine, jace) = setup_with_marker_and_jace(26_102_724);

    // Synthetic phase setup isolates the cleanup priority flag from all battlefield changes.
    engine.state.turn_step = tricerules_core::TurnStep::Cleanup;
    engine.state.cleanup_priority_active = false;
    engine.state.priority_idx = engine.state.player_idx(OPPONENT).expect("opponent seat");
    engine.state.passes_since_stack_change = 0;
    assert_eq!(engine.state.priority_player_id(), OPPONENT);
    assert!(
        !ability_info(&mut engine, jace, 0).activatable,
        "Cleanup without an active cleanup priority does not permit instant timing"
    );

    // This is the only state change before the rejection and real pass. In particular, don't
    // reseed the view cache: it must notice cleanup_priority_active as an offer input.
    engine.state.cleanup_priority_active = true;
    let wrong_player = activate_ability_for(&engine, jace, 0, vec![]);
    reject_without_mutation(&mut engine, CONTROLLER, &wrong_player);

    let batch = engine
        .apply_scenario_command(OPPONENT, &pass())
        .expect("the actual cleanup-priority holder passes to Teferi's controller");
    assert_eq!(engine.state.turn_step, tricerules_core::TurnStep::Cleanup);
    assert!(engine.state.cleanup_priority_active);
    assert_eq!(engine.state.priority_player_id(), CONTROLLER);
    let view = batch
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::ZoneView(view)) => Some(view),
            _ => None,
        })
        .expect("the pass publishes its tracked battlefield view");
    assert!(
        !view.battlefields_unchanged,
        "changing cleanup_priority_active refreshes serialized ability flags"
    );
    let jace_view = view
        .per_player
        .iter()
        .find(|player_view| player_view.player_id == CONTROLLER)
        .expect("Jace's controller board scope")
        .battlefield_objects
        .iter()
        .find(|object| object.object_id == jace)
        .expect("the refreshed view includes the actual Jace");
    assert!(
        jace_view
            .activated_abilities
            .iter()
            .find(|ability| ability.ability_index == 0)
            .expect("Jace's +1 offer")
            .activatable,
        "the current offer is refreshed with cleanup priority active"
    );

    let activation = activate_ability_for(&engine, jace, 0, vec![]);
    engine
        .apply_scenario_command(CONTROLLER, &activation)
        .expect("the emblem permits Jace's loyalty ability during cleanup priority");
    assert_eq!(
        engine.state.objects[&jace].counter_count(CounterKind::Loyalty),
        6
    );
    assert_eq!(
        engine.state.stack.last().unwrap().source_permanent_id,
        Some(jace)
    );
}

#[test]
fn teferi_emblem_does_not_allow_loyalty_activation_outside_instant_timing_steps() {
    let (mut engine, jace) = setup_with_marker_and_jace(26_102_725);

    // Synthetic phase setup supplies a placeholder priority actor; Untap itself has no instant
    // timing window, regardless of the emblem.
    engine.state.turn_step = tricerules_core::TurnStep::Untap;
    engine.state.priority_idx = engine
        .state
        .player_idx(CONTROLLER)
        .expect("controller seat");
    let offer = ability_info(&mut engine, jace, 0);
    assert!(
        !offer.activatable,
        "Untap is outside the modeled instant timing steps"
    );
    let activation = activate_ability_for(&engine, jace, 0, vec![]);
    reject_without_mutation(&mut engine, CONTROLLER, &activation);

    let (mut engine, jace) = setup_with_marker_and_jace(26_102_726);

    // Synthetic phase setup: the placeholder priority index is not a Cleanup priority window.
    engine.state.turn_step = tricerules_core::TurnStep::Cleanup;
    engine.state.cleanup_priority_active = false;
    engine.state.priority_idx = engine
        .state
        .player_idx(CONTROLLER)
        .expect("controller seat");
    engine.state.passes_since_stack_change = 0;
    assert!(
        !ability_info(&mut engine, jace, 0).activatable,
        "Cleanup without cleanup priority is outside the modeled instant timing steps"
    );
    let activation = activate_ability_for(&engine, jace, 0, vec![]);
    reject_without_mutation(&mut engine, CONTROLLER, &activation);
}

#[test]
fn teferi_emblem_does_not_bypass_explicit_sorcery_speed_on_jace() {
    let mut engine = setup(26_102_727);
    let teferi = cast_teferi(&mut engine, 0);
    activate_ultimate_and_create_marker(&mut engine, CONTROLLER, teferi);
    let jace = cast_jace(&mut engine, 0);

    // Synthetic copied-face fixture on the actual cast Jace changes only its first ability's
    // explicit timing field. Card/face/ability identity and the Program owner/body stay intact.
    let definition = tricerules_cards::registry::global()
        .get(JACE)
        .expect("the actual Jace definition is registered");
    let mut face = definition.primary_face().clone();
    let original_program = face.activated_abilities[0]
        .program
        .clone()
        .expect("Jace's first loyalty ability owns its Scry/Draw Program");
    face.activated_abilities[0].timing = tricerules_card_model::ActivationTiming::SorcerySpeed;
    engine
        .state
        .objects
        .get_mut(&jace)
        .expect("the actual Jace remains on the battlefield")
        .copiable_values = Some(tricerules_core::state::CopiableValues {
        prototyped: false,
        source_card_id: JACE.into(),
        source_face_index: 0,
        face,
        room_faces: None,
        display_name: definition.name.clone(),
    });
    let copied = engine.state.objects[&jace]
        .copiable_values
        .as_ref()
        .expect("the labeled copied-face timing fixture");
    assert_eq!(copied.source_card_id, JACE);
    assert_eq!(copied.source_face_index, 0);
    assert_eq!(copied.face.face_id.as_str(), "jace_unraveler_of_secrets");
    assert_eq!(
        copied.face.activated_abilities[0].ability_id.as_str(),
        "activated_01"
    );
    assert_eq!(
        copied.face.activated_abilities[0].timing,
        tricerules_card_model::ActivationTiming::SorcerySpeed
    );
    assert_eq!(
        copied.face.activated_abilities[0].program.as_ref(),
        Some(&original_program),
        "the same Jace ability keeps its original Program body and owner identity"
    );

    end_active_turn(&mut engine, CONTROLLER);
    assert_eq!(engine.state.active_player_id(), OPPONENT);
    cast_bolt(&mut engine, 1, target_player(CONTROLLER));
    engine
        .apply_scenario_command(OPPONENT, &pass())
        .expect("the Bolt caster passes to the actual Jace controller");
    assert_eq!(engine.state.priority_player_id(), CONTROLLER);
    assert_eq!(engine.state.stack.last().unwrap().card_id, "lightning_bolt");

    let offer = ability_info(&mut engine, jace, 0);
    assert!(
        !offer.activatable,
        "Teferi's emblem does not waive the copied Jace ability's explicit SorcerySpeed timing"
    );
    let activation = activate_ability_for(&engine, jace, 0, vec![]);
    reject_without_mutation(&mut engine, CONTROLLER, &activation);
    assert_eq!(
        engine.state.objects[&jace].counter_count(CounterKind::Loyalty),
        5
    );
}

#[test]
fn teferi_emblem_uses_jaces_current_controller_and_planeswalker_type() {
    use tricerules_cards::{
        ContinuousEffectKind, ControllerReference, PermanentTypeFilter, TypeLineReplacement,
    };
    use tricerules_core::TurnStep;

    let (mut engine, jace) = setup_with_marker_and_jace(26_102_728);
    // Synthetic Layer 2 control changes only the current controller; the physical owner stays P10.
    add_synthetic_continuous_effect(
        &mut engine,
        jace,
        ContinuousEffectKind::Layer2Control {
            controller: ControllerReference::Fixed(OPPONENT),
        },
    );
    assert_eq!(engine.state.objects[&jace].owner, CONTROLLER);
    assert_eq!(engine.characteristics(jace).unwrap().controller, OPPONENT);

    end_active_turn(&mut engine, CONTROLLER);
    assert_eq!(engine.state.active_player_id(), OPPONENT);
    let bolt = cast_bolt(&mut engine, 1, target_player(CONTROLLER));
    assert_eq!(engine.state.priority_player_id(), OPPONENT);
    assert_eq!(engine.state.stack.last().unwrap().id, bolt);
    let foreign_controller_offer = ability_info(&mut engine, jace, 0);
    assert!(
        !foreign_controller_offer.activatable,
        "P20's control of Jace does not borrow P10's emblem permission"
    );
    let foreign_controller_activation = activate_ability_for(&engine, jace, 0, vec![]);
    reject_without_mutation(&mut engine, OPPONENT, &foreign_controller_activation);

    engine
        .apply_scenario_command(OPPONENT, &pass())
        .expect("P20 passes priority to P10 without resolving the Bolt");
    assert_eq!(engine.state.priority_player_id(), CONTROLLER);
    engine.state.continuous_effects.retain(|effect| {
        !matches!(
            effect.kind,
            ContinuousEffectKind::Layer2Control {
                controller: ControllerReference::Fixed(OPPONENT)
            }
        )
    });
    // Re-stage the cache/index for the preserved P10 control effect before testing type alone.
    stage_synthetic_layer2_control(&mut engine, jace, CONTROLLER);
    assert_eq!(engine.characteristics(jace).unwrap().controller, CONTROLLER);
    assert_eq!(engine.state.objects[&jace].zone, Zone::Battlefield);

    // Synthetic layer 4 type change retains the real Jace definition and its loyalty ability.
    add_synthetic_continuous_effect(
        &mut engine,
        jace,
        ContinuousEffectKind::Layer4SetTypeLine(TypeLineReplacement {
            card_types: vec![PermanentTypeFilter::Artifact],
            creature_types: vec![],
            land_types: vec![],
        }),
    );
    assert!(!engine
        .characteristics(jace)
        .unwrap()
        .has_type("Planeswalker"));
    assert_eq!(engine.state.turn_step, TurnStep::Upkeep);
    let nonwalker_offer = ability_info(&mut engine, jace, 0);
    assert!(
        !nonwalker_offer.activatable,
        "Teferi's permission requires Jace's current type to include Planeswalker"
    );
    let nonwalker_activation = activate_ability_for(&engine, jace, 0, vec![]);
    reject_without_mutation(&mut engine, CONTROLLER, &nonwalker_activation);

    engine
        .state
        .continuous_effects
        .retain(|effect| !matches!(effect.kind, ContinuousEffectKind::Layer4SetTypeLine(_)));
    engine.initial_response_batch();
    assert!(engine
        .characteristics(jace)
        .unwrap()
        .has_type("Planeswalker"));
    let current_offer = ability_info(&mut engine, jace, 0);
    assert!(current_offer.activatable);
    let activation = activate_ability_for(&engine, jace, 0, vec![]);
    engine
        .apply_scenario_command(CONTROLLER, &activation)
        .expect("P10 may activate the current planeswalker under P10's emblem");
    assert_eq!(
        engine.state.objects[&jace].counter_count(CounterKind::Loyalty),
        6
    );
    assert_eq!(
        engine.state.stack.last().unwrap().source_permanent_id,
        Some(jace)
    );
}

#[test]
fn teferi_emblem_survives_departure_of_its_creating_sources_owner() {
    use tricerules_cards::{ContinuousEffectKind, ControllerReference};
    use tricerules_core::TurnStep;

    let mut engine = setup_four_player(26_102_729);
    // Synthetic placement and loyalty set the registered Teferi up for its real paid -10;
    // ownership remains P20 while a synthetic layer 2 effect gives current control to P10.
    let teferi = inject_permanent_on_battlefield(&mut engine, 1, TEFERI);
    engine
        .state
        .objects
        .get_mut(&teferi)
        .expect("synthetically placed registered Teferi")
        .set_counter(CounterKind::Loyalty, 10);
    stage_synthetic_layer2_control(&mut engine, teferi, CONTROLLER);
    assert_eq!(engine.state.objects[&teferi].owner, OPPONENT);
    assert_eq!(
        engine.characteristics(teferi).unwrap().controller,
        CONTROLLER
    );
    assert!(ability_info(&mut engine, teferi, 2).activatable);

    let marker = activate_ultimate_and_create_marker(&mut engine, CONTROLLER, teferi);
    assert_eq!(engine.state.objects[&teferi].owner, OPPONENT);
    assert_eq!(engine.state.objects[&teferi].zone, Zone::Graveyard);
    assert_eq!(engine.state.stack.len(), 0);
    assert_eq!(
        engine
            .state
            .static_emblems
            .iter()
            .find(|item| item.object_id == marker)
            .unwrap()
            .controller,
        CONTROLLER
    );
    // The temporary synthetic control effect has served its setup purpose; the marker is
    // controller-scoped and carries no dependency on this effect or the Teferi object.
    engine.state.continuous_effects.retain(|effect| {
        !matches!(
            effect.kind,
            ContinuousEffectKind::Layer2Control {
                controller: ControllerReference::Fixed(CONTROLLER)
            }
        )
    });
    let jace = inject_permanent_on_battlefield(&mut engine, 0, JACE);
    engine
        .state
        .objects
        .get_mut(&jace)
        .expect("synthetically placed registered Jace")
        .set_counter(CounterKind::Loyalty, 5);

    engine
        .apply_scenario_command(OPPONENT, &concede())
        .expect("the creating Teferi's owner leaves while P10 remains");
    assert!(engine.state.players[1].has_lost);
    assert_eq!(
        engine
            .state
            .players
            .iter()
            .filter(|player| !player.has_lost)
            .count(),
        3,
        "the three surviving players continue the four-player game"
    );
    assert!(engine.state.winner().is_none());
    assert!(
        !engine.state.objects.contains_key(&teferi),
        "P20's owned Teferi leaves the game with its owner"
    );
    let retained = engine
        .state
        .static_emblems
        .iter()
        .find(|item| item.object_id == marker)
        .expect("the source owner's departure does not remove P10's emblem");
    assert_eq!(retained.controller, CONTROLLER);
    assert_eq!(
        retained.effects,
        [StaticEmblemEffect::PlaneswalkerLoyaltyInstantTiming]
    );
    assert_eq!(engine.state.objects[&jace].owner, CONTROLLER);

    // Synthetic EndStep priority isolates the permission after the source owner has departed.
    engine.state.turn_step = TurnStep::EndStep;
    engine.state.priority_idx = engine.state.player_idx(CONTROLLER).expect("P10 seat");
    engine.state.passes_since_stack_change = 0;
    let surviving_offer = ability_info(&mut engine, jace, 0);
    assert!(surviving_offer.activatable);
    let activation = activate_ability_for(&engine, jace, 0, vec![]);
    engine
        .apply_scenario_command(CONTROLLER, &activation)
        .expect("the retained marker still grants Jace's actual ability timing");
    assert_eq!(
        engine.state.objects[&jace].counter_count(CounterKind::Loyalty),
        6
    );
    assert_eq!(
        engine.state.stack.last().unwrap().source_permanent_id,
        Some(jace)
    );
}

#[test]
fn teferi_emblem_and_stolen_jace_leave_with_the_emblem_controller() {
    use tricerules_core::TurnStep;

    let mut engine = setup_four_player(26_102_730);
    // Synthetic placement and loyalty supply a registered Teferi's real -10 activation.
    let teferi = inject_permanent_on_battlefield(&mut engine, 0, TEFERI);
    engine
        .state
        .objects
        .get_mut(&teferi)
        .expect("synthetically placed registered Teferi")
        .set_counter(CounterKind::Loyalty, 10);
    let marker = activate_ultimate_and_create_marker(&mut engine, CONTROLLER, teferi);
    assert_eq!(engine.state.objects[&teferi].owner, CONTROLLER);
    assert_eq!(engine.state.objects[&teferi].zone, Zone::Graveyard);

    // Synthetic placement preserves Jace's owner; only a layer 2 effect gives the marked player
    // current control. Both leave-game outcomes therefore remain distinguishable.
    let jace = inject_permanent_on_battlefield(&mut engine, 1, JACE);
    engine
        .state
        .objects
        .get_mut(&jace)
        .expect("synthetically placed registered Jace")
        .set_counter(CounterKind::Loyalty, 5);
    stage_synthetic_layer2_control(&mut engine, jace, CONTROLLER);
    assert_eq!(engine.state.objects[&jace].owner, OPPONENT);
    assert_eq!(engine.characteristics(jace).unwrap().controller, CONTROLLER);
    engine.state.turn_step = TurnStep::EndStep;
    engine.state.priority_idx = engine.state.player_idx(CONTROLLER).expect("P10 seat");
    engine.state.passes_since_stack_change = 0;
    let marked_offer = ability_info(&mut engine, jace, 0);
    assert!(marked_offer.activatable);

    engine
        .apply_scenario_command(CONTROLLER, &concede())
        .expect("the emblem controller leaves the four-player game");
    assert!(engine.state.players[0].has_lost);
    assert_eq!(
        engine
            .state
            .players
            .iter()
            .filter(|player| !player.has_lost)
            .count(),
        3,
        "three surviving players remain after P10 concedes"
    );
    assert!(engine.state.winner().is_none());
    assert!(!engine.state.objects.contains_key(&teferi));
    assert!(
        !engine
            .state
            .static_emblems
            .iter()
            .any(|item| item.object_id == marker),
        "the emblem leaves with its controller"
    );
    assert!(!engine
        .state
        .continuous_effects
        .iter()
        .any(|effect| effect.source_id == Some(marker)));
    assert_eq!(engine.state.objects[&jace].owner, OPPONENT);
    assert_eq!(engine.characteristics(jace).unwrap().controller, OPPONENT);
    assert_eq!(engine.state.priority_player_id(), OPPONENT);
    assert!(!ability_info(&mut engine, jace, 0).activatable);
    let activation = activate_ability_for(&engine, jace, 0, vec![]);
    reject_without_mutation(&mut engine, OPPONENT, &activation);
    assert_eq!(
        engine.state.objects[&jace].counter_count(CounterKind::Loyalty),
        5
    );
}

#[test]
fn multiple_teferi_emblems_do_not_bypass_jaces_per_turn_limit_and_reset() {
    let mut engine = setup(26_102_731);
    let first_teferi = cast_teferi(&mut engine, 0);
    let first_marker = activate_ultimate_and_create_marker(&mut engine, CONTROLLER, first_teferi);
    let second_teferi = cast_teferi(&mut engine, 0);
    let second_marker = activate_ultimate_and_create_marker(&mut engine, CONTROLLER, second_teferi);
    assert_ne!(first_marker, second_marker);
    assert_eq!(engine.state.static_emblems.len(), 2);
    assert!(engine
        .state
        .static_emblems
        .iter()
        .all(|marker| marker.controller == CONTROLLER));

    let jace = cast_jace(&mut engine, 0);
    let first_turn = engine.state.turn_instance;
    end_active_turn(&mut engine, CONTROLLER);
    assert_eq!(engine.state.active_player_id(), OPPONENT);
    let first_bolt = cast_bolt(&mut engine, 1, target_player(CONTROLLER));
    engine
        .apply_scenario_command(OPPONENT, &pass())
        .expect("the active Bolt caster passes to Jace's controller");
    assert_eq!(engine.state.priority_player_id(), CONTROLLER);
    assert_eq!(engine.state.stack.last().unwrap().id, first_bolt);
    let offer = ability_info(&mut engine, jace, 0);
    assert!(
        offer.activatable,
        "both controller-owned emblems grant timing"
    );
    let activation = activate_ability_for(&engine, jace, 0, vec![]);
    engine
        .apply_scenario_command(CONTROLLER, &activation)
        .expect("activate Jace once with two matching emblems");
    assert_eq!(
        engine.state.objects[&jace].counter_count(CounterKind::Loyalty),
        6
    );
    assert_eq!(
        engine.state.stack.last().unwrap().source_permanent_id,
        Some(jace)
    );

    engine
        .apply_scenario_command(CONTROLLER, &pass())
        .expect("Jace's controller passes back to the active player");
    cast_bolt(&mut engine, 1, target_player(CONTROLLER));
    engine
        .apply_scenario_command(OPPONENT, &pass())
        .expect("the second Bolt caster passes back to Jace's controller");
    let spent = ability_info(&mut engine, jace, 0);
    assert!(
        !spent.activatable,
        "two Teferi emblems do not allow a second loyalty activation this turn"
    );
    let second_activation = activate_ability_for(&engine, jace, 0, vec![]);
    reject_without_mutation(&mut engine, CONTROLLER, &second_activation);

    drain_stack_answering_jace_scry(&mut engine);
    advance_to_main1_from_game_start(&mut engine);
    end_active_turn(&mut engine, OPPONENT);
    advance_to_main1_from_game_start(&mut engine);
    end_active_turn(&mut engine, CONTROLLER);
    assert_eq!(engine.state.active_player_id(), OPPONENT);
    assert!(
        engine.state.turn_instance > first_turn,
        "the test crossed actual turn boundaries before checking the reset"
    );

    let next_turn_bolt = cast_bolt(&mut engine, 1, target_player(CONTROLLER));
    engine
        .apply_scenario_command(OPPONENT, &pass())
        .expect("the next-turn Bolt caster passes to Jace's controller");
    assert_eq!(engine.state.priority_player_id(), CONTROLLER);
    assert_eq!(engine.state.stack.last().unwrap().id, next_turn_bolt);
    let reset_offer = ability_info(&mut engine, jace, 0);
    assert!(
        reset_offer.activatable,
        "the real per-turn loyalty limit resets on a later opponent turn"
    );
    let reset_activation = activate_ability_for(&engine, jace, 0, vec![]);
    engine
        .apply_scenario_command(CONTROLLER, &reset_activation)
        .expect("Jace may activate again in the later turn");
    assert_eq!(
        engine.state.objects[&jace].counter_count(CounterKind::Loyalty),
        7
    );
    assert_eq!(
        engine.state.stack.last().unwrap().source_permanent_id,
        Some(jace)
    );
}

#[test]
fn oath_veil_carth_and_emblem_allow_three_then_reject_a_fourth_offturn_jace_use() {
    let mut engine = setup(26_102_732);

    // This is a noncommander gameplay fixture. Only hand and mana are synthetic; each tested
    // permanent is cast from hand and uses its registered card and entry/activation semantics.
    let oath = cast_oath_of_teferi_first(&mut engine);
    assert!(engine.state.players[0].battlefield.contains(&oath));
    let teferi = cast_teferi(&mut engine, 0);
    assert_eq!(
        engine.state.objects[&teferi].counter_count(CounterKind::Loyalty),
        5
    );

    // Oath permits two real +1 activations on each of two turns, bringing Teferi from five to
    // nine without assigning loyalty counters in the fixture.
    for expected in [7, 9] {
        activate_and_resolve_teferi_plus_one(&mut engine, teferi);
        activate_and_resolve_teferi_plus_one(&mut engine, teferi);
        assert_eq!(
            engine.state.objects[&teferi].counter_count(CounterKind::Loyalty),
            expected
        );
        advance_from_p10_through_p20_to_p10(&mut engine);
    }

    activate_and_resolve_teferi_plus_one(&mut engine, teferi);
    assert_eq!(
        engine.state.objects[&teferi].counter_count(CounterKind::Loyalty),
        10
    );
    assert!(ability_info(&mut engine, teferi, 2).activatable);
    let ultimate = activate_ability_for(&engine, teferi, 2, vec![]);
    engine
        .apply_scenario_command(CONTROLLER, &ultimate)
        .expect("pay Teferi's actual -10 loyalty cost after the real +1s");
    assert_eq!(
        engine.state.objects[&teferi].counter_count(CounterKind::Loyalty),
        0
    );
    assert_eq!(engine.state.objects[&teferi].zone, Zone::Graveyard);
    assert_eq!(engine.state.stack.len(), 1);
    pass_priority_round(&mut engine);
    assert!(engine.state.stack.is_empty());
    assert_eq!(engine.state.static_emblems.len(), 1);
    let emblem = &engine.state.static_emblems[0];
    assert_eq!(emblem.controller, CONTROLLER);
    assert_eq!(
        emblem.effects,
        [StaticEmblemEffect::PlaneswalkerLoyaltyInstantTiming]
    );

    let carth = cast_and_resolve_carth(&mut engine);
    assert_eq!(engine.state.objects[&carth].zone, Zone::Battlefield);
    let veil = cast_registered_spell(
        &mut engine,
        0,
        CHAIN_VEIL,
        ManaGift {
            c: 4,
            ..Default::default()
        },
    );
    assert_eq!(engine.state.objects[&veil].zone, Zone::Battlefield);
    let jace = cast_jace(&mut engine, 0);
    assert_eq!(
        engine.state.objects[&jace].counter_count(CounterKind::Loyalty),
        5
    );
    assert_eq!(
        engine.state.objects[&jace].controller, CONTROLLER,
        "P10 controls the actual Jace for the emblem, Oath, Chain Veil and Carth effects"
    );

    end_active_turn(&mut engine, CONTROLLER);
    advance_to_main1_from_game_start(&mut engine);
    assert_eq!(engine.state.active_player_id(), OPPONENT);
    cast_bolt(&mut engine, 1, target_player(CONTROLLER));
    assert_eq!(engine.state.priority_player_id(), OPPONENT);
    assert_eq!(engine.state.stack.len(), 1);
    assert_eq!(engine.state.stack[0].card_id, "lightning_bolt");
    engine
        .apply_scenario_command(OPPONENT, &pass())
        .expect("P20 passes priority to P10 while Bolt stays unresolved");
    assert_eq!(engine.state.priority_player_id(), CONTROLLER);
    assert_eq!(engine.state.stack.len(), 1);

    assert!(!engine.state.objects[&veil].tapped);
    give_mana(
        &mut engine,
        CONTROLLER,
        ManaGift {
            c: 4,
            ..Default::default()
        },
    );
    let veil_activation = activate_ability_for(&engine, veil, 0, vec![]);
    engine
        .apply_scenario_command(CONTROLLER, &veil_activation)
        .expect("activate the actual Chain Veil for {4} and tap it over the unresolved Bolt");
    assert!(engine.state.objects[&veil].tapped);
    assert_eq!(engine.state.players[0].mana_pool, Default::default());
    assert_eq!(engine.state.stack.len(), 2);
    assert_eq!(engine.state.stack[0].card_id, "lightning_bolt");
    assert_eq!(engine.state.stack[1].source_permanent_id, Some(veil));

    pass_priority_round(&mut engine);
    assert_eq!(engine.state.stack.len(), 1);
    assert_eq!(engine.state.stack[0].card_id, "lightning_bolt");
    assert_eq!(
        engine.state.priority_player_id(),
        OPPONENT,
        "Veil resolution returns priority to the active P20"
    );
    engine
        .apply_scenario_command(OPPONENT, &pass())
        .expect("P20 must pass priority to P10 again after Veil resolves");
    assert_eq!(engine.state.priority_player_id(), CONTROLLER);
    assert_eq!(engine.state.stack.len(), 1);

    // The real Teferi emblem supplies off-turn timing; Oath gives two uses and the resolved Veil
    // one more. Carth's actual static makes Jace's printed +1 cost two loyalty each time.
    for (activation_index, expected_loyalty) in [7, 9, 11].into_iter().enumerate() {
        assert_eq!(engine.state.priority_player_id(), CONTROLLER);
        let before = engine.state.objects[&jace].counter_count(CounterKind::Loyalty);
        assert_eq!(before + 2, expected_loyalty);
        assert!(ability_info(&mut engine, jace, 0).activatable);
        let activation = activate_ability_for(&engine, jace, 0, vec![]);
        engine
            .apply_scenario_command(CONTROLLER, &activation)
            .expect("queue another actual Jace +1 without passing priority");
        assert_eq!(
            engine.state.objects[&jace].counter_count(CounterKind::Loyalty),
            expected_loyalty,
            "Carth makes the +1 activation add two loyalty"
        );
        assert_eq!(engine.state.priority_player_id(), CONTROLLER);
        assert_eq!(engine.state.stack.len(), activation_index + 2);
        assert_eq!(engine.state.stack[0].card_id, "lightning_bolt");
        assert_eq!(
            engine.state.stack.last().unwrap().source_permanent_id,
            Some(jace)
        );
    }

    let fourth_offer = ability_info(&mut engine, jace, 0);
    assert!(
        !fourth_offer.activatable,
        "Oath and one actual Veil activation allow three, not four, Jace uses this turn"
    );
    let before_rejection = engine.diagnostic_snapshot().unwrap();
    let fourth = activate_ability_for(&engine, jace, 0, vec![]);
    assert!(engine.apply_command(CONTROLLER, &fourth).is_err());
    assert_eq!(engine.diagnostic_snapshot().unwrap(), before_rejection);
    assert_eq!(
        engine.state.objects[&jace].counter_count(CounterKind::Loyalty),
        11
    );
    assert_eq!(engine.state.stack.len(), 4);
    assert_eq!(engine.state.stack[0].card_id, "lightning_bolt");
}
