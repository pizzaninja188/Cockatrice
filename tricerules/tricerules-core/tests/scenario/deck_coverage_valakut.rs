//! Actual-card coverage for Valakut, the Molten Pinnacle.
//!
//! Exact Scryfall Oracle text and rulings were checked against the card and rulings endpoints.
//! CR 305.6/305.8 govern the Mountain subtype; CR 614.1c the tapped entry; CR 115.1d, 603.3d,
//! 603.4, 603.5 and 608.2b govern event identity, target timing, intervening-if checks, the
//! resolution-time may choice, and target legality.

use super::helpers::*;
use tricerules_cards::Layout;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, ChooseTriggerTarget, DevCommand, DevMoveCard, DevZone,
    ResolutionChoiceDecision, RuledCommand, SubmitResolutionChoice, TargetRef, TargetRefKind,
};

const VALAKUT: &str = "valakut,_the_molten_pinnacle";
const VALAKUT_FACE: &str = "valakut_the_molten_pinnacle";

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[]), deck_with("island", &[])]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    grant_pool(&mut engine, 0);
    grant_pool(&mut engine, 1);
    engine
}

fn move_owned_card(engine: &mut GameEngine, player: usize, card_id: &str, zone: DevZone) {
    let name = tricerules_cards::registry::global()
        .get(card_id)
        .unwrap_or_else(|| panic!("missing card definition: {card_id}"))
        .name
        .clone();
    let player_id = engine.state.players[player].id;
    engine.enable_dev_commands();
    engine
        .apply_command(
            player_id,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: player_id,
                    dev: Some(tricerules_proto::ruled::v1::dev_command::Dev::MoveCard(
                        DevMoveCard {
                            card_name: name,
                            zone: zone as i32,
                            ready: true,
                        },
                    )),
                })),
            },
        )
        .expect("move card through the engine");
}

fn enter_owned_card(engine: &mut GameEngine, player: usize, card_id: &str) -> u32 {
    inject_card_into_hand(engine, player, card_id);
    move_ready_to_battlefield(engine, player, card_id)
}

fn enter_mountain(engine: &mut GameEngine) -> u32 {
    enter_owned_card(engine, 0, "mountain")
}

fn battlefield_object(engine: &GameEngine, player: usize, card_id: &str) -> u32 {
    *engine.state.players[player]
        .battlefield
        .iter()
        .find(|object_id| engine.state.objects[object_id].card_id == card_id)
        .unwrap_or_else(|| panic!("{card_id} on P{player}'s battlefield"))
}

fn choose_target(target: TargetRef) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            targets: vec![target],
            ..Default::default()
        })),
    }
}

fn choose_player_target(player: i32) -> RuledCommand {
    choose_target(target_player(player).remove(0))
}

fn choose_permanent_target(object_id: u32) -> RuledCommand {
    choose_target(TargetRef {
        object_id,
        kind: TargetRefKind::Permanent as i32,
        ..Default::default()
    })
}

fn resolution_decision(decision: ResolutionChoiceDecision) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            decision: decision as i32,
            selected_branch_index: 0,
            ..Default::default()
        })),
    }
}

fn require_resolution_choice(engine: &mut GameEngine) {
    for _ in 0..12 {
        if engine.state.pending_resolution.is_some() {
            return;
        }
        pass_priority_round(engine);
    }
    panic!("Valakut must ask whether to deal damage after target and condition checks");
}

fn make_one_valakut_trigger(seed: u64) -> (GameEngine, u32) {
    let mut engine = engine(seed);
    enter_owned_card(&mut engine, 0, VALAKUT);
    for _ in 0..5 {
        enter_mountain(&mut engine);
    }
    assert!(engine.state.pending_triggers.is_empty());
    let entrant = enter_mountain(&mut engine);
    assert_eq!(engine.state.pending_triggers.len(), 1);
    (engine, entrant)
}

#[test]
fn valakut_registers_exact_identity_and_complete_land_definition() {
    let card = tricerules_cards::registry::global()
        .get(VALAKUT)
        .expect("reviewed Valakut definition");
    assert_eq!(card.id, VALAKUT);
    assert_eq!(card.name, "Valakut, the Molten Pinnacle");
    assert_eq!(card.layout, Layout::Normal);
    assert_eq!(card.face_count(), 1);

    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), VALAKUT_FACE);
    assert_eq!(face.types, vec!["Land"]);
    assert_eq!(face.activated_abilities.len(), 1);
    assert_eq!(face.triggered_abilities.len(), 1);
    assert_eq!(face.static_abilities.len(), 1);
}

