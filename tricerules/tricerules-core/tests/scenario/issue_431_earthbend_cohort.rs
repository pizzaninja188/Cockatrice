//! Actual-card scenarios for the first #431 Earthbend cohort.
//!
//! Expectations are printed Oracle values. CR 701.66: target land you control becomes a 0/0 land
//! creature with haste and gets N +1/+1 counters; when it dies or is exiled it returns tapped.
//! Pinned rulings: if the land target becomes illegal, earthbend does nothing, and a spell with
//! no other target doesn't resolve (CR 608.2b). Generic animation/return mechanics are covered by
//! `type_adding_effects`; these tests pin each card's own trigger, quantity and targets.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::primitives::CounterKind;
use tricerules_cards::Keyword;
use tricerules_core::{TurnStep, Zone};
use tricerules_proto::ruled::v1::{ChooseTriggerTarget, StackPushed, TargetRef};

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[]), deck_with("island", &[])]);
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("engine");
    advance_to_main1_from_game_start(&mut e);
    grant_pool(&mut e, 0);
    e
}

fn cast(e: &mut GameEngine, card: &str, targets: Vec<TargetRef>) -> u32 {
    let oid = inject_card_into_hand(e, 0, card);
    grant_pool(e, 0);
    let slot = hand_index_for_card(e, 0, card);
    semantic::accepted(e, 0, &cast_spell(slot, targets));
    oid
}

fn choose_trigger_targets(targets: Vec<TargetRef>) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: Vec::new(),
            targets,
        })),
    }
}

/// Asserts the printed Earthbend result: a 0/0 land creature with haste and `n` counters.
fn assert_earthbent(e: &GameEngine, land: u32, n: u32) {
    let c = e.characteristics(land).expect("earthbent land");
    assert!(c.is_creature(), "the land is a creature");
    assert!(c.types.iter().any(|t| t == "Land"), "still a land");
    assert_eq!((c.power, c.toughness), (Some(n), Some(n)));
    assert!(e.effective_has_keyword(land, Keyword::Haste));
    assert_eq!(
        e.state.objects[&land].counter_count(CounterKind::PlusOnePlusOne),
        n
    );
}

fn assert_unbent(e: &GameEngine, land: u32) {
    assert!(!e.characteristics(land).expect("land").is_creature());
    assert_eq!(
        e.state.objects[&land].counter_count(CounterKind::PlusOnePlusOne),
        0
    );
}

fn kill(e: &mut GameEngine, creature: u32) {
    cast(e, "murder", target_object(creature));
    resolve_entire_stack_two_player(e);
}

/// CR 701.66 reminder text, which the engine owns because the return trigger is engine-created.
const EARTHBEND_RETURN_TEXT: &str =
    "When it dies or is exiled, return it to the battlefield tapped.";

/// Kills the earthbent land and returns the public stack push of its return trigger (#528).
fn kill_and_capture_return_trigger(e: &mut GameEngine, land: u32) -> StackPushed {
    cast(e, "murder", target_object(land));
    let mut pushed = Vec::new();
    loop {
        answer_trigger_order_in_engine_order(e);
        if e.state.stack.is_empty() {
            break;
        }
        pushed.extend(
            pass_both_players_with_batch(e)
                .events
                .into_iter()
                .filter_map(|event| match event.ev {
                    Some(Ev::StackPushed(push)) if push.is_triggered => Some(push),
                    _ => None,
                }),
        );
    }
    assert_eq!(pushed.len(), 1, "exactly the return trigger: {pushed:?}");
    pushed.remove(0)
}

/// The return trigger shows its rules text, never an Oracle line of the earthbending source.
fn assert_return_trigger_text(push: &StackPushed, source_name: &str) {
    assert_eq!(push.ability_annotation, EARTHBEND_RETURN_TEXT);
    let presentation = push
        .primary_presentation
        .as_ref()
        .expect("return trigger presentation");
    assert_eq!(presentation.fallback_text, EARTHBEND_RETURN_TEXT);
    assert!(presentation.oracle_line_indices.is_empty());
    assert_eq!(presentation.external_card_name, source_name);
    assert_eq!(
        presentation
            .path
            .last()
            .map(|component| component.id.as_str()),
        Some("earthbend_return")
    );
}

