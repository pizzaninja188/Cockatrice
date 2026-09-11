//! Issue #260 — exact generated creature triggers reuse attack, combat-damage, target,
//! library-movement, private Surveil, and canonical token paths.
//!
//! Oracle and rulings checked 2026-09-11. CR 111.10b/111.10s define Food and Map,
//! CR 115.1d and 608.2b govern trigger targets and resolution revalidation, CR 508.1m and
//! 510.2 establish the attack/combat-damage boundaries, CR 603 governs triggered abilities,
//! and CR 701.17/701.25 define mill and Surveil.

use super::helpers::*;
use tricerules_cards::CounterKind;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{permanent_moved, ChoiceKind};

fn resolve_top_stack(engine: &mut GameEngine) -> RuledEventBatch {
    let first = engine.state.priority_player_id();
    let second = if first == engine.state.players[0].id {
        engine.state.players[1].id
    } else {
        engine.state.players[0].id
    };
    engine.apply_command(first, &pass()).expect("first pass");
    engine
        .apply_command(second, &pass())
        .expect("second pass resolves stack item")
}

fn seat_on_top(engine: &mut GameEngine, player: usize, card_ids: &[&str]) -> Vec<u32> {
    let object_ids = card_ids
        .iter()
        .map(|card_id| inject_library_card(engine, player, card_id))
        .collect::<Vec<_>>();
    engine.state.players[player]
        .library
        .retain(|object_id| !object_ids.contains(object_id));
    for object_id in object_ids.iter().rev() {
        engine.state.players[player].library.push_front(*object_id);
    }
    object_ids
}

fn choose_trigger_target(target_object_id: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: Vec::new(),
            targets: vec![TargetRef {
                object_id: target_object_id,
                group_index: 0,
                kind: TargetRefKind::Permanent as i32,
                ..Default::default()
            }],
        })),
    }
}