#[test]
fn valakut_enters_tapped_and_can_tap_for_red() {
    let mut enters_tapped = engine(20_261_011);
    inject_card_into_hand(&mut enters_tapped, 0, VALAKUT);
    let slot = hand_index_for_card(&enters_tapped, 0, VALAKUT);
    enters_tapped
        .apply_command(0, &play_land(slot))
        .expect("play Valakut as a land");
    let valakut = battlefield_object(&enters_tapped, 0, VALAKUT);
    assert!(enters_tapped.state.objects[&valakut].tapped);

    let mut mana = engine(20_261_012);
    let valakut = enter_owned_card(&mut mana, 0, VALAKUT);
    assert!(tricerules_cards::registry::global()
        .get(VALAKUT)
        .unwrap()
        .primary_face()
        .activated_abilities[0]
        .is_mana_ability());
    mana.state.objects.get_mut(&valakut).unwrap().tapped = false;
    let before = mana.state.players[0].mana_pool.red;
    let ability = activate_ability_for(&mana, valakut, 0, vec![]);
    semantic::accepted(&mut mana, 0, &ability);
    assert_eq!(mana.state.players[0].mana_pool.red, before + 1);
    assert!(mana.state.objects[&valakut].tapped);
    assert!(mana.state.stack.is_empty(), "a mana ability uses no stack");
}

#[test]
fn valakut_requires_five_other_mountains_and_makes_its_may_choice_on_resolution() {
    let mut below = engine(20_261_013);
    enter_owned_card(&mut below, 0, VALAKUT);
    for _ in 0..4 {
        enter_mountain(&mut below);
    }
    enter_mountain(&mut below);
    assert!(
        below.state.pending_triggers.is_empty(),
        "four other Mountains fail"
    );

    let (mut declined, _) = make_one_valakut_trigger(20_261_014);
    declined
        .apply_command(0, &choose_player_target(1))
        .expect("choose target when the trigger is put on the stack");
    assert!(declined.state.pending_resolution.is_none());
    require_resolution_choice(&mut declined);
    assert_eq!(declined.state.players[1].life, 20);
    semantic::accepted(
        &mut declined,
        0,
        &resolution_decision(ResolutionChoiceDecision::Decline),
    );
    assert_eq!(declined.state.players[1].life, 20);
    assert!(declined.state.stack.is_empty());

    let (mut accepted, _) = make_one_valakut_trigger(20_261_015);
    accepted
        .apply_command(0, &choose_player_target(1))
        .expect("choose target before the optional resolution choice");
    require_resolution_choice(&mut accepted);
    semantic::accepted(
        &mut accepted,
        0,
        &resolution_decision(ResolutionChoiceDecision::SelectBranch),
    );
    assert_eq!(accepted.state.players[1].life, 17);
    assert!(accepted.state.stack.is_empty());
}

#[test]
fn valakut_counts_two_simultaneous_mountains_as_each_others_fifth_other() {
    let mut engine = engine(20_261_016);
    enter_owned_card(&mut engine, 0, VALAKUT);
    for _ in 0..4 {
        enter_mountain(&mut engine);
    }
    let mountain_a = inject_library_card(&mut engine, 0, "mountain");
    let mountain_b = inject_library_card(&mut engine, 0, "mountain");
    inject_card_into_hand(&mut engine, 0, "circuitous_route");
    let route = hand_index_for_card(&engine, 0, "circuitous_route");
    semantic::accepted(&mut engine, 0, &cast_spell(route, vec![]));
    pass_priority_round(&mut engine);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .presentation
            .choice_kind,
        tricerules_proto::ruled::v1::ChoiceKind::LibrarySearch
    );
    let mut completion = engine
        .apply_command(0, &submit_resolution_choice(vec![mountain_a, mountain_b]))
        .expect("search for two basic Mountains");
    for (_, _, ordered) in answer_simultaneous_entry_order_in_engine_order(&mut engine) {
        completion.events.extend(ordered.events);
    }
    answer_trigger_order_in_engine_order(&mut engine);

    assert_eq!(engine.state.objects[&mountain_a].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&mountain_b].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.pending_triggers.len(),
        1,
        "the first target choice serializes placement of the second same-controller trigger"
    );
    engine
        .apply_command(0, &choose_player_target(1))
        .expect("choose first simultaneous-entry trigger target");
    engine
        .apply_command(0, &choose_player_target(1))
        .expect("choose second simultaneous-entry trigger target");
    for _ in 0..2 {
        require_resolution_choice(&mut engine);
        semantic::accepted(
            &mut engine,
            0,
            &resolution_decision(ResolutionChoiceDecision::SelectBranch),
        );
    }
    assert_eq!(engine.state.players[1].life, 14);
    assert!(engine.state.stack.is_empty());
}