#[test]
fn earthbending_lesson_earthbends_four_and_the_land_returns_tapped() {
    let mut e = engine(431_101);
    let forest = inject_permanent_on_battlefield(&mut e, 0, "forest");
    let lesson = cast(&mut e, "earthbending_lesson", target_object(forest));
    resolve_entire_stack_two_player(&mut e);
    assert_earthbent(&e, forest, 4);
    assert_eq!(e.state.objects[&lesson].zone, Zone::Graveyard);

    let push = kill_and_capture_return_trigger(&mut e, forest);
    assert_return_trigger_text(&push, "Earthbending Lesson");
    assert_eq!(e.state.objects[&forest].zone, Zone::Battlefield);
    assert!(e.state.objects[&forest].tapped, "returns tapped");
    assert_unbent(&e, forest);
}

#[test]
fn earthbending_lesson_requires_a_land_you_control_and_fizzles_if_it_is_bounced() {
    let mut e = engine(431_102);
    let theirs = inject_permanent_on_battlefield(&mut e, 1, "island");
    inject_card_into_hand(&mut e, 0, "earthbending_lesson");
    let slot = hand_index_for_card(&e, 0, "earthbending_lesson");
    let before = e.state.command_index;
    e.apply_scenario_command(0, &cast_spell(slot, target_object(theirs)))
        .expect_err("only a land you control is a legal target");
    assert_eq!(e.state.command_index, before);
    assert!(!e.characteristics(theirs).unwrap().is_creature());

    let forest = inject_permanent_on_battlefield(&mut e, 0, "forest");
    let lesson = semantic_cast_from_slot(&mut e, "earthbending_lesson", forest);
    cast(&mut e, "boomerang", target_object(forest));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&forest].zone, Zone::Hand);
    assert_eq!(e.state.objects[&lesson].zone, Zone::Graveyard);
    assert!(
        e.state.active_event_observers.is_empty(),
        "no earthbend return was set up"
    );
}

fn semantic_cast_from_slot(e: &mut GameEngine, card: &str, target: u32) -> u32 {
    let oid = e.state.players[0]
        .hand
        .iter()
        .copied()
        .find(|oid| e.state.objects[oid].card_id == card)
        .expect("card in hand");
    grant_pool(e, 0);
    let slot = hand_index_for_card(e, 0, card);
    semantic::accepted(e, 0, &cast_spell(slot, target_object(target)));
    oid
}

#[test]
fn earth_village_ruffians_earthbends_two_when_it_dies() {
    let mut e = engine(431_201);
    let forest = inject_permanent_on_battlefield(&mut e, 0, "forest");
    let ruffians = inject_permanent_on_battlefield(&mut e, 0, "earth_village_ruffians");
    kill(&mut e, ruffians);
    assert_eq!(e.state.objects[&ruffians].zone, Zone::Graveyard);
    assert_eq!(e.state.pending_triggers.len(), 1, "dies trigger");
    e.apply_scenario_command(0, &choose_trigger_targets(target_object(forest)))
        .expect("choose the land");
    resolve_entire_stack_two_player(&mut e);
    assert_earthbent(&e, forest, 2);

    // A permanent's trigger is the parent presentation; the return child keeps its own text.
    let push = kill_and_capture_return_trigger(&mut e, forest);
    assert_return_trigger_text(&push, "Earth Village Ruffians");
    assert!(e.state.objects[&forest].tapped, "returns tapped");
}

#[test]
fn earth_village_ruffians_dying_without_a_land_puts_no_trigger_on_the_stack() {
    let mut e = engine(431_202);
    inject_permanent_on_battlefield(&mut e, 1, "island");
    let ruffians = inject_permanent_on_battlefield(&mut e, 0, "earth_village_ruffians");
    kill(&mut e, ruffians);
    assert!(e.state.pending_triggers.is_empty());
    assert!(e.state.stack.is_empty(), "no legal land target (CR 603.3d)");
}