#[test]
fn generated_boulderborn_dragon_triggers_only_from_its_attack_declaration_and_surveille() {
    let mut engine = GameEngine::new(260_001, &[0, 1], 20, None, true).expect("engine");
    advance_to_declare_attackers(&mut engine);
    let dragon = inject_creature_on_battlefield(&mut engine, 0, "boulderborn_dragon");
    let top = seat_on_top(&mut engine, 0, &["storm_crow"]);

    assert!(
        engine.state.stack.is_empty(),
        "being combat-ready is not attacking"
    );
    engine
        .apply_command(0, &declare_attackers(vec![dragon]))
        .expect("declare Boulderborn Dragon");
    assert_eq!(
        engine.state.stack.len(),
        1,
        "one declaration stages one trigger"
    );

    let resolution = resolve_top_stack(&mut engine);
    let choice = find_resolution_choice(&resolution).expect("Surveil 1 choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibraryLook);
    assert_eq!(choice.deciding_player_id, 0);
    assert_eq!(choice.candidate_object_ids, top);
    engine
        .apply_command(0, &submit_resolution_choice(top.clone()))
        .expect("move the private Surveil candidate to the graveyard");
    assert_eq!(engine.state.objects[&top[0]].zone, Zone::Graveyard);
}

#[test]
fn generated_cartographers_companion_creates_a_functional_canonical_map() {
    let decks = Some(vec![
        deck_with("plains", &["cartographers_companion"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(260_002, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let companion = move_ready_to_battlefield(&mut engine, 0, "cartographers_companion");
    resolve_entire_stack_two_player(&mut engine);
    let maps = battlefield_token_oids(&engine, 0, "map");
    let [map] = maps.as_slice() else {
        panic!("the trigger controller must receive one canonical Map")
    };
    let map = *map;

    let top = seat_on_top(&mut engine, 0, &["storm_crow"])[0];
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    apply_ability(&mut engine, 0, map, 0, target_object(companion))
        .expect("activate canonical Map");
    assert!(battlefield_token_oids(&engine, 0, "map").is_empty());
    pass_both_players(&mut engine);
    assert_eq!(
        engine.state.objects[&companion].counter_count(CounterKind::PlusOnePlusOne),
        1
    );
    engine
        .apply_command(0, &submit_resolution_choice(vec![top]))
        .expect("finish Explore");
}

#[test]
fn generated_venomized_cat_mills_up_to_two_cards_in_library_order_with_move_facts() {
    for (seed, cards) in [
        (260_003, vec!["storm_crow"]),
        (260_004, vec!["storm_crow", "grizzly_bears", "hill_giant"]),
    ] {
        let decks = Some(vec![
            deck_with("swamp", &["venomized_cat"]),
            deck_with("forest", &[]),
        ]);
        let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("engine");
        advance_to_main1_from_game_start(&mut engine);
        let top = seat_on_top(&mut engine, 0, &cards);
        let other_library_cards = engine.state.players[0]
            .library
            .iter()
            .copied()
            .filter(|object_id| !top.contains(object_id))
            .collect::<Vec<_>>();
        engine.state.players[0]
            .library
            .retain(|object_id| top.contains(object_id));
        for object_id in other_library_cards {
            engine
                .state
                .objects
                .get_mut(&object_id)
                .expect("object")
                .zone = Zone::Graveyard;
            engine.state.players[0].graveyard.push(object_id);
        }
        move_ready_to_battlefield(&mut engine, 0, "venomized_cat");
        let resolution = resolve_top_stack(&mut engine);

        let expected = &top[..top.len().min(2)];
        assert!(expected
            .iter()
            .all(|object_id| engine.state.objects[object_id].zone == Zone::Graveyard));
        if top.len() > 2 {
            assert_eq!(engine.state.objects[&top[2]].zone, Zone::Library);
        }
        let moves = permanents_moved_in(&resolution)
            .into_iter()
            .filter(|moved| expected.contains(&moved.object_id))
            .collect::<Vec<_>>();
        assert_eq!(moves.len(), expected.len());
        assert_eq!(
            moves
                .iter()
                .map(|moved| moved.object_id)
                .collect::<Vec<_>>(),
            expected
        );
        assert!(moves
            .iter()
            .all(|moved| moved.destination() == permanent_moved::Destination::Graveyard));
    }
}

#[test]
fn generated_bounce_restricts_targets_and_revalidates_the_exact_generation() {
    let decks = Some(vec![
        deck_with("island", &["bigfin_bouncer", "exclusion_mage"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(260_005, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let own_creature = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let opponent_creature = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let opponent_artifact = inject_permanent_on_battlefield(&mut engine, 1, "icy_manipulator");

    move_ready_to_battlefield(&mut engine, 0, "bigfin_bouncer");
    assert_eq!(engine.state.pending_triggers.len(), 1);
    assert!(engine
        .apply_command(0, &choose_trigger_target(own_creature))
        .is_err());
    assert!(engine
        .apply_command(0, &choose_trigger_target(opponent_artifact))
        .is_err());
    let generation_before = engine
        .state
        .zone_change_generation
        .get(&opponent_creature)
        .copied()
        .unwrap_or(0);
    engine
        .apply_command(0, &choose_trigger_target(opponent_creature))
        .expect("choose legal opponent creature");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&opponent_creature].zone, Zone::Hand);
    assert!(engine.state.players[1].hand.contains(&opponent_creature));
    assert!(engine.state.zone_change_generation[&opponent_creature] > generation_before);

    let stale_target = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    move_ready_to_battlefield(&mut engine, 0, "exclusion_mage");
    engine
        .apply_command(0, &choose_trigger_target(stale_target))
        .expect("choose second legal target");
    engine.state.players[1]
        .battlefield
        .retain(|object_id| *object_id != stale_target);
    engine.state.players[0].battlefield.push(stale_target);
    let object = engine.state.objects.get_mut(&stale_target).expect("target");
    object.base_controller = 0;
    object.controller = 0;
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&stale_target].zone, Zone::Battlefield);
    assert!(engine.state.players[0].battlefield.contains(&stale_target));
}

#[test]
fn generated_eager_trufflesnout_creates_food_only_after_player_combat_damage() {
    let decks = Some(vec![
        deck_with("forest", &["eager_trufflesnout"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(260_006, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let trufflesnout = relocate_to_battlefield(&mut engine, 0, "eager_trufflesnout", false);
    engine.apply_command(0, &primitive_yield()).unwrap();
    pass_both_players(&mut engine);
    engine
        .apply_command(0, &declare_attackers(vec![trufflesnout]))
        .expect("declare attacker");
    engine
        .apply_command(0, &pass())
        .expect("active pass attackers");
    engine
        .apply_command(1, &pass())
        .expect("defender pass attackers");
    engine
        .apply_command(0, &pass())
        .expect("active pass blockers");
    engine.apply_command(1, &pass()).expect("combat damage");
    assert_eq!(engine.state.players[1].life, 16);
    assert_eq!(engine.state.stack.len(), 1);
    resolve_entire_stack_two_player(&mut engine);
    let foods = battlefield_token_oids(&engine, 0, "food");
    let [food] = foods.as_slice() else {
        panic!("positive player combat damage must create one canonical Food")
    };
    let food = *food;
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    apply_ability(&mut engine, 0, food, 0, vec![]).expect("activate canonical Food");
    pass_both_players(&mut engine);
    assert_eq!(engine.state.players[0].life, 23);
    assert!(!engine.state.objects.contains_key(&food));

    let decks = Some(vec![
        deck_with("forest", &["eager_trufflesnout"]),
        deck_with("forest", &["giant_spider"]),
    ]);
    let mut blocked = GameEngine::new(260_007, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut blocked);
    let attacker = relocate_to_battlefield(&mut blocked, 0, "eager_trufflesnout", false);
    let blocker = relocate_to_battlefield(&mut blocked, 1, "giant_spider", false);
    blocked.apply_command(0, &primitive_yield()).unwrap();
    pass_both_players(&mut blocked);
    blocked
        .apply_command(0, &declare_attackers(vec![attacker]))
        .unwrap();
    blocked.apply_command(0, &pass()).unwrap();
    blocked.apply_command(1, &pass()).unwrap();
    blocked
        .apply_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: attacker,
                blocker_id: blocker,
            }]),
        )
        .expect("block all trample damage");
    blocked.apply_command(0, &pass()).unwrap();
    blocked.apply_command(1, &pass()).unwrap();
    assert_eq!(blocked.state.players[1].life, 20);
    assert!(blocked.state.stack.is_empty());
    assert!(battlefield_token_oids(&blocked, 0, "food").is_empty());
}