#[test]
fn valakut_rechecks_other_mountains_when_the_trigger_resolves() {
    let mut event_leaves = engine(20_261_017);
    enter_owned_card(&mut event_leaves, 0, VALAKUT);
    for _ in 0..5 {
        enter_mountain(&mut event_leaves);
    }
    let cinder_glade = enter_owned_card(&mut event_leaves, 0, "cinder_glade");
    event_leaves
        .apply_command(0, &choose_player_target(1))
        .expect("choose an Any target");
    // The triggering object itself is gone, but the five other Mountains still count.
    move_owned_card(&mut event_leaves, 0, "cinder_glade", DevZone::Graveyard);
    require_resolution_choice(&mut event_leaves);
    semantic::accepted(
        &mut event_leaves,
        0,
        &resolution_decision(ResolutionChoiceDecision::SelectBranch),
    );
    assert_eq!(
        event_leaves.state.objects[&cinder_glade].zone,
        Zone::Graveyard
    );
    assert_eq!(event_leaves.state.players[1].life, 17);

    let mut failed = engine(20_261_018);
    enter_owned_card(&mut failed, 0, VALAKUT);
    for _ in 0..4 {
        enter_mountain(&mut failed);
    }
    enter_owned_card(&mut failed, 0, "cinder_glade");
    enter_owned_card(&mut failed, 0, "cinder_glade");
    failed
        .apply_command(0, &choose_player_target(1))
        .expect("choose an Any target");
    move_owned_card(&mut failed, 0, "cinder_glade", DevZone::Graveyard);
    pass_priority_round(&mut failed);
    assert!(failed.state.pending_resolution.is_none());
    assert_eq!(failed.state.players[1].life, 20);
    assert!(failed.state.stack.is_empty());
}

#[test]
fn valakut_reentered_mountain_is_a_new_generation_for_the_other_count() {
    let mut engine = engine(20_261_019);
    enter_owned_card(&mut engine, 0, VALAKUT);
    for _ in 0..5 {
        enter_mountain(&mut engine);
    }
    let observed = enter_owned_card(&mut engine, 0, "cinder_glade");
    let trigger_generation = engine.state.zone_change_generation[&observed];
    engine
        .apply_command(0, &choose_player_target(1))
        .expect("choose target for the original Cinder Glade entry");
    move_owned_card(&mut engine, 0, "mountain", DevZone::Graveyard);
    move_owned_card(&mut engine, 0, "cinder_glade", DevZone::Graveyard);
    move_owned_card(&mut engine, 0, "cinder_glade", DevZone::Battlefield);
    let new_generation = engine.state.zone_change_generation[&observed];
    assert!(
        new_generation > trigger_generation,
        "the exact observed physical Mountain returned as a new battlefield generation"
    );
    require_resolution_choice(&mut engine);
    semantic::accepted(
        &mut engine,
        0,
        &resolution_decision(ResolutionChoiceDecision::SelectBranch),
    );
    assert_eq!(engine.state.players[1].life, 17);
}

#[test]
fn valakut_source_leaves_but_its_targeted_trigger_still_resolves() {
    let mut engine = engine(20_261_021);
    let valakut = enter_owned_card(&mut engine, 0, VALAKUT);
    for _ in 0..5 {
        enter_mountain(&mut engine);
    }
    enter_mountain(&mut engine);
    engine
        .apply_command(0, &choose_player_target(1))
        .expect("choose the target before the source leaves");

    move_owned_card(&mut engine, 0, VALAKUT, DevZone::Graveyard);
    assert_eq!(engine.state.objects[&valakut].zone, Zone::Graveyard);
    require_resolution_choice(&mut engine);
    semantic::accepted(
        &mut engine,
        0,
        &resolution_decision(ResolutionChoiceDecision::SelectBranch),
    );
    assert_eq!(
        engine.state.players[1].life, 17,
        "the independent triggered ability deals damage after Valakut leaves"
    );
}

#[test]
fn valakut_fizzles_if_its_only_chosen_permanent_target_leaves() {
    let mut engine = engine(20_261_020);
    enter_owned_card(&mut engine, 0, VALAKUT);
    for _ in 0..5 {
        enter_mountain(&mut engine);
    }
    let target = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    enter_mountain(&mut engine);
    engine
        .apply_command(0, &choose_permanent_target(target))
        .expect("choose a legal permanent target at stack placement");
    inject_card_into_hand(&mut engine, 0, "unsummon");
    grant_pool(&mut engine, 0);
    let unsummon = hand_index_for_card(&engine, 0, "unsummon");
    semantic::accepted(&mut engine, 0, &cast_spell(unsummon, target_object(target)));
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.objects[&target].zone, Zone::Hand);
    pass_priority_round(&mut engine);
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.players[1].life, 20);
    assert!(engine.state.stack.is_empty());
}