#[test]
fn haru_earthbends_one_for_another_ally_you_control_only() {
    let mut e = engine(431_301);
    let forest = inject_permanent_on_battlefield(&mut e, 0, "forest");
    inject_card_into_hand(&mut e, 0, "haru,_hidden_talent");
    move_ready_to_battlefield(&mut e, 0, "haru,_hidden_talent");
    assert!(
        e.state.pending_triggers.is_empty(),
        "Haru's own entry is not another Ally"
    );

    inject_card_into_hand(&mut e, 0, "grizzly_bears");
    move_ready_to_battlefield(&mut e, 0, "grizzly_bears");
    assert!(e.state.pending_triggers.is_empty(), "a non-Ally creature");

    inject_card_into_hand(&mut e, 1, "compassionate_healer");
    move_ready_to_battlefield(&mut e, 1, "compassionate_healer");
    assert!(e.state.pending_triggers.is_empty(), "an opponent's Ally");

    inject_card_into_hand(&mut e, 0, "compassionate_healer");
    move_ready_to_battlefield(&mut e, 0, "compassionate_healer");
    assert_eq!(e.state.pending_triggers.len(), 1);
    e.apply_scenario_command(0, &choose_trigger_targets(target_object(forest)))
        .expect("choose the land");
    resolve_entire_stack_two_player(&mut e);
    assert_earthbent(&e, forest, 1);
}

fn enter(e: &mut GameEngine, card: &str) -> u32 {
    inject_card_into_hand(e, 0, card);
    move_ready_to_battlefield(e, 0, card)
}

fn advance_to_end_step(e: &mut GameEngine) {
    for _ in 0..12 {
        if e.state.turn_step == TurnStep::EndStep || !e.state.pending_triggers.is_empty() {
            return;
        }
        e.apply_scenario_command(0, &primitive_yield())
            .expect("advance toward the end step");
    }
    panic!("did not reach the end step");
}

#[test]
fn toph_makes_nontoken_artifacts_you_control_lands_and_earthbends_two_at_end_step() {
    let mut e = engine(431_401);
    let ring = inject_permanent_on_battlefield(&mut e, 0, "sol_ring");
    let their_ring = inject_permanent_on_battlefield(&mut e, 1, "sol_ring");
    let clue = inject_permanent_on_battlefield(&mut e, 0, "clue");
    let is_land = |e: &GameEngine, oid| {
        e.characteristics(oid)
            .unwrap()
            .types
            .iter()
            .any(|t| t == "Land")
    };
    assert!(!is_land(&e, ring));

    enter(&mut e, "toph,_the_first_metalbender");
    assert!(is_land(&e, ring), "nontoken artifact you control is a land");
    assert!(
        e.characteristics(ring)
            .unwrap()
            .types
            .iter()
            .any(|t| t == "Artifact"),
        "in addition to its other types"
    );
    assert!(!is_land(&e, their_ring), "an opponent's artifact");
    assert!(!is_land(&e, clue), "a token artifact");

    advance_to_end_step(&mut e);
    assert_eq!(e.state.pending_triggers.len(), 1, "end-step trigger");
    e.apply_scenario_command(0, &choose_trigger_targets(target_object(clue)))
        .expect_err("a token artifact is not a land");
    e.apply_scenario_command(0, &choose_trigger_targets(target_object(ring)))
        .expect("an artifact made a land is a legal earthbend target");
    resolve_entire_stack_two_player(&mut e);
    assert_earthbent(&e, ring, 2);
}

#[test]
fn toph_ceasing_to_be_on_the_battlefield_ends_the_land_type() {
    let mut e = engine(431_402);
    let ring = inject_permanent_on_battlefield(&mut e, 0, "sol_ring");
    let toph = enter(&mut e, "toph,_the_first_metalbender");
    assert!(e
        .characteristics(ring)
        .unwrap()
        .types
        .iter()
        .any(|t| t == "Land"));
    kill(&mut e, toph);
    assert!(!e
        .characteristics(ring)
        .unwrap()
        .types
        .iter()
        .any(|t| t == "Land"));
}

#[test]
fn rockalanche_counts_forests_you_control_on_resolution_and_flashes_back() {
    let mut e = engine(431_501);
    let forest = inject_permanent_on_battlefield(&mut e, 0, "forest");
    inject_permanent_on_battlefield(&mut e, 0, "forest");
    inject_permanent_on_battlefield(&mut e, 0, "mountain");
    inject_permanent_on_battlefield(&mut e, 1, "forest");
    let rock = cast(&mut e, "rockalanche", target_object(forest));
    inject_permanent_on_battlefield(&mut e, 0, "forest");
    resolve_entire_stack_two_player(&mut e);
    assert_earthbent(&e, forest, 3);
    assert_eq!(e.state.objects[&rock].zone, Zone::Graveyard);

    let mountain = e.state.players[0]
        .battlefield
        .iter()
        .copied()
        .find(|oid| e.state.objects[oid].card_id == "mountain")
        .unwrap();
    let generation = e.state.zone_change_generation[&rock];
    let flashback = RuledCommand {
        cmd: Some(Cmd::CastSpell(tricerules_proto::ruled::v1::CastSpell {
            cast_method: tricerules_proto::ruled::v1::CastMethod::Flashback as i32,
            source: Some(graveyard_cast_source(rock, generation)),
            targets: target_object(mountain),
            ..Default::default()
        })),
    };
    e.state.players[0].mana_pool = Default::default();
    give_mana(
        &mut e,
        0,
        ManaGift {
            g: 1,
            c: 4,
            ..Default::default()
        },
    );
    let before = e.state.command_index;
    e.apply_scenario_command(0, &flashback)
        .expect_err("flashback costs {5}{G}");
    assert_eq!(e.state.command_index, before);
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    semantic::accepted(&mut e, 0, &flashback);
    resolve_entire_stack_two_player(&mut e);
    // The animated Forest is still a Forest: three Forests you control.
    assert_earthbent(&e, mountain, 3);
    assert_eq!(e.state.objects[&rock].zone, Zone::Exile);
}

#[test]
fn sandbenders_storm_destroys_only_a_creature_with_power_four_or_greater() {
    let mut e = engine(431_601);
    let bear = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let maw = inject_permanent_on_battlefield(&mut e, 1, "colossal_dreadmaw");
    inject_card_into_hand(&mut e, 0, "sandbenders_storm");
    let slot = hand_index_for_card(&e, 0, "sandbenders_storm");
    let before = e.state.command_index;
    e.apply_scenario_command(0, &cast_modal_spell(slot, vec![(0, target_object(bear))]))
        .expect_err("a 2/2 is not a legal target");
    assert_eq!(e.state.command_index, before);

    semantic::accepted(
        &mut e,
        0,
        &cast_modal_spell(slot, vec![(0, target_object(maw))]),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&maw].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&bear].zone, Zone::Battlefield);
}

#[test]
fn sandbenders_storm_second_mode_earthbends_three() {
    let mut e = engine(431_602);
    let forest = inject_permanent_on_battlefield(&mut e, 0, "forest");
    inject_card_into_hand(&mut e, 0, "sandbenders_storm");
    let slot = hand_index_for_card(&e, 0, "sandbenders_storm");
    semantic::accepted(
        &mut e,
        0,
        &cast_modal_spell(slot, vec![(1, target_object(forest))]),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_earthbent(&e, forest, 3);
}

#[test]
fn fatal_fissure_earthbends_four_when_the_chosen_creature_dies_this_turn() {
    let mut e = engine(431_701);
    let forest = inject_permanent_on_battlefield(&mut e, 0, "forest");
    let bear = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let other = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    cast(&mut e, "fatal_fissure", target_object(bear));
    resolve_entire_stack_two_player(&mut e);
    assert_unbent(&e, forest);

    kill(&mut e, other);
    assert!(
        e.state.pending_triggers.is_empty(),
        "another creature dying does not trigger"
    );
    kill(&mut e, bear);
    assert_eq!(
        e.state.pending_triggers.len(),
        1,
        "the watched creature died"
    );
    e.apply_scenario_command(0, &choose_trigger_targets(target_object(forest)))
        .expect("the land target is chosen as the delayed trigger is put on the stack");
    resolve_entire_stack_two_player(&mut e);
    assert_earthbent(&e, forest, 4);
}

#[test]
fn fatal_fissure_does_nothing_if_the_creature_survives_the_turn() {
    let mut e = engine(431_702);
    let forest = inject_permanent_on_battlefield(&mut e, 0, "forest");
    let bear = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    cast(&mut e, "fatal_fissure", target_object(bear));
    resolve_entire_stack_two_player(&mut e);
    end_active_turn(&mut e, 0);
    assert_eq!(e.state.active_player_id(), 1);
    grant_pool(&mut e, 1);
    inject_card_into_hand(&mut e, 1, "murder");
    let slot = hand_index_for_card(&e, 1, "murder");
    semantic::accepted(&mut e, 1, &cast_spell(slot, target_object(bear)));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&bear].zone, Zone::Graveyard);
    assert!(
        e.state.pending_triggers.is_empty(),
        "the watch lasted only this turn"
    );
    assert_unbent(&e, forest);
}

#[test]
fn the_cave_of_two_lovers_makes_allies_fetches_a_mountain_or_cave_then_earthbends_three() {
    let mut e = engine(431_901);
    let forest = inject_permanent_on_battlefield(&mut e, 0, "forest");
    let cave = inject_library_card(&mut e, 0, "hidden_cataract");
    let island = inject_library_card(&mut e, 0, "island");

    inject_card_into_hand(&mut e, 0, "the_cave_of_two_lovers");
    let saga = move_ready_to_battlefield(&mut e, 0, "the_cave_of_two_lovers");
    resolve_entire_stack_two_player(&mut e);
    let allies = battlefield_token_oids(&e, 0, "ally_w_1_1");
    assert_eq!(allies.len(), 2, "chapter I");
    for ally in allies {
        let c = e.characteristics(ally).unwrap();
        assert_eq!((c.power, c.toughness), (Some(1), Some(1)));
    }

    e.state.turn_step = TurnStep::Draw;
    e.state.priority_idx = 0;
    e.state.passes_since_stack_change = 0;
    pass_both_players(&mut e);
    assert_eq!(e.state.objects[&saga].counter_count(CounterKind::Lore), 2);
    let first = e.state.priority_player_id();
    let second = 1 - first;
    e.apply_scenario_command(first, &pass()).unwrap();
    let batch = e.apply_scenario_command(second, &pass()).unwrap();
    let choice = find_resolution_choice(&batch).expect("chapter II search");
    assert!(choice.candidate_object_ids.contains(&cave));
    assert!(!choice.candidate_object_ids.contains(&island));
    e.apply_scenario_command(0, &submit_resolution_choice(vec![cave]))
        .expect("fetch the Cave");
    assert_eq!(e.state.objects[&cave].zone, Zone::Hand);

    e.state.turn_step = TurnStep::Draw;
    e.state.priority_idx = 0;
    e.state.passes_since_stack_change = 0;
    pass_both_players(&mut e);
    assert_eq!(e.state.pending_triggers.len(), 1, "chapter III");
    e.apply_scenario_command(0, &choose_trigger_targets(target_object(forest)))
        .expect("choose the land");
    resolve_entire_stack_two_player(&mut e);
    assert_earthbent(&e, forest, 3);
    assert_eq!(
        e.state.objects[&saga].zone,
        Zone::Graveyard,
        "sacrificed after III"
    );
}
